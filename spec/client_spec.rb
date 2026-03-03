# frozen_string_literal: true

require "spec_helper"
require "timeout"
require "securerandom"

# Integration tests for Prosody::Client, verifying end-to-end messaging functionality
# with a real Kafka instance. Tests cover initialization, subscription, message
# publishing/consumption, message ordering, shutdown, and error handling.
RSpec.describe Prosody::Client, integration: true do
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

    # Wait for events matching a specific condition
    # @param condition [Proc] Block that returns true for matching events
    # @param timeout [Numeric] Total timeout in seconds
    # @return [Array<Hash>] Matching events
    def wait_for_matching_events(condition, timeout)
      events = []
      deadline = Time.now + timeout

      while Time.now < deadline
        begin
          event = Timeout.timeout(0.1) { @queue.pop }
          events << event if event && condition.call(event)
        rescue Timeout::Error
          # Short timeout to check condition periodically
        end
      end

      events
    end

    # Clear any queued events
    # @return [void]
    def clear
      @queue.clear
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
  let(:config) { TestConfig.create_configuration(topic) }
  let(:client) { Prosody::Client.new(config) }
  let(:admin_client_class) { Prosody.const_get(:AdminClient) }
  let(:admin) { admin_client_class.new([TestConfig::BOOTSTRAP_SERVERS]) }

  # Test setup: create the topic before each test
  before do
    admin.create_topic(topic, 4, 1)

    # Add a small delay to ensure topic creation propagates
    sleep 1
  end

  # Test cleanup: unsubscribe client and delete topic
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

  # Verify client initialization works correctly
  it "initializes correctly" do
    tracer.in_span("test.initialize") do |span|
      expect(client).to be_a(Prosody::Client)
    end
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

  # Verify end-to-end message delivery functionality
  it "sends and receives a message" do
    tracer.in_span("test.send_receive") do |span|
      # Create handler class that forwards messages to our stream
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
      received_messages = message_stream.wait_for_messages(1, TestConfig::MESSAGE_TIMEOUT)
      received_message = received_messages.first

      # Verify the message
      expect(received_message).not_to be_nil
      expect(received_message.topic).to eq(topic)
      expect(received_message.key).to eq(test_message[:key])
      # Compare with string keys since JSON serializes to string keys
      expect(received_message.payload).to eq(test_message[:payload].transform_keys(&:to_s))
    end
  end

  # Verify correct handling of multiple messages with ordering guarantees
  it "handles multiple messages with correct ordering" do
    tracer.in_span("test.multiple_messages") do |span|
      # Create handler class that forwards messages to our stream
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
      received_messages = message_stream.wait_for_messages(messages_to_send.length, TestConfig::MESSAGE_TIMEOUT)

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

  # Verify transient errors are retried properly
  it "handles transient errors with retry" do
    tracer.in_span("test.transient_error") do |span|
      message_count = [0] # Use array to share state
      retry_event = EventNotifier.new

      # Create a handler that fails on first attempt but succeeds on retry
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
      retry_event.once("retry", TestConfig::MESSAGE_TIMEOUT)

      # Expect message_count to be greater than 1 (initial + retry)
      expect(message_count[0]).to be > 1
    end
  end

  # Verify permanent errors are not retried
  it "handles permanent errors without retry" do
    tracer.in_span("test.permanent_error") do |span|
      message_count = [0] # Use array to share state
      error_event = EventNotifier.new

      # Create a handler that permanently fails
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
      error_event.once("error-event", TestConfig::MESSAGE_TIMEOUT)

      # Wait a bit to ensure no retries happen
      sleep 2

      # Expect message_count to be exactly 1 (no retries)
      expect(message_count[0]).to eq(1)
    end
  end

  # Helper methods for timer operations
  def create_timer_test_handler(event_stream)
    Class.new do
      def initialize(event_stream)
        @event_stream = event_stream
      end

      def on_message(context, message)
        case message.payload["action"]
        when "test_schedule_multiple"
          test_schedule_multiple_timers(context)
        when "test_unschedule"
          test_unschedule_specific_timer(context)
        when "test_clear_and_schedule"
          test_clear_and_schedule_operation(context)
        when "test_clear_scheduled"
          test_clear_all_scheduled_timers(context)
        when "test_scheduled_empty"
          test_scheduled_when_empty(context)
        end
      end

      def on_timer(context, timer)
        # Timer fired - not used in these tests
      end

      private

      def test_schedule_multiple_timers(context)
        times = create_future_times([30, 60, 90])
        schedule_times(context, times)

        scheduled_times = context.scheduled
        @event_stream.push({
          operation: :schedule_multiple,
          scheduled_count: scheduled_times.length,
          times_are_time_objects: scheduled_times.all? { |t| t.is_a?(Time) },
          original_times: times
        })
      end

      def test_unschedule_specific_timer(context)
        times = create_future_times([40, 80])
        schedule_times(context, times)

        before_count = context.scheduled.length
        context.unschedule(times.first)
        after_count = context.scheduled.length

        @event_stream.push({
          operation: :unschedule,
          before_count: before_count,
          after_count: after_count,
          unscheduled_time: times.first,
          remaining_time: times.last
        })
      end

      def test_clear_and_schedule_operation(context)
        initial_times = create_future_times([50, 100])
        new_time = create_future_times([150]).first

        schedule_times(context, initial_times)
        before_count = context.scheduled.length

        context.clear_and_schedule(new_time)
        after_scheduled = context.scheduled

        @event_stream.push({
          operation: :clear_and_schedule,
          before_count: before_count,
          after_count: after_scheduled.length,
          new_time: new_time,
          time_matches: after_scheduled.any? { |t| (t.to_i - new_time.to_i).abs <= 1 }
        })
      end

      def test_clear_all_scheduled_timers(context)
        times = create_future_times([70, 140])
        schedule_times(context, times)

        before_count = context.scheduled.length
        context.clear_scheduled
        after_count = context.scheduled.length

        @event_stream.push({
          operation: :clear_scheduled,
          before_count: before_count,
          after_count: after_count,
          completely_cleared: after_count == 0
        })
      end

      def test_scheduled_when_empty(context)
        scheduled_times = context.scheduled

        @event_stream.push({
          operation: :scheduled_empty,
          scheduled_count: scheduled_times.length,
          is_array: scheduled_times.is_a?(Array),
          is_empty: scheduled_times.empty?
        })
      end

      def create_future_times(offsets)
        now = Time.now
        offsets.map { |offset| now + offset }
      end

      def schedule_times(context, times)
        times.each { |time| context.schedule(time) }
      end
    end
  end

  def send_timer_test_messages(client, topic)
    client.send_message(topic, "timer-ops-1", {action: "test_schedule_multiple"})
    client.send_message(topic, "timer-ops-2", {action: "test_unschedule"})
    client.send_message(topic, "timer-ops-3", {action: "test_clear_and_schedule"})
    client.send_message(topic, "timer-ops-4", {action: "test_clear_scheduled"})
    client.send_message(topic, "timer-ops-5", {action: "test_scheduled_empty"})
  end

  def verify_timer_operations(timer_operations_data)
    expect(timer_operations_data.length).to eq(5)

    verify_schedule_multiple_operation(timer_operations_data)
    verify_unschedule_operation(timer_operations_data)
    verify_clear_and_schedule_operation(timer_operations_data)
    verify_clear_scheduled_operation(timer_operations_data)
    verify_scheduled_empty_operation(timer_operations_data)
  end

  def verify_schedule_multiple_operation(data)
    schedule_data = data.find { |d| d[:operation] == :schedule_multiple }
    expect(schedule_data).not_to be_nil
    expect(schedule_data[:scheduled_count]).to eq(3)
    expect(schedule_data[:times_are_time_objects]).to eq(true)
  end

  def verify_unschedule_operation(data)
    unschedule_data = data.find { |d| d[:operation] == :unschedule }
    expect(unschedule_data).not_to be_nil
    expect(unschedule_data[:before_count]).to eq(2)
    expect(unschedule_data[:after_count]).to eq(1)
  end

  def verify_clear_and_schedule_operation(data)
    clear_and_schedule_data = data.find { |d| d[:operation] == :clear_and_schedule }
    expect(clear_and_schedule_data).not_to be_nil
    expect(clear_and_schedule_data[:before_count]).to eq(2)
    expect(clear_and_schedule_data[:after_count]).to eq(1)
    expect(clear_and_schedule_data[:time_matches]).to eq(true)
  end

  def verify_clear_scheduled_operation(data)
    clear_data = data.find { |d| d[:operation] == :clear_scheduled }
    expect(clear_data).not_to be_nil
    expect(clear_data[:before_count]).to eq(2)
    expect(clear_data[:after_count]).to eq(0)
    expect(clear_data[:completely_cleared]).to eq(true)
  end

  def verify_scheduled_empty_operation(data)
    empty_data = data.find { |d| d[:operation] == :scheduled_empty }
    expect(empty_data).not_to be_nil
    expect(empty_data[:scheduled_count]).to eq(0)
    expect(empty_data[:is_array]).to eq(true)
    expect(empty_data[:is_empty]).to eq(true)
  end

  # Test comprehensive timer functionality
  it "provides comprehensive timer scheduling operations" do
    timer_stream = TimerEventStream.new
    handler = create_timer_test_handler(timer_stream)

    client.subscribe(handler.new(timer_stream))
    send_timer_test_messages(client, topic)

    timer_operations_data = timer_stream.wait_for_events(5, TestConfig::MESSAGE_TIMEOUT)
    verify_timer_operations(timer_operations_data)
  end

  def create_timer_firing_test_handler(event_stream)
    Class.new do
      def initialize(event_stream)
        @event_stream = event_stream
      end

      def on_message(context, message)
        if message.payload["action"] == "test_timer_firing"
          schedule_timers_for_firing_test(context)
        end
      end

      def on_timer(context, timer)
        capture_timer_firing_event(timer)
      end

      private

      def schedule_timers_for_firing_test(context)
        now = Time.now
        timer1_time = now + 1
        timer2_time = now + 2

        context.schedule(timer1_time)
        context.schedule(timer2_time)

        @event_stream.push({
          type: :scheduled,
          scheduled_at: now,
          timer1_target: timer1_time,
          timer2_target: timer2_time,
          scheduled_count: context.scheduled.length
        })
      end

      def capture_timer_firing_event(timer)
        fired_at = Time.now
        @event_stream.push({
          type: :timer_fired,
          key: timer.key,
          timer_time: timer.time,
          actual_fire_time: fired_at,
          timer_time_class: timer.time.class.name
        })
      end
    end
  end

  def verify_timer_scheduling_event(scheduling_events)
    expect(scheduling_events.length).to eq(1)

    scheduled_event = scheduling_events.first
    expect(scheduled_event[:type]).to eq(:scheduled)
    expect(scheduled_event[:scheduled_count]).to eq(2)

    scheduled_event
  end

  def verify_timer_firing_events(fired_events, scheduled_event)
    expect(fired_events.length).to eq(2)

    fired_events.each do |event|
      verify_timer_properties(event)
      verify_timer_accuracy(event, scheduled_event)
    end

    verify_timer_firing_order(fired_events)
  end

  def verify_timer_properties(event)
    expect(event[:type]).to eq(:timer_fired)
    expect(event[:key]).to eq("timer-fire-test")
    expect(event[:timer_time_class]).to eq("Time")
    expect(event[:timer_time]).to be_a(Time)
  end

  def verify_timer_accuracy(event, scheduled_event)
    expected_target = if event[:timer_time].to_i == scheduled_event[:timer1_target].to_i
      scheduled_event[:timer1_target]
    else
      scheduled_event[:timer2_target]
    end

    expect(event[:actual_fire_time]).to be_within(1).of(expected_target)
    expect(event[:timer_time].to_i).to be_within(1).of(expected_target.to_i)
  end

  def verify_timer_firing_order(fired_events)
    first_timer = fired_events.min_by { |e| e[:actual_fire_time] }
    second_timer = fired_events.max_by { |e| e[:actual_fire_time] }

    expect(first_timer[:actual_fire_time]).to be < second_timer[:actual_fire_time]
  end

  # Test that timers actually fire at the correct time
  it "fires timers at the correct time with proper Ruby Time objects" do
    timer_stream = TimerEventStream.new
    handler = create_timer_firing_test_handler(timer_stream)

    client.subscribe(handler.new(timer_stream))
    client.send_message(topic, "timer-fire-test", {action: "test_timer_firing"})

    scheduling_events = timer_stream.wait_for_events(1, TestConfig::MESSAGE_TIMEOUT)
    scheduled_event = verify_timer_scheduling_event(scheduling_events)

    fired_events = timer_stream.wait_for_events(2, 5)
    verify_timer_firing_events(fired_events, scheduled_event)
  end
end
