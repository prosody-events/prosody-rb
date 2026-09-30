# frozen_string_literal: true

require "securerandom"
require "timeout"

# Shared helpers for the Prosody::Client integration specs.
# Collects and provides messages in a thread-safe manner.
# Used to verify message receipt in tests.
class MessageStream
  def initialize
    @queue = Queue.new
  end

  # Add a message to the stream
  # @param message [Prosody::Message] Message to add
  # @return [void]
  def push(message)
    @queue.push(message)
  end

  # Retrieve a specific number of messages with timeout
  # @param count [Integer] Number of messages to retrieve
  # @param timeout [Numeric] How long to wait for each message in seconds
  # @return [Array<Prosody::Message>] Retrieved messages (may be fewer than requested)
  def wait_for_messages(count, timeout)
    messages = []

    count.times do
      # Use standard timeout for Queue
      message = Timeout.timeout(timeout) { @queue.pop }
      messages << message if message
    rescue Timeout::Error
      # Timeout occurred, just continue
    end

    messages
  end
end

# Collects and provides timer events in a thread-safe manner.
# Used to verify timer operations and firing in tests.
class TimerEventStream
  def initialize
    @queue = Queue.new
  end

  # Add a timer event to the stream
  # @param event [Hash] Timer event to add
  # @return [void]
  def push(event)
    @queue.push(event)
  end

  # Wait for a specific number of timer events with timeout
  # @param count [Integer] Number of events to retrieve
  # @param timeout [Numeric] How long to wait for each event in seconds
  # @return [Array<Hash>] Retrieved events (may be fewer than requested)
  def wait_for_events(count, timeout)
    events = []

    count.times do
      event = Timeout.timeout(timeout) { @queue.pop }
      events << event if event
    rescue Timeout::Error
      # Timeout occurred, just continue
    end

    events
  end
end

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

# Shared setup for the Prosody::Client integration specs: a fresh topic with
# four partitions, a client, and teardown that shuts the client down and
# deletes the topic.
RSpec.shared_context "client integration" do
  # Create a unique topic name for test isolation
  # @return [String] Generated unique topic name
  def generate_topic_name
    "test-topic-#{Time.now.to_i}-#{SecureRandom.hex(4)}"
  end

  # OpenTelemetry tracer for test spans
  let(:tracer) { OpenTelemetry.tracer_provider.tracer("prosody-ruby-test") }

  # Test variables
  let(:topic) { generate_topic_name }
  let(:message_stream) { MessageStream.new }
  let(:config) { TestConfig.create_configuration(topic, subsystem: "inventory") }
  let(:client) { Prosody::Client.new(config) }
  let(:admin_client_class) { Prosody.const_get(:AdminClient) }
  let(:admin) { admin_client_class.new([TestConfig::BOOTSTRAP_SERVERS]) }

  # Test setup: create the topic before each test
  before do
    admin.create_topic(topic, 4, 1)

    # Add a small delay to ensure topic creation propagates
    sleep 1
  end

  # Test cleanup: shut down the client and delete the topic.
  after do
    if client.respond_to?(:consumer_state) && client.consumer_state != :shut_down
      client.shutdown
    end

    begin
      admin.delete_topic(topic)
    rescue => e
      puts "Could not delete topic: #{e.message}"
    end
  end
end
