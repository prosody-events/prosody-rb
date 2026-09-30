# frozen_string_literal: true

require "spec_helper"

# Integration tests for Prosody::Client against a real Kafka instance. They cover
# initialization, subscription, message delivery and ordering, and shutdown.
RSpec.describe Prosody::Client, integration: true do
  include_context "client integration"

  # Verify client initialization works correctly
  it "initializes correctly" do
    tracer.in_span("test.initialize") do |span|
      expect(client).to be_a(Prosody::Client)
    end
  end

  # Verify the client raises a clear error when called from a forked child process
  it "raises when called after fork", :fork do
    # Force the lazy let to resolve in the parent before forking
    _ = client

    rd, wr = IO.pipe

    pid = Process.fork do
      rd.close
      begin
        client.consumer_state
        wr.write("ok")
      rescue RuntimeError => e
        wr.write("error:#{e.message}")
      rescue => e
        wr.write("unexpected:#{e.message}")
      ensure
        wr.close
        exit!(0)
      end
    end

    wr.close
    result = rd.read
    rd.close
    Process.waitpid(pid)

    expect(result).to start_with("error:"), "expected RuntimeError in child, got: #{result}"
    expect(result).to include("after fork")
  end

  # Verify source system identifier is accessible
  it "exposes source system identifier" do
    tracer.in_span("test.source_system") do |span|
      expect(client.source_system).to eq(TestConfig::SOURCE_NAME)
      expect(client.source_system).to be_a(String)
    end
  end

  # Verify basic subscription and unsubscription functionality
  it "subscribes and unsubscribes" do
    tracer.in_span("test.subscribe_unsubscribe") do |span|
      # Create handler class that pushes messages to our stream
      handler_class = Class.new(CompleteHandler) do
        def initialize(stream)
          @stream = stream
        end

        def on_message(_context, message)
          @stream.push(message)
        end
      end

      # Subscribe with a handler
      handler = handler_class.new(sink)
      client.subscribe(handler)

      # Verify subscription state
      expect(client.consumer_state).to eq(:running)
      expect(client.assigned_partition_count).to be_a(Integer)
      expect(client.stalled?).to be(false)

      # Unsubscribe
      client.unsubscribe

      # Verify unsubscribed state
      expect(client.consumer_state).to eq(:configured)
    end
  end

  # Verify end-to-end message delivery functionality
  it "sends and receives a message" do
    tracer.in_span("test.send_receive") do |span|
      # Create handler class that forwards messages to our stream
      handler_class = Class.new(CompleteHandler) do
        def initialize(stream)
          @stream = stream
        end

        def on_message(_context, message)
          @stream.push(message)
        end
      end

      # Subscribe with handler
      handler = handler_class.new(sink)
      client.subscribe(handler)

      # Send a test message
      test_message = {
        key: "test-key",
        payload: {content: "Hello, Kafka!"}
      }

      client.send_message(topic, test_message[:key], test_message[:payload])

      # Wait for the message
      received_messages = sink.wait(1)
      received_message = received_messages.first

      # Verify the message
      expect(received_message).not_to be_nil
      expect(received_message.topic).to eq(topic)
      expect(received_message.key).to eq(test_message[:key])
      expect(received_message.source_system).to eq(TestConfig::SOURCE_NAME)
      expect(received_message.response_requested?).to be(false)
      # Compare with string keys since JSON serializes to string keys
      expect(received_message.payload).to eq(test_message[:payload].transform_keys(&:to_s))
    end
  end

  it "sends and receives an excise record" do
    handler_class = Class.new(CompleteHandler) do
      def initialize(stream)
        @stream = stream
      end

      def on_excise(_context, message)
        @stream.push(message)
      end
    end

    client.subscribe(handler_class.new(sink))
    client.excise(topic, "obsolete-key")
    message = sink.wait(1).first

    expect(message.key).to eq("obsolete-key")
    expect(message).to be_a(Prosody::ExciseMessage)
    expect(message.source_system).to eq(TestConfig::SOURCE_NAME)
    expect(message.response_requested?).to be(false)
    expect(message).not_to respond_to(:payload)
  end

  # Verify correct handling of multiple messages with ordering guarantees
  it "handles multiple messages with correct ordering" do
    tracer.in_span("test.multiple_messages") do |span|
      # Create handler class that forwards messages to our stream
      handler_class = Class.new(CompleteHandler) do
        def initialize(stream)
          @stream = stream
        end

        def on_message(_context, message)
          @stream.push(message)
        end
      end

      # Subscribe with handler
      handler = handler_class.new(sink)
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
      received_messages = sink.wait(messages_to_send.length)

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

  # Verify client handles clean shutdown during active message processing
  it "handles clean shutdown during message processing" do
    tracer.in_span("test.shutdown_during_processing") do |span|
      # Set up event notification
      events = EventNotifier.new
      processing_semaphore = ThreadSafeSemaphore.new(0) # Start locked (0 permits)

      # Handler that signals when processing starts and waits on a semaphore
      handler_class = Class.new(CompleteHandler) do
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
      events.once("processing_started", TestConfig::MESSAGE_TIMEOUT)

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
end
