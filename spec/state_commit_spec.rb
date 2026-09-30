# frozen_string_literal: true

require "spec_helper"

# Mid-handler commit and rollback against real Kafka and Cassandra.
RSpec.describe "Prosody state commit and rollback (integration)", integration: true do
  include_context "keyed state integration"

  describe "commit and rollback" do
    it "keeps a committed value visible on the retry of a later-failed attempt" do
      definition = Prosody.value(random_state_name("val"))
      committed = {"v" => "committed-#{SecureRandom.hex(4)}"}
      handler_class = Class.new(CompleteHandler) do
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
      handler_class = Class.new(StateHandler) do
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
      handler_class = Class.new(StateHandler) do
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

  describe "commit and rollback outcomes" do
    it "maps each store outcome to a symbol for every collection kind" do
      definitions = {
        value: Prosody.value(random_state_name("val")),
        map: Prosody.map(random_state_name("map")),
        set: Prosody.set(random_state_name("set")),
        deque: Prosody.deque(random_state_name("deque")),
        message_map: Prosody.message_map(random_state_name("message-map"))
      }
      writes = {
        value: ->(state, _message) { state.set(1) },
        map: ->(state, _message) { state.set("k", 1) },
        set: ->(state, _message) { state.add("m") },
        deque: ->(state, _message) { state.push(1) },
        message_map: ->(state, message) { state.set("k", message) }
      }
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink, definitions, writes)
          @sink = sink
          @definitions = definitions
          @writes = writes
        end

        def on_message(context, message)
          outcomes = @definitions.to_h do |kind, definition|
            state = context.state(definition)
            write = @writes.fetch(kind)
            idle = [state.commit, state.rollback]
            write.call(state, message)
            committed = state.commit
            write.call(state, message)
            rolled_back = state.rollback
            [kind, idle + [committed, rolled_back]]
          end
          @sink.push(outcomes)
        end
      end

      client = build_client(*definitions.values)
      client.subscribe(handler_class.new(sink, definitions, writes))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expected = [:no_op, :no_op, :applied, :applied]
      expect(observation).to eq(definitions.keys.to_h { |kind| [kind, expected] })
    end
  end
end
