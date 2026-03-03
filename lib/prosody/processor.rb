# frozen_string_literal: true

require "async"
require "async/barrier"
require "logger"
require "opentelemetry-api"
require "prosody/version"

module Prosody
  # Provides a mechanism for canceling asynchronous tasks.
  #
  # This class implements a simple cancellation mechanism using a Ruby Queue,
  # allowing tasks to be safely canceled while they're in progress. Each token
  # maintains its own queue for signaling cancellation.
  class CancellationToken
    # Creates a new cancellation token with an internal queue for signaling.
    def initialize
      @queue = Queue.new
    end

    # Signals that the associated task should be canceled.
    #
    # This method pushes a cancellation signal to the internal queue, which will
    # wake up any threads waiting on #wait.
    def cancel
      @queue.push(:cancel)
    end

    # Blocks until cancellation is requested.
    #
    # This method blocks the current thread until the token is canceled by
    # another thread calling #cancel.
    #
    # @return [Boolean] Always returns true after cancellation is received
    def wait
      @queue.pop
      true
    end
  end

  # Contains command classes for the AsyncTaskProcessor's command queue.
  #
  # This module implements a command pattern for communication with the processor
  # thread, allowing for type-safe message passing between threads.
  module Commands
    # Base class for all processor commands.
    #
    # All commands sent to the AsyncTaskProcessor must inherit from this class
    # for proper type identification.
    class Command; end

    # Command to execute a task with the given parameters.
    #
    # This command encapsulates all the information needed to execute an
    # asynchronous task in the Ruby runtime.
    class Execute < Command
      # Task identifier for logging and debugging
      attr_reader :task_id

      # OpenTelemetry context carrier for trace propagation
      attr_reader :carrier

      # The block of code to execute
      attr_reader :block

      # Callback to invoke when execution completes or fails
      attr_reader :callback

      # Cancellation token for this task
      attr_reader :token

      # Creates a new execute command with all required parameters.
      #
      # @param task_id [String] Unique identifier for the task
      # @param carrier [Hash] OpenTelemetry context carrier with trace information
      # @param block [Proc] The code to execute
      # @param callback [Proc] Called with (success, result) when complete
      # @param token [CancellationToken] Token that can be used to cancel execution
      def initialize(task_id, carrier, block, callback, token)
        @task_id = task_id
        @carrier = carrier
        @block = block
        @callback = callback
        @token = token
      end
    end

    # Command that signals the processor to shut down.
    #
    # When received, the processor will complete all in-flight tasks before
    # shutting down.
    class Shutdown < Command; end
  end

  # Processes asynchronous tasks in a dedicated thread with OpenTelemetry tracing.
  #
  # This processor manages a dedicated Ruby thread that executes tasks asynchronously
  # with proper OpenTelemetry context propagation. It provides:
  #
  # - Task submission and execution in an isolated thread
  # - Cancellation support for in-flight tasks
  # - Context propagation for distributed tracing
  # - Graceful shutdown with task completion
  class AsyncTaskProcessor
    # Creates a new processor with the given logger.
    #
    # @param logger [Logger] Logger for diagnostic messages (defaults to STDOUT)
    def initialize(logger = Logger.new($stdout))
      @logger = logger
      @command_queue = Queue.new
      @processing_thread = nil
      @tracer = nil
    end

    # Starts the processor by launching a dedicated thread.
    #
    # The OpenTelemetry tracer is initialized in the processing thread
    # to avoid crossing thread boundaries. Does nothing if the processor
    # is already running.
    def start
      return if running?

      @logger.debug("Starting async task processor")
      @processing_thread = Thread.new do
        # Initialize the tracer in the processing thread to keep
        # OpenTelemetry context within the same thread
        @tracer = OpenTelemetry.tracer_provider.tracer(
          "Prosody::AsyncTaskProcessor",
          Prosody::VERSION
        )
        process_commands
      end
    end

    # Gracefully stops the processor.
    #
    # Tasks in progress will complete before the processor fully shuts down.
    # Does nothing if the processor is already stopped.
    def stop
      return unless running?

      @logger.debug("Stopping async task processor")
      @command_queue.push(Commands::Shutdown.new)
    end

    # Submits a task for asynchronous execution.
    #
    # @param task_id [String] Unique identifier for the task
    # @param carrier [Hash] OpenTelemetry context carrier for tracing
    # @param callback [Proc] Called with (success, result) when task completes
    # @yield The block to execute asynchronously
    # @return [CancellationToken] Token that can be used to cancel the task
    def submit(task_id, carrier, callback, &task_block)
      token = CancellationToken.new
      @command_queue.push(
        Commands::Execute.new(task_id, carrier, task_block, callback, token)
      )
      token
    end

    private

    # Checks if the processor thread is running.
    #
    # @return [Boolean] true if the processor is running, false otherwise
    def running?
      @processing_thread&.alive?
    end

    # Main processing loop for the async thread.
    #
    # Uses the async gem to handle concurrent task execution and tracks
    # active tasks with a barrier for clean shutdown.
    def process_commands
      Async do
        # Barrier tracks all running tasks for clean shutdown
        barrier = Async::Barrier.new

        loop do
          command = @command_queue.pop

          case command
          when Commands::Execute
            handle_execute(command, barrier)
          when Commands::Shutdown
            @logger.debug("Received shutdown command")
            # Wait for all tasks to complete before shutting down
            barrier.wait
            break
          else
            @logger.warn("Unknown command type: #{command.class}")
          end
        end
      end
    rescue => e
      @logger.error("Error in process_commands: #{e.message}")
      @logger.error(e.backtrace.join("\n"))
    end

    # Handles execution of a task with proper context propagation and error handling.
    #
    # @param command [Commands::Execute] The command containing task details
    # @param barrier [Async::Barrier] Barrier for tracking active tasks
    def handle_execute(command, barrier)
      task_id = command.task_id
      carrier = command.carrier
      token = command.token
      callback = command.callback
      task_block = command.block

      # Extract parent context from the incoming carrier for distributed tracing
      parent_ctx = OpenTelemetry.propagation.extract(carrier)

      # Create the dispatch span as a child of the extracted context, then
      # capture the context with the span active so the fiber inherits it.
      # The span is owned by run_with_cancellation, which finishes it in ensure.
      dispatch_ctx = OpenTelemetry::Context.with_current(parent_ctx) do
        span = @tracer.start_span("async_dispatch", kind: :consumer)
        OpenTelemetry::Trace.with_span(span) { OpenTelemetry::Context.current }
      end

      @logger.debug("Executing task #{task_id}")

      begin
        barrier.async do
          run_with_cancellation(task_id, token, task_block, callback, dispatch_ctx)
        end
      rescue => e
        # If we failed to enqueue, finish the span here since run_with_cancellation
        # will never take ownership.
        OpenTelemetry::Trace.current_span(dispatch_ctx).finish
        raise e
      end
    end

    # Executes a task with proper cancellation support.
    #
    # Spawns a worker task and a cancellation watcher within a barrier. When
    # cancellation is signaled, the worker receives Async::Stop (similar to
    # Python's asyncio.CancelledError). The worker can catch Async::Stop for
    # cleanup. The barrier ensures both tasks are cleaned up when the block
    # exits, even if an unexpected error occurs.
    #
    # @param task_id [String] The task identifier for logging
    # @param token [CancellationToken] The token to monitor for cancellation
    # @param task_block [Proc] The work to execute
    # @param callback [Proc] The callback to notify of completion or error
    # @param dispatch_ctx [OpenTelemetry::Context] Context with async_dispatch span active
    def run_with_cancellation(task_id, token, task_block, callback, dispatch_ctx)
      span = OpenTelemetry::Trace.current_span(dispatch_ctx)
      # Use a barrier to ensure both tasks are cleaned up when block exits
      barrier = Async::Barrier.new

      # Spawn worker task - handles its own result reporting
      worker_task = barrier.async do |task|
        task.annotate("Worker for task #{task_id}")
        # Async fibers do not inherit fiber-local OTel context from their parent,
        # so we must explicitly restore dispatch_ctx so user spans are children
        # of async_dispatch.
        OpenTelemetry::Context.with_current(dispatch_ctx) do
          result = task_block.call
          if callback.call(true, result)
            @logger.debug("Task #{task_id} completed successfully")
          end
        rescue Async::Stop
          # Task was cancelled - report via callback
          if callback.call(false, RuntimeError.new("Task cancelled"))
            @logger.debug("Task #{task_id} was cancelled")
          end
        rescue => e
          if callback.call(false, e)
            @logger.error("Error executing task #{task_id}: #{e.message}")
            span.record_exception(e)
            span.status = OpenTelemetry::Trace::Status.error(e.to_s)
          end
        ensure
          # Always signal the cancellation watcher to stop waiting
          token.cancel
        end
      end

      # Spawn cancellation watcher - bridges the thread-boundary cancellation signal
      # into the fiber scheduler. The CancellationToken is a one-shot channel: the
      # Rust bridge pushes a signal from its thread, and this fiber pops it and
      # translates it into Async::Stop on the worker. We can't call worker_task.stop
      # directly from the Rust thread since Async task control must happen on the
      # scheduler thread.
      barrier.async do |task|
        task.annotate("Cancellation watcher for task #{task_id}")
        begin
          token.wait
          worker_task.stop
        rescue => e
          @logger.debug("Cancellation watcher error: #{e.message}")
          span.record_exception(e)
          span.status = OpenTelemetry::Trace::Status.error(e.to_s)
        end
      end

      # Wait for worker to complete (normally, via Async::Stop, or with error)
      worker_task.wait
    ensure
      # Stop any remaining tasks (primarily the cancellation watcher).
      # Finish the span last so it covers the full execution, and is guaranteed
      # to close even if barrier.stop raises.
      begin
        barrier&.stop
      ensure
        span.finish
      end
    end
  end
end
