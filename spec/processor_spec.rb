# frozen_string_literal: true

require "prosody"
require "async"
require "logger"
require "opentelemetry-api"
require "opentelemetry/sdk"

# Tests for the CancellationToken class which provides a mechanism
# for canceling in-flight asynchronous tasks
RSpec.describe Prosody::CancellationToken do
  subject(:token) { described_class.new }

  # Verify that cancellation properly signals waiting threads
  describe "#cancel and #wait" do
    it "unblocks wait and returns true" do
      signal_queue = Queue.new
      Thread.new {
        token.wait
        signal_queue.push(true)
      }
      token.cancel
      expect(signal_queue.pop).to be true
    end
  end
end

# Tests for the Execute command used to schedule task execution
# in the AsyncTaskProcessor
RSpec.describe Prosody::Commands::Execute do
  # Test inputs
  let(:task_id) { "task-123" }
  let(:carrier) { {"trace-id" => "abc"} }
  let(:block) { proc { 42 } }
  let(:callback) { proc {} }
  let(:token) { Prosody::CancellationToken.new }

  let(:event_context) { {event_type: "message", topic: "t"} }

  subject(:command) { described_class.new(task_id, carrier, event_context, block, callback, token) }

  # Verify that all command attributes are accessible
  describe "attributes" do
    it "exposes task_id, carrier, event_context, block, callback, and token" do
      expect(command.task_id).to eq(task_id)
      expect(command.carrier).to eq(carrier)
      expect(command.event_context).to eq(event_context)
      expect(command.block).to eq(block)
      expect(command.callback).to eq(callback)
      expect(command.token).to be(token)
    end
  end
end

# Tests for the Shutdown command used to gracefully stop
# the AsyncTaskProcessor
RSpec.describe Prosody::Commands::Shutdown do
  subject(:shutdown) { described_class.new }

  # Verify that Shutdown is a Command subclass
  it "is a Command" do
    expect(shutdown).to be_a(Prosody::Commands::Command)
  end
end

