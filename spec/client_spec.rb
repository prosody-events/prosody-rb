# frozen_string_literal: true

require "spec_helper"
require "timeout"
require "securerandom"

RSpec.describe Prosody::Client, integration: true do
  # Constants
  MESSAGE_TIMEOUT = 5
  GROUP_NAME = "test-group"
  SOURCE_NAME = "test-source"
  BOOTSTRAP_SERVERS = ENV.fetch("PROSODY_BOOTSTRAP_SERVERS", "localhost:9094")

  # Simple message stream class that collects and provides messages
  class MessageStream
    def initialize
      @queue = Queue.new
    end

    def push(message)
      @queue.push(message)
    end

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

  # Asynchronous event notification system
  class EventNotifier
    def initialize
      @listeners = {}
      @mutex = Mutex.new
    end

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

  # Thread-safe semaphore implementation
  class ThreadSafeSemaphore
    def initialize(permits = 1)
      @permits = permits
      @mutex = Mutex.new
      @condition = ConditionVariable.new
    end

    def acquire
      @mutex.synchronize do
        while @permits <= 0
          @condition.wait(@mutex)
        end
        @permits -= 1
        true
      end
    end

    def release
      @mutex.synchronize do
        @permits += 1
        @condition.signal
        true
      end
    end
  end

  # Helper methods
  def generate_topic_name
    "test-topic-#{Time.now.to_i}-#{SecureRandom.hex(4)}"
  end

  # Setup OpenTelemetry
  let(:tracer) { OpenTelemetry.tracer_provider.tracer("prosody-ruby-test") }

  # Test subjects
  let(:topic) { generate_topic_name }
  let(:message_stream) { MessageStream.new }
  let(:config) do
    Prosody::Configuration.new(
      bootstrap_servers: BOOTSTRAP_SERVERS,
      group_id: GROUP_NAME,
      source_system: SOURCE_NAME,
      subscribed_topics: topic,
      probe_port: false,
      mode: :pipeline
    )
  end
  let(:client) { Prosody::Client.new(config) }
  let(:admin_client_class) { Prosody.const_get(:AdminClient) }
  let(:admin) { admin_client_class.new([BOOTSTRAP_SERVERS]) }

  # Test lifecycle
  before do
    # Create topic before each test
    admin.create_topic(topic, 4, 1)

    # Add a small delay to ensure topic creation propagates
    sleep 1
  end

  after do
    # Clean up after each test - only unsubscribe if running
    if client.respond_to?(:consumer_state) && client.consumer_state == :running
      client.unsubscribe
    end

    begin
      admin.delete_topic(topic)
    rescue => e
      puts "Could not delete topic: #{e.message}"
    end
  end

  it "initializes correctly" do
    tracer.in_span("test.initialize") do |span|
      expect(client).to be_a(Prosody::Client)
    end
  end

  it "subscribes and unsubscribes" do
    tracer.in_span("test.subscribe_unsubscribe") do |span|
      # Create handler class
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(stream)
          @stream = stream
        end

        def on_message(_context, message)
          @stream.push(message)
        end
      end

      # Subscribe with a handler
      handler = handler_class.new(message_stream)
      client.subscribe(handler)

      # Verify subscription state
      expect(client.consumer_state).to eq(:running)

      # Unsubscribe
      client.unsubscribe

      # Verify unsubscribed state
      expect(client.consumer_state).to eq(:configured)
    end
  end

  it "sends and receives a message" do
    tracer.in_span("test.send_receive") do |span|
      # Create handler class
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(stream)
          @stream = stream
        end

        def on_message(_context, message)
          @stream.push(message)
        end
      end

      # Subscribe with handler
      handler = handler_class.new(message_stream)
      client.subscribe(handler)

      # Send a test message
      test_message = {
        key: "test-key",
        payload: {content: "Hello, Kafka!"}
      }

      client.send_message(topic, test_message[:key], test_message[:payload])

      # Wait for the message
      received_messages = message_stream.wait_for_messages(1, MESSAGE_TIMEOUT)
      received_message = received_messages.first

      # Verify the message
      expect(received_message).not_to be_nil
      expect(received_message.topic).to eq(topic)
      expect(received_message.key).to eq(test_message[:key])
      # Fix: Compare with string keys since JSON serializes to string keys
      expect(received_message.payload).to eq(test_message[:payload].transform_keys(&:to_s))
    end
  end

  it "handles multiple messages with correct ordering" do
    tracer.in_span("test.multiple_messages") do |span|
      # Create handler class
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(stream)
          @stream = stream
        end

        def on_message(_context, message)
          @stream.push(message)
        end
      end

      # Subscribe with handler
      handler = handler_class.new(message_stream)
      client.subscribe(handler)

      # Prepare messages to send
      messages_to_send = [
        {key: "key1", payload: {content: "Message 1", sequence: 1}},
        {key: "key2", payload: {content: "Message 2", sequence: 1}},
        {key: "key1", payload: {content: "Message 3", sequence: 2}},
        {key: "key3", payload: {content: "Message 4", sequence: 1}},
        {key: "key2", payload: {content: "Message 5", sequence: 2}}
      ]

      # Send all messages
      messages_to_send.each do |msg|
        client.send_message(topic, msg[:key], msg[:payload])
      end

      # Wait for all messages
      received_messages = message_stream.wait_for_messages(messages_to_send.length, MESSAGE_TIMEOUT)

      # Check message count
      expect(received_messages.length).to eq(messages_to_send.length)

      # Group messages by key for ordering checks
      received_messages_by_key = received_messages.group_by(&:key)

      # Expected keys
      expected_keys = messages_to_send.map { |msg| msg[:key] }.uniq

      # Verify we received messages for all sent keys
      expect(received_messages_by_key.keys).to match_array(expected_keys)

      # Check ordering within each key
      received_messages_by_key.each do |key, messages|
        sequences = messages.map { |msg| msg.payload["sequence"] }
        expect(sequences).to eq(sequences.sort)
      end

      # Verify topic on all messages
      received_messages.each do |msg|
        expect(msg.topic).to eq(topic)
      end
    end
  end

  # Test for clean shutdown using thread-safe components
  it "handles clean shutdown during message processing" do
    tracer.in_span("test.shutdown_during_processing") do |span|
      # Set up event notification
      events = EventNotifier.new
      processing_semaphore = ThreadSafeSemaphore.new(0) # Start locked (0 permits)

      # Handler that will emit an event when processing starts
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(events, semaphore)
          @events = events
          @semaphore = semaphore
        end

        def on_message(_context, message)
          # Signal that processing has started
          @events.emit("processing_started", message)

          # Wait for the semaphore to be released
          @semaphore.acquire

          # Simulate long-running work
          sleep 1

          # This may or may not execute depending on unsubscribe timing
          @events.emit("processing_completed", message)
        end
      end

      # Subscribe with handler
      handler = handler_class.new(events, processing_semaphore)
      client.subscribe(handler)

      # Send a message that will trigger processing
      client.send_message(topic, "test-key", {content: "Long running task"})

      # Wait for processing to start
      events.once("processing_started", MESSAGE_TIMEOUT)

      # Allow processing to continue
      processing_semaphore.release

      # Give a moment for processing to commence
      sleep 0.1

      # Unsubscribe should interrupt the processing
      client.unsubscribe

      # Verify the consumer is no longer running
      expect(client.consumer_state).to eq(:configured)
    end
  end

  it "handles transient errors with retry" do
    tracer.in_span("test.transient_error") do |span|
      message_count = [0] # Use array to share state
      retry_event = EventNotifier.new

      # Create a handler that will fail transiently on first attempt
      handler_class = Class.new(Prosody::EventHandler) do
        extend Prosody::ErrorClassification

        def initialize(retry_event, message_count)
          @retry_event = retry_event
          @message_count = message_count
        end

        # Make StandardError transient (will be retried)
        transient :on_message, StandardError

        def on_message(_context, message)
          @message_count[0] += 1

          if @message_count[0] == 1
            raise StandardError, "Transient error occurred"
          end

          @retry_event.emit("retry", message)
        end
      end

      # Subscribe
      handler = handler_class.new(retry_event, message_count)
      client.subscribe(handler)

      # Send message to trigger error
      client.send_message(topic, "test-key", {content: "Trigger transient error"})

      # Wait for retry to succeed
      retry_event.once("retry", MESSAGE_TIMEOUT)

      # Expect message_count to be greater than 1 (initial + retry)
      expect(message_count[0]).to be > 1
    end
  end

  it "handles permanent errors without retry" do
    tracer.in_span("test.permanent_error") do |span|
      message_count = [0] # Use array to share state
      error_event = EventNotifier.new

      # Create a handler that will fail permanently
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(error_event, message_count)
          @error_event = error_event
          @message_count = message_count
        end

        # Make StandardError permanent (will not be retried)
        permanent :on_message, StandardError

        def on_message(_context, message)
          @message_count[0] += 1
          @error_event.emit("error-event", message)

          raise StandardError, "Permanent error occurred"
        end
      end

      # Subscribe
      handler = handler_class.new(error_event, message_count)
      client.subscribe(handler)

      # Send message to trigger error
      client.send_message(topic, "test-key", {content: "Trigger permanent error"})

      # Wait for error to occur
      error_event.once("error-event", MESSAGE_TIMEOUT)

      # Wait a bit to ensure no retries happen
      sleep 2

      # Expect message_count to be exactly 1 (no retries)
      expect(message_count[0]).to eq(1)
    end
  end
end
