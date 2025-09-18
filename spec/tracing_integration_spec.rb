# frozen_string_literal: true

require "spec_helper"
require "securerandom"
require "opentelemetry/sdk"
require "opentelemetry-exporter-otlp"

# Integration test for OpenTelemetry tracing across the entire message processing pipeline
RSpec.describe "OpenTelemetry Integration", :integration, :tracing do
  let(:topic) { "tracing-test-#{Time.now.to_i}-#{SecureRandom.hex(4)}" }
  let(:config) { TestConfig.create_configuration(topic) }
  let(:client) { Prosody::Client.new(config) }
  let(:admin_client_class) { Prosody.const_get(:AdminClient) }
  let(:admin) { admin_client_class.new([TestConfig::BOOTSTRAP_SERVERS]) }

  # Test handler that creates spans and schedules timers
  class TracingHandler < Prosody::EventHandler
    attr_reader :message_received_latch, :timer_fired_latch, :tracer, :logger

    def initialize
      @message_received_latch = Thread::Queue.new
      @timer_fired_latch = Thread::Queue.new
      @tracer = OpenTelemetry.tracer_provider.tracer("prosody-ruby-test")
      @logger = Logger.new($stdout)
      @logger.level = Logger::DEBUG
    end

    def message_received?
      !@message_received_latch.empty?
    end

    def timer_fired?
      !@timer_fired_latch.empty?
    end

    def on_message(context, message)
      @logger.info "📨 Handler: on_message called with topic=#{message.topic}, key=#{message.key}"
      @tracer.in_span("test-message-handler", kind: :consumer) do |span|
        span.set_attribute("message.topic", message.topic)
        span.set_attribute("message.key", message.key)
        span.set_attribute("test.phase", "message_received")

        # Schedule a timer to fire in 2 seconds
        timer_time = Time.now + 2
        @logger.info "⏰ Handler: Scheduling timer for #{timer_time}"

        context.schedule(timer_time)

        span.add_event("timer_scheduled", attributes: { "timer.time" => timer_time.to_f })

        # Signal that message was received
        @message_received_latch << :received
        @logger.debug "✅ Handler: Message received latch signaled"
      end
    end

    def on_timer(context, timer)
      @logger.info "⏲️  Handler: on_timer called with key=#{timer.key}, time=#{timer.time}"
      @tracer.in_span("test-timer-handler", kind: :internal) do |span|
        span.set_attribute("timer.key", timer.key)
        span.set_attribute("timer.time", timer.time.to_f)
        span.set_attribute("test.phase", "timer_fired")

        span.add_event("timer_execution_complete")

        # Signal that timer was fired
        @timer_fired_latch << :fired
        @logger.debug "✅ Handler: Timer fired latch signaled"
      end
    end
  end

  before(:all) do
    # Set service name via environment variable for consistency with Rust
    ENV["OTEL_SERVICE_NAME"] = "prosody-ruby-tracing-test"
    ENV["OTEL_SERVICE_VERSION"] = Prosody::VERSION

    # Set OTLP endpoint if not already set
    ENV["OTEL_EXPORTER_OTLP_ENDPOINT"] ||= "http://localhost:4318"

    # Configure OpenTelemetry SDK with both OTLP and console exporters
    OpenTelemetry::SDK.configure do |c|
      c.service_name = ENV["OTEL_SERVICE_NAME"]
      c.service_version = ENV["OTEL_SERVICE_VERSION"]
      c.resource = OpenTelemetry::SDK::Resources::Resource.create({
        "service.name" => ENV["OTEL_SERVICE_NAME"],
        "service.version" => ENV["OTEL_SERVICE_VERSION"],
        "deployment.environment" => "test",
        "test.suite" => "tracing_integration"
      })

      # Add OTLP exporter
      c.add_span_processor(
        OpenTelemetry::SDK::Trace::Export::BatchSpanProcessor.new(
          OpenTelemetry::Exporter::OTLP::Exporter.new
        )
      )

#       # Add console exporter for debugging
#       c.add_span_processor(
#         OpenTelemetry::SDK::Trace::Export::SimpleSpanProcessor.new(
#           OpenTelemetry::SDK::Trace::Export::ConsoleSpanExporter.new
#         )
#       )
    end
  end

  before do
    # Create the topic
    admin.create_topic(topic, 1, 1)
  end

  after do
    begin
      # Properly shutdown the OpenTelemetry SDK to flush all spans
      logger = Logger.new($stdout)
      logger.info "Shutting down OpenTelemetry SDK to flush spans..."
      # Sleep to allow natural span flushing
      sleep 5
      logger.info "Unsubscribing"
      client.unsubscribe if client.consumer_state == :running

      logger.info "Deleting topic"
      admin.delete_topic(topic)
    rescue
      # Ignore errors during cleanup
    end
  end

  it "creates a complete distributed trace across message sending, receiving, and timer execution" do
    handler = TracingHandler.new
    tracer = OpenTelemetry.tracer_provider.tracer("prosody-ruby-test")
    logger = Logger.new($stdout)
    logger.level = Logger::INFO

    # Start the root span for the entire test
    tracer.in_span("test-root-span", kind: :client) do |root_span|
      root_span.set_attribute("test.name", "tracing_integration")
      root_span.set_attribute("test.topic", topic)
      root_span.add_event("test_started")

      # Subscribe to messages
      client.subscribe(handler)

      # Send a message within a span
      tracer.in_span("test-message-send", kind: :producer) do |send_span|
        send_span.set_attribute("message.topic", topic)
        send_span.set_attribute("message.key", "test-key-123")
        send_span.add_event("sending_message")

        # Debug: Log the current span context details
        logger.info "Send span context - trace_id: #{send_span.context.trace_id&.unpack('H*')&.first}, span_id: #{send_span.context.span_id&.unpack('H*')&.first}"

        client.send_message(topic, "test-key-123", {
          test: "tracing_integration",
          timestamp: Time.now.to_f,
          message: "Hello from tracing test!"
        })

        send_span.add_event("message_sent")
      end

      # Wait for message to be received with proper synchronization
      tracer.in_span("test-wait-for-message", kind: :internal) do |wait_span|
        wait_span.set_attribute("test.phase", "waiting_for_message")
        logger.info "Waiting for message reception..."
        handler.message_received_latch.pop # Block until message is received

        wait_span.add_event("message_received_confirmed")
        root_span.add_event("message_received_by_handler")
        logger.info "Message reception confirmed"
      end

      # Wait for timer to fire with proper synchronization
      tracer.in_span("test-wait-for-timer", kind: :internal) do |wait_span|
        wait_span.set_attribute("test.phase", "waiting_for_timer")
        logger.info "Waiting for timer execution..."
        handler.timer_fired_latch.pop # Block until timer fires

        wait_span.add_event("timer_fired_confirmed")
        root_span.add_event("timer_fired_by_handler")
        logger.info "Timer execution confirmed"
      end

      root_span.add_event("test_completed")
    end

    # Verify both events occurred (latches should be empty after popping)
    expect(handler.message_received?).to be false # Queue should be empty after pop
    expect(handler.timer_fired?).to be false # Queue should be empty after pop
  end
end