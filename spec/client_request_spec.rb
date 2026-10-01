# frozen_string_literal: true

require "spec_helper"

# Integration tests for Prosody::Client#request and #request_excise against a
# local handler that answers for the "inventory" subsystem.
RSpec.describe Prosody::Client, integration: true do
  include_context "client integration"

  it "returns the local handler response for a request" do
    handler_class = Class.new(CompleteHandler) do
      def on_message(_context, message)
        {"key" => message.key, "requested" => message.response_requested?}
      end
    end

    client.subscribe(handler_class.new)
    results = client.request(
      topic: topic,
      key: "order-1",
      payload: {"type" => "order.created"},
      subsystems: ["inventory"],
      timeout: TestConfig::MESSAGE_TIMEOUT
    )

    expect(results).to eq(
      "inventory" => Prosody::Success.new(value: {"key" => "order-1", "requested" => true})
    )
  end

  it "returns the local handler response for an excise request" do
    handler_class = Class.new(CompleteHandler) do
      def on_excise(_context, message)
        {"key" => message.key, "requested" => message.response_requested?}
      end
    end

    client.subscribe(handler_class.new)
    results = client.request_excise(
      topic: topic,
      key: "order-1",
      subsystems: ["inventory"],
      timeout: TestConfig::MESSAGE_TIMEOUT
    )

    expect(results).to eq(
      "inventory" => Prosody::Success.new(value: {"key" => "order-1", "requested" => true})
    )
  end

  # A result with no JSON form is a handler mistake, so it classifies as
  # transient. Best-effort mode does not retry, so the requester receives it.
  context "when a result cannot encode" do
    let(:config) { TestConfig.create_configuration(topic, subsystem: "inventory", mode: :best_effort) }

    it "returns a transient handler failure" do
      handler_class = Class.new(CompleteHandler) do
        def on_message(_context, _message)
          Object.new
        end
      end

      client.subscribe(handler_class.new)
      outcome = client.request(
        topic: topic,
        key: "order-1",
        payload: {"type" => "order.created"},
        subsystems: ["inventory"],
        timeout: TestConfig::MESSAGE_TIMEOUT
      ).fetch("inventory")

      expect(outcome).to be_a(Prosody::Failure)
      expect(outcome.error).to be_a(Prosody::HandlerError)
      expect(outcome.error.message).to start_with("transient error:")
    end
  end
end
