# frozen_string_literal: true

require "async"
require "async/barrier"
require "logger"
require "opentelemetry-api"
require "prosody/version"

module Prosody
  # Provides a mechanism for canceling asynchronous tasks.
  # Used by the AsyncTaskProcessor to allow tasks to be canceled while in-flight.
  class CancellationToken
    # Creates a new token with an internal queue for signaling.
    def initialize
      @queue = Queue.new
    end

    # Signals that the associated task should be canceled.
    # Wakes up any threads waiting on #wait.
    def cancel
      @queue.push(:cancel)
    end

    # Blocks until cancellation is requested.
    #
    # @return [Boolean] Always returns true after cancellation is received
    def wait
      @queue.pop
      true
    end
  end

  # Internal command types for the AsyncTaskProcessor's command queue.
  # Implements a command pattern for communication with the processor thread.
  module Commands
    # Base class for all processor commands
    class Command; end

    # Command to execute a task with the given parameters
    class Execute < Command
      # Task identifier for logging and debugging
      attr_reader :task_id

      # The block of code to execute
      attr_reader :block

      # Callback to invoke when execution completes or fails
      attr_reader :callback

      # Cancellation token for this task
      attr_reader :token

      # OpenTelemetry context carrier for trace propagation
      attr_reader :carrier

      # Creates a new execute command with all required parameters
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

    # Command that signals the processor to shut down
    class Shutdown < Command; end
  end

  # Processes asynchronous tasks in a dedicated thread with OpenTelemetry tracing.
  #
  # This processor handles:
  # - Executing tasks asynchronously using the async gem
  # - Propagating OpenTelemetry context for distributed tracing
  # - Providing cancellation support for in-flight tasks
  # - Ensuring graceful shutdown when requested
  class AsyncTaskProcessor
    # Creates a new processor with the given logger
    #
    # @param logger [Logger] Logger for diagnostic messages (defaults to STDOUT)
    def initialize(logger = Logger.new($stdout))
      @logger = logger
      @command_queue = Queue.new
      @processing_thread = nil
      @tracer = nil
    end

    # Starts the processor by launching a dedicated thread.
    # The OpenTelemetry tracer is initialized in the processing thread
    # to avoid crossing thread boundaries.
    #
    # Does nothing if the processor is already running.
    def start
      return if @processing_thread&.alive?

      @logger.debug("Starting async task processor")
      @processing_thread = Thread.new do
        @tracer = OpenTelemetry.tracer_provider.tracer(
          "Prosody::AsyncTaskProcessor",
          Prosody::VERSION
        )
        process_commands
      end
    end

    # Gracefully stops the processor.
    # Tasks in progress will complete before the processor fully shuts down.
    #
    # Does nothing if the processor is already stopped.
    def stop
      return unless @processing_thread&.alive?
      @logger.debug("Stopping async task processor")
      @command_queue.push(Commands::Shutdown.new)
    end

    # Submits a task for asynchronous execution
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

    # Main processing loop for the async thread.
    # Uses the async gem to handle concurrent task execution.
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
      parent_ctx = OpenTelemetry.propagation.extract(
        carrier,
        getter: OpenTelemetry::Context::Propagation.text_map_getter
      )

      # Execute within the extracted OpenTelemetry context
      OpenTelemetry::Context.with_current(parent_ctx) do
        @tracer.in_span(
          "ruby-receive",
          kind: :consumer
        ) do |span, _span_ctx|
          @logger.debug("Executing task #{task_id}")

          barrier.async do
            # Launch two concurrent tasks - one for cancellation and one for work

            # Cancellation watcher monitors the token and cancels the task if requested
            Async do |task|
              task.annotate("Cancellation watcher for task #{task_id}")
              begin
                token.wait
                @logger.debug("Cancellation received for task #{task_id}")
                callback.call(false, RuntimeError.new("Task cancelled"))
              rescue => e
                @logger.debug("Cancellation watcher error: #{e.message}")
                span.record_exception(e)
                span.status = OpenTelemetry::Trace::Status.error(e.to_s)
              end
            end

            # Worker executes the actual task and calls back with the result
            Async do |task|
              task.annotate("Worker for task #{task_id}")
              begin
                result = task_block.call
                @logger.debug("Task #{task_id} completed successfully")
                callback.call(true, result)
              rescue => e
                @logger.error("Error executing task #{task_id}: #{e.message}")
                span.record_exception(e)
                span.status = OpenTelemetry::Trace::Status.error(e.to_s)
                callback.call(false, e)
              end
            end
          end
        end
      end
    end
  end
end
