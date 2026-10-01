# frozen_string_literal: true

require "spec_helper"

# Integration tests for handler error classification: a transient error
# retries, and a permanent error does not.
RSpec.describe Prosody::Client, integration: true do
  include_context "client integration"

  # Verify transient errors are retried properly
  it "handles transient errors with retry" do
    tracer.in_span("test.transient_error") do |span|
      message_count = [0] # Use array to share state
      retry_event = EventNotifier.new

      # Create a handler that fails on first attempt but succeeds on retry
      handler_class = Class.new(CompleteHandler) do
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

      # Create a handler that permanently fails
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink, message_count)
          @sink = sink
          @message_count = message_count
        end

        # Make StandardError permanent (will not be retried)
        permanent :on_message, StandardError

        def on_message(_context, message)
          return @sink.push(:drained) if message.payload["drain"]

          @message_count[0] += 1
          raise StandardError, "Permanent error occurred"
        end
      end

      # Subscribe
      handler = handler_class.new(sink, message_count)
      client.subscribe(handler)

      # A later message on the same key runs only after every attempt of the
      # first message, so its arrival proves that no retry is pending.
      client.send_message(topic, "test-key", {content: "Trigger permanent error"})
      client.send_message(topic, "test-key", {drain: true})
      expect(sink.wait(1)).to eq([:drained])

      # Expect message_count to be exactly 1 (no retries)
      expect(message_count[0]).to eq(1)
    end
  end
end
