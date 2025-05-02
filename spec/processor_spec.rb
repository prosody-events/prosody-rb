require "prosody"
require "async"
require "logger"
require "opentelemetry-api"

RSpec.describe Prosody::CancellationToken do
  subject(:token) { described_class.new }

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

RSpec.describe Prosody::Commands::Execute do
  let(:task_id) { "task-123" }
  let(:carrier) { {"trace-id" => "abc"} }
  let(:block) { proc { 42 } }
  let(:callback) { proc {} }
  let(:token) { Prosody::CancellationToken.new }

  subject(:command) { described_class.new(task_id, carrier, block, callback, token) }

  describe "attributes" do
    it "exposes task_id, carrier, block, callback, and token" do
      expect(command.task_id).to eq(task_id)
      expect(command.carrier).to eq(carrier)
      expect(command.block).to eq(block)
      expect(command.callback).to eq(callback)
      expect(command.token).to be(token)
    end
  end
end

RSpec.describe Prosody::Commands::Shutdown do
  subject(:shutdown) { described_class.new }

  it "is a Command" do
    expect(shutdown).to be_a(Prosody::Commands::Command)
  end
end

RSpec.describe Prosody::AsyncTaskProcessor do
  let(:logger) { instance_double(Logger) }

  before do
    allow(logger).to receive(:debug)
    allow(logger).to receive(:warn)
    allow(logger).to receive(:error)
  end

  subject(:processor) { described_class.new(logger) }

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

  describe "#submit" do
    let(:task_id) { "my-task" }
    let(:carrier) { {"foo" => "bar"} }
    let(:callback) { proc {} }

    it "returns a CancellationToken and enqueues Execute command" do
      queue = processor.instance_variable_get(:@command_queue)
      token = processor.submit(task_id, carrier, callback) { :result }
      expect(token).to be_a(Prosody::CancellationToken)
      cmd = queue.pop(true)
      expect(cmd).to be_a(Prosody::Commands::Execute)
      expect(cmd.task_id).to eq(task_id)
      expect(cmd.carrier).to eq(carrier)
      expect(cmd.callback).to eq(callback)
      expect(cmd.token).to eq(token)
      expect(cmd.block.call).to eq(:result)
    end
  end

  context "integration: concurrent execution and cancellation" do
    let(:tokens) { [] }

    before { processor.start }
    after do
      tokens.each(&:cancel)
      processor.stop
      processor.instance_variable_get(:@processing_thread).join
    end

    it "executes tasks and invokes callback with result" do
      results_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }
      token = processor.submit("t1", {}, callback) { "hello" }
      tokens << token
      success, result = results_queue.pop
      expect(success).to be true
      expect(result).to eq("hello")
    end

    it "cancels in-flight tasks and invokes callback with cancellation error" do
      results_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }
      token = processor.submit("t2", {}, callback) do
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
  end

  context "task exception handling" do
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

    it "invokes callback with false and exception, and logs error" do
      results_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }
      processor.submit("e3", {}, callback) { raise StandardError, "boom" }
      success, result = results_queue.pop
      expect(success).to be false
      expect(result).to be_a(StandardError)
      expect(result.message).to eq("boom")
      expect(logger).to have_received(:error).with("Error executing task e3: boom")
    end
  end

  context "graceful shutdown waits for tasks" do
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

    it "waits for in-flight tasks to complete before stopping" do
      results_queue = Queue.new
      callback = proc { |success, result| results_queue.push([success, result]) }
      processor.submit("g1", {}, callback) { "done1" }
      processor.submit("g2", {}, callback) { "done2" }
      processor.stop
      processor.instance_variable_get(:@processing_thread).join
      results = [results_queue.pop, results_queue.pop]
      expect(results).to include([true, "done1"], [true, "done2"])
    end
  end
end
