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

  describe "point reads" do
    # Writes one item to each collection, so each one has a publication.
    class PublishingHandler < CompleteHandler
      def initialize(sink, definitions)
        @sink = sink
        @definitions = definitions
      end

      def on_message(context, _message)
        value, map, set, deque = @definitions.map { |definition| context.state(definition) }
        value.set(1)
        map.set("a", 1)
        set.add("a")
        deque.push(1)
        @sink.push([value, map, set, deque].map(&:commit))
      end
    end

    # One point read of each published reader method. The owner publishes one
    # collection of each kind under a fresh subsystem.
    def point_reads
      subsystem = "reads-#{SecureRandom.hex(4)}"
      definitions = {
        value: Prosody.value(random_state_name("val"), published: true, read_cache: false),
        map: Prosody.map(random_state_name("map"), published: true, read_cache: false),
        set: Prosody.set(random_state_name("set"), published: true, read_cache: false),
        deque: Prosody.deque(random_state_name("deq"), published: true, read_cache: false)
      }
      client = build_client(*definitions.values, subsystem: subsystem)
      client.subscribe(PublishingHandler.new(sink, definitions.values))
      client.send_message(topic, "k", {go: true})
      expect(sink.wait(1).first).to eq(%i[applied applied applied applied])

      value, map, set, deque = definitions.values.map { |definition| client.state(subsystem, definition) }
      {
        "value get" => -> { value.get("k") },
        "map get" => -> { map.get("k", "a") },
        "map get_many" => -> { map.get_many("k", %w[a]) },
        "map key?" => -> { map.key?("k", "a") },
        "map contains_many" => -> { map.contains_many("k", %w[a]) },
        "map empty?" => -> { map.empty?("k") },
        "set include?" => -> { set.include?("k", "a") },
        "set contains_many" => -> { set.contains_many("k", %w[a]) },
        "set empty?" => -> { set.empty?("k") },
        "deque get" => -> { deque.get("k", 0) },
        "deque length" => -> { deque.length("k") },
        "deque empty?" => -> { deque.empty?("k") },
        "deque first" => -> { deque.first("k") },
        "deque last" => -> { deque.last("k") },
        "map scan" => -> { map.each_pair("k").to_a }
      }
    end

    it "joins the caller's OpenTelemetry context" do
      point_reads.each do |name, read|
        injected = false
        allow(OpenTelemetry.propagation).to receive(:inject).and_wrap_original do |original, *args, **kwargs|
          injected = true
          original.call(*args, **kwargs)
        end
        read.call
        expect(injected).to be(true), "#{name} did not read the caller's context"
      end
    end

    it "raises in a forked child instead of waiting forever", :fork do
      reads = point_reads
      rd, wr = IO.pipe
      pid = Process.fork do
        rd.close
        failures = reads.filter_map do |name, read|
          read.call
          "#{name}: returned"
        rescue => e
          "#{name}: #{e.message}" unless e.message.include?("after fork")
        end
        wr.write(failures.empty? ? "refused" : failures.join("\n"))
      ensure
        wr.close
        exit!(0)
      end
      wr.close
      # The deadline is a hang guard: a read that waits on the dead runtime
      # never returns.
      finished = IO.select([rd], nil, nil, TestConfig::MESSAGE_TIMEOUT)
      Process.kill(:KILL, pid) unless finished
      result = finished ? rd.read : "a read did not return"
      rd.close
      Process.waitpid(pid)

      expect(result).to eq("refused")
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
