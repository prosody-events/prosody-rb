# frozen_string_literal: true

require "spec_helper"

# Scan close coverage.
#
# A recording fake scan checks that every traversal path closes the native
# scan exactly once through `ensure`. The paths are normal exhaustion, early
# break, and a block exception. Prosody core serializes concurrent chunk pulls
# on one cursor and tests that guarantee.
RSpec.describe "Prosody keyed state scans" do
  # A fake native handle whose `scan` returns a recording cursor. The cursor
  # returns `items` as one chunk, then `nil`. It records each `close` call so
  # the spec can check the traversal's `ensure`.
  def recording_native(items)
    closes = []
    native = Object.new
    native.define_singleton_method(:scan) do |_direction, _query = {}|
      chunks = [items]
      cursor = Object.new
      cursor.define_singleton_method(:next_chunk) { chunks.shift }
      cursor.define_singleton_method(:close) { closes << true }
      cursor
    end
    [native, closes]
  end

  describe "close is wired into every traversal path (unit)" do
    it "closes the scan exactly once on normal exhaustion" do
      native, closes = recording_native([["a", 1], ["b", 2]])
      collected = []
      Prosody::MapState.new(native).each_pair { |k, v| collected << [k, v] }
      expect(collected).to eq([["a", 1], ["b", 2]])
      expect(closes.length).to eq(1)
    end

    it "closes the scan exactly once on early break" do
      native, closes = recording_native([["a", 1], ["b", 2], ["c", 3]])
      Prosody::MapState.new(native).each_pair { break }
      expect(closes.length).to eq(1)
    end

    it "closes the scan exactly once when the block raises, and propagates" do
      native, closes = recording_native([["a", 1], ["b", 2]])
      expect {
        Prosody::MapState.new(native).each_pair { raise "boom" }
      }.to raise_error("boom")
      expect(closes.length).to eq(1)
    end

    it "returns an Enumerator without a block and closes on exhaustion" do
      native, closes = recording_native([["a", 1], ["b", 2]])
      enumerator = Prosody::MapState.new(native).each_pair
      expect(enumerator).to be_a(Enumerator)
      expect(enumerator.next).to eq(["a", 1])
      expect(enumerator.next).to eq(["b", 2])
      expect { enumerator.next }.to raise_error(StopIteration)
      expect(closes.length).to eq(1)
    end

    # Hash#each_pair yields one [key, value] Array. The owned and published map
    # traversals yield the same shape to a block, a lambda, and an Enumerator.
    it "yields each map entry as one [key, value] pair" do
      pairs = [["a", 1], ["b", 2]]
      owned, = recording_native(pairs)
      published = Object.new
      published.define_singleton_method(:scan) { |_key, direction, query| owned.scan(direction, query) }
      traversals = {
        owned: ->(&block) { Prosody::MapState.new(owned).each_pair(&block) },
        published: ->(&block) { Prosody::PublishedMap.new(published).each_pair("user", &block) }
      }

      traversals.each do |name, traverse|
        from_lambda = []
        traverse.call(&->(pair) { from_lambda << pair })
        expect(from_lambda).to eq(pairs), "#{name}: lambda"
        expect(traverse.call.map { |entry| entry }).to eq(pairs), "#{name}: one-parameter block"
        expect(traverse.call.map(&:first)).to eq(%w[a b]), "#{name}: symbol block"
      end
    end

    it "closes the deque scan exactly once on early break" do
      native, closes = recording_native([1, 2, 3])
      Prosody::DequeState.new(native).each { break }
      expect(closes.length).to eq(1)
    end
  end

  describe "leaked enumerators (integration)", integration: true do
    include_context "keyed state integration"

    it "raises the terminated (transient) error for an enumerator leaked past the handler" do
      definition = Prosody.map(random_state_name("map"))
      handler_class = Class.new(StateHandler) do
        def on_message(context, message)
          case message.payload["step"]
          when 1
            map = context.state(@def)
            map.set("x", 1)
            @enum = map.each_pair
            @sink.push({ev: "captured"})
          when 2
            transient = begin
              @enum.next
              false
            rescue Prosody::TransientStateError
              true
            end
            @sink.push({ev: "leaked", transient: transient})
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {step: 1})
      expect(sink.wait(1).first).to eq({ev: "captured"})

      client.send_message(topic, "k1", {step: 2})
      observation = sink.wait(1).first
      expect(observation[:ev]).to eq("leaked")
      expect(observation[:transient]).to be(true)
    end
  end
end
