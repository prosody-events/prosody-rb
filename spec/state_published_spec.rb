# frozen_string_literal: true

require "spec_helper"

# Published-state readers against real Kafka and Cassandra: read errors and a
# client that only reads.
RSpec.describe "Prosody published state (integration)", integration: true do
  include_context "keyed state integration"

  describe "published reader errors" do
    it "raises RuntimeError from point reads and traversals alike" do
      subsystem = "errors-#{SecureRandom.hex(4)}"
      name = random_state_name("val")
      definition = Prosody.value(name, published: true, read_cache: false)
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, _message)
          context.state(@def).set({"v" => 1})
          @sink.push(:written)
        end
      end

      client = build_client(definition, subsystem: subsystem)
      client.subscribe(handler_class.new(sink, definition))
      client.send_message(topic, "k1", {go: true})
      expect(sink.wait(1).first).to eq(:written)

      # Each reader names the published value collection with another kind, so
      # every read fails with a descriptor identity mismatch.
      reads = {
        map: [Prosody.map(name, read_cache: false), ->(r) { r.get("k1", "a") }, ->(r) { r.each_key("k1", limit: 1).to_a }],
        set: [Prosody.set(name, read_cache: false), ->(r) { r.include?("k1", "a") }, ->(r) { r.each("k1").to_a }],
        deque: [Prosody.deque(name, read_cache: false), ->(r) { r.get("k1", 0) }, ->(r) { r.reverse_each("k1").to_a }]
      }
      reads.each do |kind, (reader_definition, point, traversal)|
        reader = client.state(subsystem, reader_definition)
        [point, traversal].each do |read|
          error = begin
            read.call(reader)
            nil
          rescue RuntimeError => e
            e
          end
          expect([kind, error.class, error&.message]).to match([kind, RuntimeError, /identity mismatch/])
        end
      end
    end
  end

  describe "reader-only client" do
    it "reads published state without subscribed topics" do
      subsystem = "reader-#{SecureRandom.hex(4)}"
      definition = Prosody.value(random_state_name("val"), published: true, read_cache: false)
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, _message)
          value = context.state(@def)
          value.set({"v" => 1})
          @sink.push(value.commit)
        end
      end

      owner = build_client(definition, subsystem: subsystem)
      owner.subscribe(handler_class.new(sink, definition))
      owner.send_message(topic, "k1", {go: true})
      expect(sink.wait(1).first).to eq(:applied)

      reader = track_client(Prosody::Client.new(
        bootstrap_servers: TestConfig::BOOTSTRAP_SERVERS,
        cassandra_nodes: TestConfig::CASSANDRA_NODES,
        group_id: "reader-#{SecureRandom.hex(4)}",
        source_system: TestConfig::SOURCE_NAME,
        subscribed_topics: [],
        probe_port: false
      ))
      expect(reader.state(subsystem, definition).get("k1")).to eq({"v" => 1})
    end
  end
end
