# frozen_string_literal: true

require "spec_helper"

# End-to-end keyed-state scenarios against real Kafka and Cassandra. Handlers
# read and write state and push observations into a StateSink. Each test uses
# a fresh topic and fresh collection names.
RSpec.describe "Prosody keyed state (integration)", integration: true do
  include_context "keyed state integration"

  describe "value" do
    it "persists a value written in one event and read in the next" do
      definition = Prosody.value(random_state_name("val"))
      token = "tok-#{SecureRandom.hex(4)}"
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink, definition, token)
          @sink = sink
          @def = definition
          @token = token
        end

        def on_message(context, message)
          value = context.state(@def)
          case message.payload["step"]
          when 1
            value.set({"v" => @token})
            @sink.push({ev: "set"})
          when 2
            @sink.push({ev: "get", got: value.get})
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition, token))

      client.send_message(topic, "k1", {step: 1})
      expect(sink.wait(1).first).to eq({ev: "set"})

      client.send_message(topic, "k1", {step: 2})
      observation = sink.wait(1).first
      expect(observation[:ev]).to eq("get")
      expect(observation[:got]).to eq({"v" => token})
    end

    it "reads-its-own-writes, reports absence, and clears" do
      definition = Prosody.value(random_state_name("val"))
      rich = {
        "s" => "café 😀", "n" => 3.5, "b" => true,
        "arr" => [1, "x", nil], "nested" => {"z" => [true, 2]}
      }
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink, definition, rich)
          @sink = sink
          @def = definition
          @rich = rich
        end

        def on_message(context, _message)
          value = context.state(@def)
          before = value.get
          value.set(@rich)
          after = value.get
          value.clear
          cleared = value.get
          @sink.push({before: before, after: after, cleared: cleared})
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition, rich))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:before]).to be_nil
      expect(observation[:after]).to eq(rich)
      expect(observation[:cleared]).to be_nil
    end
  end

  describe "map" do
    it "sets, deletes, scans both directions in key order, and round-trips unicode" do
      definition = Prosody.map(random_state_name("map"))
      handler_class = Class.new(StateHandler) do
        def on_message(context, _message)
          map = context.state(@def)
          map.set("a", 1)
          map.set("b", 2)
          map.set("c", 3)
          map.delete("b")
          map.set("😀", 9)
          forward = []
          map.each_pair { |k, v| forward << [k, v] }
          reverse = []
          map.reverse_each_pair { |k, v| reverse << [k, v] }
          @sink.push({forward: forward, reverse: reverse, gone: map.get("b"), unicode: map.get("😀")})
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:forward]).to eq([["a", 1], ["c", 3], ["😀", 9]])
      expect(observation[:reverse]).to eq(observation[:forward].reverse)
      expect(observation[:gone]).to be_nil
      expect(observation[:unicode]).to eq(9)
    end
  end

  describe "map emptiness and batch presence" do
    it "reports emptiness and batch presence for owned and published maps" do
      subsystem = "presence-#{SecureRandom.hex(4)}"
      definition = Prosody.map(random_state_name("map"), published: true, read_cache: false)
      handler_class = Class.new(StateHandler) do
        def on_message(context, _message)
          map = context.state(@def)
          empty_before = map.empty?
          map.set("a", 1)
          map.set("b", false)
          empty_after = map.empty?
          present = map.contains_many(%w[a x b a])
          map.commit
          @sink.push({empty_before: empty_before, empty_after: empty_after, present: present})
        end
      end

      client = build_client(definition, subsystem: subsystem)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation).to eq({empty_before: true, empty_after: false, present: [true, false, true, true]})

      reader = client.state(subsystem, definition)
      expect(reader.empty?("k1")).to be(false)
      expect(reader.empty?("k2")).to be(true)
      expect(reader.contains_many("k1", %w[x b a])).to eq([false, true, true])
    end
  end

  describe "set" do
    it "wires every set method for owned and published sets", :aggregate_failures do
      subsystem = "set-#{SecureRandom.hex(4)}"
      definition = Prosody.set(random_state_name("set"), published: true, read_cache: false)
      handler_class = Class.new(StateHandler) do
        def on_message(context, _message)
          set = context.state(@def)
          observation = {empty_before: set.empty?}
          set.add("b1") << "a1" << "a2" << "c1"
          set.delete("c1").delete("absent")
          observation[:empty_after] = set.empty?
          observation[:include] = [set.include?("a1"), set.member?("c1")]
          observation[:many] = set.contains_many(%w[a2 c1 b1])
          observation[:members] = set.each.to_a
          observation[:prefix] = set.each(prefix: "a").to_a
          observation[:reverse] = set.reverse_each(after: "b1", limit: 1).to_a
          observation[:commit] = set.commit
          @sink.push(observation)
        end
      end

      client = build_client(definition, subsystem: subsystem)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation).to eq({
        empty_before: true,
        empty_after: false,
        include: [true, false],
        many: [true, false, true],
        members: %w[a1 a2 b1],
        prefix: %w[a1 a2],
        reverse: %w[a2],
        commit: :applied
      })

      reader = client.state(subsystem, definition)
      expect(reader).to be_a(Prosody::PublishedSet)
      expect([reader.include?("k1", "a1"), reader.member?("k1", "c1")]).to eq([true, false])
      expect(reader.contains_many("k1", %w[b1 z])).to eq([true, false])
      expect([reader.empty?("k1"), reader.empty?("k2")]).to eq([false, true])
      expect(reader.each("k1", range: "a2"..).to_a).to eq(%w[a2 b1])
      expect(reader.reverse_each("k1", prefix: "a").to_a).to eq(%w[a2 a1])
    end
  end

  describe "deque" do
    it "pushes, unshifts, scans, and pops from both ends" do
      definition = Prosody.deque(random_state_name("deq"))
      handler_class = Class.new(StateHandler) do
        def on_message(context, _message)
          deque = context.state(@def)
          deque.push("a")
          deque.push("b")
          deque.unshift("z")
          forward = []
          deque.each { |x| forward << x }
          reverse = []
          deque.reverse_each { |x| reverse << x }
          @sink.push({
            length: deque.length, size: deque.size, empty: deque.empty?,
            first_at: deque.get(0), forward: forward, reverse: reverse,
            shifted: deque.shift, popped: deque.pop
          })
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:length]).to eq(3)
      expect(observation[:size]).to eq(3)
      expect(observation[:empty]).to be(false)
      expect(observation[:first_at]).to eq("z")
      expect(observation[:forward]).to eq(["z", "a", "b"])
      expect(observation[:reverse]).to eq(["b", "a", "z"])
      expect(observation[:shifted]).to eq("z")
      expect(observation[:popped]).to eq("b")
    end

    it "reports empty-deque behavior" do
      definition = Prosody.deque(random_state_name("deq"))
      handler_class = Class.new(StateHandler) do
        def on_message(context, _message)
          deque = context.state(@def)
          @sink.push({
            length: deque.length, empty: deque.empty?,
            shifted: deque.shift, popped: deque.pop
          })
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:length]).to eq(0)
      expect(observation[:empty]).to be(true)
      expect(observation[:shifted]).to be_nil
      expect(observation[:popped]).to be_nil
    end
  end

  describe "null-write rejection" do
    it "surfaces core's permanent rejection of a JSON-null write and leaves the store untouched" do
      value_def = Prosody.value(random_state_name("val"))
      deque_def = Prosody.deque(random_state_name("deq"))
      seeded = "seed-#{SecureRandom.hex(4)}"
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink, value_def, deque_def, seeded)
          @sink = sink
          @value_def = value_def
          @deque_def = deque_def
          @seeded = seeded
        end

        def on_message(context, _message)
          value = context.state(@value_def)
          deque = context.state(@deque_def)
          value.set({"v" => @seeded})
          value.commit
          results = {}
          results[:value] = capture { value.set(nil) }
          results[:deque] = capture { deque.push(nil) }
          results[:after] = value.get["v"]
          @sink.push(results)
        end

        private

        def capture
          yield
          {threw: false}
        rescue => e
          {
            threw: true,
            permanent: e.is_a?(Prosody::PermanentStateError)
          }
        end
      end

      client = build_client(value_def, deque_def)
      client.subscribe(handler_class.new(sink, value_def, deque_def, seeded))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:value]).to eq(threw: true, permanent: true)
      expect(observation[:deque]).to eq(threw: true, permanent: true)
      expect(observation[:after]).to eq(seeded)
    end

    it "rejects a non-message item written to a message collection as transient" do
      definition = Prosody.message_value(random_state_name("mval"))
      handler_class = Class.new(StateHandler) do
        def on_message(context, _message)
          value = context.state(@def)
          value.set({"not" => "a message"})
          @sink.push({threw: false})
        rescue => e
          @sink.push({threw: true, transient: e.is_a?(Prosody::TransientStateError), msg: e.message})
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:threw]).to be(true)
      expect(observation[:transient]).to be(true)
      expect(observation[:msg]).to match(/Prosody::Message/)
    end
  end

  describe "async bridging" do
    it "lets a handler for a different key make progress while one handler is blocked" do
      definition = Prosody.value(random_state_name("val"))

      # Phase 1: probe partitions on the shared topic with a throwaway consumer
      # so we can pick two keys on the same partition and prove intra-partition
      # cross-key concurrency. Partition assignment is deterministic per key, so
      # the same two keys share a partition on the fresh phase-2 topic too.
      probe_sink = new_sink
      probe_handler = Class.new(StateHandler) do
        def on_message(_context, message)
          @sink.push({key: message.key, partition: message.partition})
        end
      end
      probe_config = TestConfig.create_configuration(topic, group_id: "probe-#{SecureRandom.hex(6)}")
      probe_client = track_client(Prosody::Client.new(probe_config))
      probe_client.subscribe(probe_handler.new(probe_sink))
      probe_keys = (0...8).map { |i| "probe-#{i}" }
      probe_keys.each { |k| probe_client.send_message(topic, k, {probe: true}) }
      partitions = {}
      probe_sink.wait(probe_keys.length).each { |obs| partitions[obs[:key]] = obs[:partition] }
      probe_client.unsubscribe
      shared = partitions.group_by { |_k, p| p }.values.find { |group| group.length >= 2 }
      expect(shared).not_to be_nil, "expected two probe keys on the same partition"
      key_a, key_b = shared.first(2).map(&:first)

      # Phase 2 runs on a FRESH topic so the phase-1 probe messages (which carry
      # key_a/key_b) are not replayed into the phase-2 handler.
      phase2_topic = create_extra_topic

      # keyA blocks on a gate after a real (yielding) state op; keyB's handler
      # must still complete its own state op while keyA is parked.
      gate = Queue.new
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink, definition, gate, key_a)
          @sink = sink
          @def = definition
          @gate = gate
          @key_a = key_a
          @a_started = false
          @a_finished = false
        end

        def on_message(context, message)
          value = context.state(@def)
          value.get
          if message.key == @key_a
            @a_started = true
            @sink.push({ev: "A-blocked"})
            @gate.pop
            @a_finished = true
            @sink.push({ev: "A-done"})
          else
            @sink.push({ev: "B-done", a_started: @a_started, a_finished: @a_finished})
          end
        end
      end

      client = track_client(Prosody::Client.new(state_config(phase2_topic, definition)))
      client.subscribe(handler_class.new(sink, definition, gate, key_a))

      client.send_message(phase2_topic, key_a, {n: 1})
      expect(sink.wait(1).first).to eq({ev: "A-blocked"})

      client.send_message(phase2_topic, key_b, {n: 2})
      b_done = sink.wait(1).first
      expect(b_done[:ev]).to eq("B-done")
      expect(b_done[:a_started]).to be(true)
      expect(b_done[:a_finished]).to be(false)

      gate.push(nil)
      expect(sink.wait(1).first).to eq({ev: "A-done"})
    end
  end
end
