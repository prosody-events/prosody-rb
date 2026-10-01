# frozen_string_literal: true

require "spec_helper"

# Message collections against real Kafka and Cassandra: stored messages
# round-trip through value, deque, and map collections.
RSpec.describe "Prosody message state (integration)", integration: true do
  include_context "keyed state integration"

  # Step 1 writes the live message to each collection. Step 2 reads each
  # collection and reports every message it returns as
  # [topic, key, payload, offset].
  class MessageRoundTripHandler < StateHandler
    def initialize(sink, rows)
      super(sink)
      @rows = rows
    end

    def on_message(context, message)
      if message.payload["step"] == 1
        @rows.each { |definition, (write, _)| write.call(context.state(definition), message) }
        @sink.push(message.offset)
      else
        reads = @rows.map { |definition, (_, read)| read.call(context.state(definition)).map { |item| summary(item) } }
        @sink.push([message.offset, reads])
      end
    end

    private

    def summary(item)
      item.is_a?(Prosody::Message) ? [item.topic, item.key, item.payload, item.offset] : item
    end
  end

  it "round-trips a stored message through value, deque, and map collections" do
    rows = {
      Prosody.message_value(random_state_name("mval")) => [
        ->(value, message) { value.set(message) },
        ->(value) { [value.get] }
      ],
      Prosody.message_deque(random_state_name("mdeq")) => [
        ->(deque, message) { deque.push(message) },
        ->(deque) { [deque.get(0), *deque.each.to_a] }
      ],
      Prosody.message_map(random_state_name("mmap")) => [
        ->(map, message) { %w[primary café].each { |key| map.set(key, message) } },
        ->(map) { [map.get("café"), *map.get_many(%w[primary absent]), *map.each_pair.to_a.flatten, *map.each_key.to_a] }
      ]
    }

    client = build_client(*rows.keys)
    client.subscribe(MessageRoundTripHandler.new(sink, rows))
    client.send_message(topic, "mk", {step: 1})
    stored = sink.wait(1).first
    client.send_message(topic, "mk", {step: 2})
    live, reads = sink.wait(1).first

    expect(live).not_to eq(stored)
    message = [topic, "mk", {"step" => 1}, stored]
    expect(reads).to eq([
      [message],
      [message, message],
      [message, message, nil, "café", message, "primary", message, "café", "primary"]
    ])
  end
end