# Tests for the AsyncTaskProcessor which manages asynchronous
# task execution with OpenTelemetry context propagation
RSpec.describe Prosody::AsyncTaskProcessor do
  # Mock logger to avoid actual logging during tests
  let(:logger) { instance_double(Logger) }

  before do
    allow(logger).to receive(:debug)
    allow(logger).to receive(:warn)
    allow(logger).to receive(:error)
  end

  subject(:processor) { described_class.new(logger) }

  # Tests for the start method which creates the worker thread
  describe "#start" do
    context "when not already running" do
      it "starts a new thread and logs debug" do
        fake_thread = instance_double(Thread, alive?: false)
        expect(logger).to receive(:debug).with("Starting async task processor")
        allow(Thread).to receive(:new).and_return(fake_thread)
        processor.start
        expect(processor.instance_variable_get(:@processing_thread)).to eq(fake_thread)
      end

      it "does nothing if thread is alive" do
        alive_thread = instance_double(Thread, alive?: true)
        processor.instance_variable_set(:@processing_thread, alive_thread)
        expect(Thread).not_to receive(:new)
        processor.start
      end
    end
  end

  # Tests for the stop method which gracefully shuts down the processor
  describe "#stop" do
    let(:shutdown_cmd_class) { Prosody::Commands::Shutdown }

    context "when running" do
      it "pushes a Shutdown command and logs debug" do
        queue = processor.instance_variable_get(:@command_queue)
        alive_thread = instance_double(Thread, alive?: true)
        processor.instance_variable_set(:@processing_thread, alive_thread)
        expect(logger).to receive(:debug).with("Stopping async task processor")
        processor.stop
        expect(queue.pop(true)).to be_a(shutdown_cmd_class)
      end
    end

    context "when already stopped" do
      it "does nothing" do
        dead_thread = instance_double(Thread, alive?: false)
        processor.instance_variable_set(:@processing_thread, dead_thread)
        expect { processor.stop }.not_to raise_error
      end
    end
  end

  # Tests for the submit method which schedules tasks for execution
  describe "#submit" do
    let(:task_id) { "my-task" }
    let(:carrier) { {"foo" => "bar"} }
    let(:callback) { proc {} }

    it "returns a CancellationToken and enqueues Execute command" do
      queue = processor.instance_variable_get(:@command_queue)
      event_context = {"event_type" => "message", "topic" => "t"}
      token = processor.submit(task_id, carrier, event_context, callback) { :result }
      expect(token).to be_a(Prosody::CancellationToken)
      cmd = queue.pop(true)
      expect(cmd).to be_a(Prosody::Commands::Execute)
      expect(cmd.task_id).to eq(task_id)
      expect(cmd.carrier).to eq(carrier)
      expect(cmd.event_context).to eq(event_context.transform_keys(&:to_sym))
      expect(cmd.callback).to eq(callback)
      expect(cmd.token).to eq(token)
      expect(cmd.block.call).to eq(:result)
    end
  end

  # Integration tests for the full lifecycle of tasks in the processor
  context "integration: concurrent execution and cancellation" do
    # Track tokens to ensure proper cleanup
    let(:tokens) { [] }

    before { processor.start }
    after do
      tokens.each(&:cancel)
      processor.stop
      processor.instance_variable_get(:@processing_thread).join
    end

    # Verify successful task execution with result callback
    it "executes tasks and invokes callback with result" do
      results_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }
      token = processor.submit("t1", {}, {}, callback) { "hello" }
      tokens << token
      success, result = results_queue.pop
      expect(success).to be true
      expect(result).to eq("hello")
    end

    # Verify that tasks can be cancelled during execution
    it "cancels in-flight tasks and invokes callback with cancellation error" do
      results_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }
      token = processor.submit("t2", {}, {}, callback) do
        sleep 0.5
        "never"
      end
      tokens << token
      token.cancel
      success, result = results_queue.pop
      expect(success).to be false
      expect(result).to be_a(RuntimeError)
      expect(result.message).to eq("Task cancelled")
    end

    # Verify that Async::Stop is raised in user code on cancellation
    it "raises Async::Stop in user code when cancelled" do
      results_queue = Queue.new
      exception_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }

      token = processor.submit("t3", {}, {}, callback) do
        sleep 0.5
        "never"
      rescue Async::Stop => e
        exception_queue.push(e)
        raise  # Re-raise to let wrapper handle it
      end
      tokens << token
      token.cancel

      # Verify user code received Async::Stop
      exception = exception_queue.pop
      expect(exception).to be_a(Async::Stop)

      # Verify cancellation was reported
      success, result = results_queue.pop
      expect(success).to be false
      expect(result.message).to eq("Task cancelled")
    end

    # Verify that ensure blocks run when task is cancelled
    it "runs ensure blocks when task is cancelled" do
      results_queue = Queue.new
      ensure_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }

      token = processor.submit("t4", {}, {}, callback) do
        sleep 0.5
        "never"
      ensure
        ensure_queue.push(:cleanup_ran)
      end
      tokens << token
      token.cancel

      # Verify ensure block ran
      cleanup = ensure_queue.pop
      expect(cleanup).to eq(:cleanup_ran)

      # Verify cancellation was reported
      success, _result = results_queue.pop
      expect(success).to be false
    end

    # Verify user can catch Async::Stop and return a value instead
    it "allows user to catch Async::Stop and return a value" do
      results_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }

      token = processor.submit("t5", {}, {}, callback) do
        sleep 0.5
        "never"
      rescue Async::Stop
        :gracefully_handled  # Don't re-raise
      end
      tokens << token
      token.cancel

      # User caught and returned value, so it's reported as success
      success, result = results_queue.pop
      expect(success).to be true
      expect(result).to eq(:gracefully_handled)
    end
  end

  # Tests for exception handling during task execution
  context "task exception handling" do
    # Prevent cancellation in these tests by overriding the wait method
    before do
      allow(Prosody::CancellationToken).to receive(:new).and_wrap_original do |orig, *args|
        tok = orig.call(*args)

        def tok.wait
          raise "skip cancel"
        end

        tok
      end
      processor.start
    end

    after do
      processor.stop
      processor.instance_variable_get(:@processing_thread).join
    end

    # Verify that exceptions in tasks are properly handled and reported
    it "invokes callback with false and exception, and logs error" do
      results_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }
      processor.submit("e3", {}, {}, callback) { raise StandardError, "boom" }
      success, result = results_queue.pop
      expect(success).to be false
      expect(result).to be_a(StandardError)
      expect(result.message).to eq("boom")
      expect(logger).to have_received(:error).with("Error executing task e3: boom")
    end
  end

  # Tests for OpenTelemetry context propagation into the worker fiber
  context "span context propagation" do
    let(:span_exporter) { OpenTelemetry::SDK::Trace::Export::InMemorySpanExporter.new }
    let(:tracer) { OpenTelemetry.tracer_provider.tracer("test") }

    before do
      OpenTelemetry::SDK.configure do |c|
        c.add_span_processor(
          OpenTelemetry::SDK::Trace::Export::SimpleSpanProcessor.new(span_exporter)
        )
      end
      processor.start
    end

    after do
      processor.stop
      processor.instance_variable_get(:@processing_thread).join
      OpenTelemetry.tracer_provider = OpenTelemetry::Internal::ProxyTracerProvider.new
    end

    it "makes async_dispatch the parent of spans created inside the task block" do
      results_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }

      processor.submit("span-test", {}, {}, callback) do
        tracer.in_span("child-span") {}
      end

      results_queue.pop

      finished = span_exporter.finished_spans
      dispatch = finished.find { |s| s.name == "async_dispatch" }
      child = finished.find { |s| s.name == "child-span" }

      expect(dispatch).not_to be_nil
      expect(child).not_to be_nil
      expect(child.parent_span_id).to eq(dispatch.span_id)
    end
  end

  # Tests for graceful shutdown behavior, ensuring tasks complete
  context "graceful shutdown waits for tasks" do
    # Prevent cancellation to test task completion during shutdown
    before do
      allow(Prosody::CancellationToken).to receive(:new).and_wrap_original do |orig, *args|
        tok = orig.call(*args)

        def tok.wait
          raise "skip cancel"
        end

        tok
      end
      processor.start
    end

    # Verify that shutdown waits for in-flight tasks to complete
    it "waits for in-flight tasks to complete before stopping" do
      results_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }
      processor.submit("g1", {}, {}, callback) { "done1" }
      processor.submit("g2", {}, {}, callback) { "done2" }
      processor.stop
      processor.instance_variable_get(:@processing_thread).join
      results = [results_queue.pop, results_queue.pop]
      expect(results).to include([true, "done1"], [true, "done2"])
    end
  end
end
