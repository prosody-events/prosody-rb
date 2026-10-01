# frozen_string_literal: true

# Shared helpers for the Prosody::Client integration specs.

# Thread-safe event notification system for coordinating test activities
# across threads. Allows tests to wait for specific events to occur.
class EventNotifier
  def initialize
    @listeners = {}
    @mutex = Mutex.new
  end

  # Wait for a single occurrence of the specified event
  # @param event_name [String, Symbol] Event to wait for
  # @param timeout [Numeric, nil] Maximum wait time in seconds (nil for indefinite)
  # @return [Object, nil] Event data or nil if timeout occurred
  def once(event_name, timeout = nil)
    queue = Queue.new

    # Register this queue as a listener
    @mutex.synchronize do
      @listeners[event_name] ||= []
      @listeners[event_name] << queue
    end

    # Wait for the event with timeout
    begin
      if timeout
        Timeout.timeout(timeout) { queue.pop }
      else
        queue.pop
      end
    rescue Timeout::Error
      nil
    end
  end

  # Trigger an event with optional data
  # @param event_name [String, Symbol] Event to trigger
  # @param args [Array] Data to pass to listeners
  # @return [void]
  def emit(event_name, *args)
    # Nothing to do if no listeners
    return unless @listeners[event_name]

    # Deliver the event to all registered listeners
    queues = nil
    @mutex.synchronize do
      queues = @listeners.delete(event_name) || []
    end

    queues.each do |queue|
      queue.push((args.size == 1) ? args.first : args)
    end
  end
end

# Thread-safe counting semaphore implementation for coordinating
# concurrent activities in tests.
class ThreadSafeSemaphore
  # @param permits [Integer] Initial number of permitted acquisitions
  def initialize(permits = 1)
    @permits = permits
    @mutex = Mutex.new
    @condition = ConditionVariable.new
  end

  # Wait until a permit is available, then acquire it
  # @return [Boolean] true if permit was acquired
  def acquire
    @mutex.synchronize do
      while @permits <= 0
        @condition.wait(@mutex)
      end
      @permits -= 1
      true
    end
  end

  # Release a permit, potentially unblocking a waiting thread
  # @return [Boolean] true if permit was released
  def release
    @mutex.synchronize do
      @permits += 1
      @condition.signal
      true
    end
  end
end

# Shared setup for the Prosody::Client integration specs: the keyed-state
# topic lifecycle and sink, a tracer, and one tracked client.
RSpec.shared_context "client integration" do
  include_context "keyed state integration"

  let(:tracer) { OpenTelemetry.tracer_provider.tracer("prosody-ruby-test") }
  let(:config) { TestConfig.create_configuration(topic, subsystem: "inventory") }
  let(:client) { track_client(Prosody::Client.new(config)) }
end
