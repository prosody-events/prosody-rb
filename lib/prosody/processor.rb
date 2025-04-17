# frozen_string_literal: true

# Simple asynchronous task processing application using the Async gem
# with cancellation support via a cancellation queue.
#
# Each task is provided a CancellationToken which holds a queue.
# To cancel a task, the sender calls `cancel` on the token, which pushes
# a cancellation message on that queue. The worker spawns two async tasks:
# one that runs the task block and one that polls the cancellation queue.
# The cancellation watcher is explicitly stopped once the task block completes.

require "async"
require "async/barrier"
require "logger"

module Prosody
  # CancellationToken uses an internal queue to signal cancellation.
  class CancellationToken
    def initialize
      @queue = Queue.new
    end

    # Signal cancellation by pushing a cancellation message on the queue.
    def cancel
      @queue.push(:cancel)
    end

    # Block until a cancellation message is received.
    def wait
      @queue.pop
      true
    end
  end

  # Command classes for communication.
  module Commands
    # Base command.
    class Command; end

    # Command to execute a task.
    class Execute < Command
      attr_reader :task_id, :block, :callback, :token

      def initialize(task_id, block, callback, token)
        @task_id = task_id
        @block = block
        @callback = callback
        @token = token
      end
    end

    # Command to shut down the worker.
    class Shutdown < Command; end
  end

  # The AsyncTaskProcessor schedules tasks using Async.
  # Each submitted task is given a CancellationToken, and cancellation is
  # signaled by pushing a cancellation message on the token's queue.
  class AsyncTaskProcessor
    def initialize(logger = Logger.new($stdout))
      @logger = logger
      @command_queue = Queue.new
      @processing_thread = nil
    end

    # Start the processing thread.
    def start
      return if @processing_thread&.alive?
      @logger.debug("Starting async task processor")
      @processing_thread = Thread.new { process_commands }
    end

    # Stop the processing thread.
    def stop
      return unless @processing_thread&.alive?
      @logger.debug("Stopping async task processor")
      @command_queue.push(Commands::Shutdown.new)
    end

    # Submit a task for execution.
    # The task block is given a CancellationToken.
    def submit(task_id, callback, &task_block)
      token = CancellationToken.new
      @command_queue.push(Commands::Execute.new(task_id, task_block, callback, token))
      token
    end

    private

    def process_commands
      Async do
        barrier = Async::Barrier.new
        loop do
          command = @command_queue.pop

          case command
          when Commands::Execute
            handle_execute(command, barrier)
          when Commands::Shutdown
            @logger.debug("Received shutdown command")
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

    # Run the task block along with a cancellation watcher.
    def handle_execute(command, barrier)
      task_id = command.task_id
      task_block = command.block
      callback = command.callback
      token = command.token

      @logger.info("Executing task #{task_id}")

      # Store a reference to the task so we can stop it when cancellation is requested
      worker_task = nil

      barrier.async do
        # Spawn a cancellation watcher that polls the token.
        cancel_task = Async do |t|
          t.annotate("Cancellation watcher for task #{task_id}")
          token.wait # Blocks until cancellation is signaled.
          @logger.debug("Cancellation received for task #{task_id}")

          # When cancellation is requested, stop the worker task
          if worker_task&.running?
            worker_task.stop

            # Properly handle task cleanup after stopping
            begin
              worker_task.wait
            rescue => e
              # This is expected when a task is stopped
              callback.call(false, e)
            end
          end
        end

        # Spawn the worker task that runs the user-supplied block.
        worker_task = Async do |t|
          t.annotate("Worker for task #{task_id}")

          begin
            # Execute the actual work
            task_result = task_block.call
            @logger.debug("Task #{task_id} completed successfully")
            callback.call(true, task_result)
          rescue => e
            @logger.error("Error executing task #{task_id}: #{e.message}")
            callback.call(false, e)
          ensure
            cancel_task.stop if cancel_task&.running?
            cancel_task&.wait
          end
        end
      end
    end
  end
end
