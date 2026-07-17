# frozen_string_literal: true

require "spec_helper"

# End-to-end keyed-state scenarios (Appendix 1 items 1-5, 9, 13), driven against
# real Kafka + Cassandra. Mirrors client_spec.rb's idiom: anonymous
# EventHandler subclasses read/write state and push observations into a
# StateSink drained under Timeout; a fresh random topic per test; per-example
# nonce collection names; multi-event scenarios send twice with the same key.
RSpec.describe "Prosody keyed state (integration)", integration: true do
  include_context "keyed state integration"

  describe "item 1: value" do
    it "persists a value written in one event and read in the next" do
      definition = Prosody.value(random_state_name("val"))
      token = "tok-#{SecureRandom.hex(4)}"
      handler_class = Class.new(Prosody::EventHandler) do
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
      handler_class = Class.new(Prosody::EventHandler) do
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

  describe "item 2: map" do
    it "sets, deletes, scans both directions in key order, and round-trips unicode" do
      definition = Prosody.map(random_state_name("map"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

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

  describe "item 3: deque" do
    it "pushes, unshifts, scans, and pops from both ends" do
      definition = Prosody.deque(random_state_name("deq"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

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
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

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

  describe "item 4: message collections" do
    it "round-trips a stored message through a message value collection" do
      definition = Prosody.message_value(random_state_name("mval"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, message)
          value = context.state(@def)
          case message.payload["step"]
          when 1
            value.set(message)
            @sink.push({ev: "stored", offset: message.offset})
          when 2
            got = value.get
            @sink.push({
              ev: "read", topic: got.topic, partition: got.partition,
              offset: got.offset, key: got.key, payload: got.payload,
              live_offset: message.offset
            })
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "mk", {step: 1})
      stored = sink.wait(1).first
      expect(stored[:ev]).to eq("stored")

      client.send_message(topic, "mk", {step: 2})
      read = sink.wait(1).first
      expect(read[:topic]).to eq(topic)
      expect(read[:key]).to eq("mk")
      expect(read[:payload]).to eq({"step" => 1})
      expect(read[:offset]).to eq(stored[:offset])
      expect(read[:offset]).not_to eq(read[:live_offset])
    end

    it "round-trips a stored message through a message deque collection" do
      definition = Prosody.message_deque(random_state_name("mdeq"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, message)
          deque = context.state(@def)
          case message.payload["step"]
          when 1
            deque.push(message)
            @sink.push({ev: "stored", offset: message.offset})
          when 2
            got = deque.get(0)
            @sink.push({
              ev: "read", topic: got.topic, offset: got.offset,
              key: got.key, payload: got.payload, live_offset: message.offset
            })
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "mk", {step: 1})
      stored = sink.wait(1).first
      expect(stored[:ev]).to eq("stored")

      client.send_message(topic, "mk", {step: 2})
      read = sink.wait(1).first
      expect(read[:topic]).to eq(topic)
      expect(read[:payload]).to eq({"step" => 1})
      expect(read[:offset]).to eq(stored[:offset])
      expect(read[:offset]).not_to eq(read[:live_offset])
    end

    it "round-trips messages through a message map collection with unicode keys and get_many" do
      definition = Prosody.message_map(random_state_name("mmap"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, message)
          map = context.state(@def)
          case message.payload["step"]
          when 1
            map.set("primary", message)
            map.set("café", message)
            @sink.push({ev: "stored", offset: message.offset})
          when 2
            primary = map.get("primary")
            unicode = map.get("café")
            batch = map.get_many(["primary", "café", "absent"])
            @sink.push({
              ev: "read",
              primary_offset: primary.offset,
              primary_payload: primary.payload,
              unicode_offset: unicode.offset,
              absent: map.get("absent"),
              batch_length: batch.length,
              batch_nils: batch.count(&:nil?),
              live_offset: message.offset
            })
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "mk", {step: 1})
      stored = sink.wait(1).first
      expect(stored[:ev]).to eq("stored")

      client.send_message(topic, "mk", {step: 2})
      read = sink.wait(1).first
      expect(read[:primary_payload]).to eq({"step" => 1})
      expect(read[:primary_offset]).to eq(stored[:offset])
      expect(read[:unicode_offset]).to eq(stored[:offset])
      expect(read[:primary_offset]).not_to eq(read[:live_offset])
      expect(read[:absent]).to be_nil
      expect(read[:batch_length]).to eq(3)
      expect(read[:batch_nils]).to eq(1)
    end
  end

  describe "item 5: commit / rollback" do
    it "keeps a committed value visible on the retry of a later-failed attempt" do
      definition = Prosody.value(random_state_name("val"))
      committed = {"v" => "committed-#{SecureRandom.hex(4)}"}
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition, committed)
          @sink = sink
          @def = definition
          @committed = committed
          @attempts = 0
        end

        def on_message(context, _message)
          @attempts += 1
          value = context.state(@def)
          if @attempts == 1
            value.set(@committed)
            value.commit
            raise Prosody::TransientStateError, "fail after commit"
          else
            @sink.push({attempt: @attempts, got: value.get})
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition, committed))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:attempt]).to be > 1
      expect(observation[:got]).to eq(committed)
    end

    it "discards uncommitted value writes on rollback, back to the committed floor" do
      definition = Prosody.value(random_state_name("val"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, _message)
          value = context.state(@def)
          value.set({"v" => "A"})
          value.commit
          value.set({"v" => "B"})
          before = value.get
          value.rollback
          after = value.get
          @sink.push({before: before, after: after})
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:before]).to eq({"v" => "B"})
      expect(observation[:after]).to eq({"v" => "A"})
    end

    it "keeps a committed map entry through rollback of later writes" do
      definition = Prosody.map(random_state_name("map"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, _message)
          map = context.state(@def)
          map.set("kept", 1)
          map.commit
          map.set("kept", 2)
          map.set("dropped", 9)
          before = {kept: map.get("kept"), dropped: map.get("dropped")}
          map.rollback
          after = {kept: map.get("kept"), dropped: map.get("dropped")}
          @sink.push({before: before, after: after})
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:before]).to eq({kept: 2, dropped: 9})
      expect(observation[:after]).to eq({kept: 1, dropped: nil})
    end
  end

  describe "item 9: null-write rejection" do
    it "rejects a JSON-null write with a transient NullValueError and leaves the store untouched" do
      value_def = Prosody.value(random_state_name("val"))
      deque_def = Prosody.deque(random_state_name("deq"))
      seeded = "seed-#{SecureRandom.hex(4)}"
      handler_class = Class.new(Prosody::EventHandler) do
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
            transient: e.is_a?(Prosody::TransientStateError),
            null_err: e.is_a?(Prosody::NullValueError),
            msg: e.message
          }
        end
      end

      client = build_client(value_def, deque_def)
      client.subscribe(handler_class.new(sink, value_def, deque_def, seeded))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:value]).to include(threw: true, transient: true, null_err: true)
      expect(observation[:value][:msg]).to match(/clear/)
      expect(observation[:deque]).to include(threw: true, transient: true)
      expect(observation[:deque][:msg]).to match(/null/)
      expect(observation[:after]).to eq(seeded)
    end

    it "rejects a non-message item written to a message collection as transient" do
      definition = Prosody.message_value(random_state_name("mval"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

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

  describe "item 13: async bridging" do
    it "lets a handler for a different key make progress while one handler is blocked" do
      definition = Prosody.value(random_state_name("val"))

      # Phase 1: probe partitions on the shared topic with a throwaway consumer
      # so we can pick two keys on the same partition and prove intra-partition
      # cross-key concurrency. Partition assignment is deterministic per key, so
      # the same two keys share a partition on the fresh phase-2 topic too.
      probe_sink = new_sink
      probe_handler = Class.new(Prosody::EventHandler) do
        def initialize(sink)
          @sink = sink
        end

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
      handler_class = Class.new(Prosody::EventHandler) do
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
