# frozen_string_literal: true

require "async"
require "async/barrier"
require "logger"
require "opentelemetry-api"
require "prosody/version"

module Prosody
  class CancellationToken
    def initialize
      @queue = Queue.new
    end

    def cancel
      @queue.push(:cancel)
    end

    def wait
      @queue.pop
      true
    end
  end

  module Commands
    class Command; end

    class Execute < Command
      attr_reader :task_id, :block, :callback, :token, :carrier

      def initialize(task_id, carrier, block, callback, token)
        @task_id = task_id
        @carrier = carrier
        @block = block
        @callback = callback
        @token = token
      end
    end

    class Shutdown < Command; end
  end

  class AsyncTaskProcessor
    def initialize(logger = Logger.new($stdout))
      @logger = logger
      @command_queue = Queue.new
      @processing_thread = nil
      @tracer = nil
    end

    # Instantiate @tracer in this thread so it never crosses thread boundaries
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

    def stop
      return unless @processing_thread&.alive?
      @logger.debug("Stopping async task processor")
      @command_queue.push(Commands::Shutdown.new)
    end

    # Now takes (task_id, carrier, callback, &task_block)
    def submit(task_id, carrier, callback, &task_block)
      token = CancellationToken.new
      @command_queue.push(
        Commands::Execute.new(task_id, carrier, task_block, callback, token)
      )
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

    def handle_execute(command, barrier)
      task_id = command.task_id
      carrier = command.carrier
      token = command.token
      callback = command.callback
      task_block = command.block

      # Extract parent context from the incoming carrier
      parent_ctx = OpenTelemetry.propagation.extract(
        carrier,
        getter: OpenTelemetry::Context::Propagation.text_map_getter
      )

      OpenTelemetry::Context.with_current(parent_ctx) do
        @tracer.in_span(
          "ruby-receive",
          kind: :consumer
        ) do |span, span_ctx|
          @logger.debug("Executing task #{task_id}")

          barrier.async do
            # Cancellation watcher
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

            # Actual worker
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
