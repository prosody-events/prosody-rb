# frozen_string_literal: true

require "spec_helper"
require "async"
require "async/barrier"

# Scan concurrency and close coverage.
#
# The strong close/ordering guarantees are asserted at the Ruby level against a
# recording fake scan (fast, deterministic): every traversal path closes the
# native scan exactly once via `ensure`, on normal exhaustion, early break, and
# block exception. The native permit's "one pull at a time / exactly one close"
# are Rust-enforced and not Ruby-observable; the falsifiable proxy is the
# no-duplicate / no-loss / ordered union of concurrent `#next` calls across a
# chunk boundary, plus close idempotence, exercised against a real native cursor.
RSpec.describe "Prosody keyed state scans" do
  # A fake native handle whose `scan` returns a recording cursor: it replays
  # `items` then the `nil` exhaustion sentinel, and records each `close` call so
  # the traversal's `ensure` can be asserted.
  def recording_native(items)
    closes = []
    native = Object.new
    native.define_singleton_method(:scan) do |_direction|
      remaining = items.dup
      cursor = Object.new
      cursor.define_singleton_method(:next) { remaining.empty? ? nil : remaining.shift }
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

    it "closes the deque scan exactly once on early break" do
      native, closes = recording_native([1, 2, 3])
      Prosody::DequeState.new(native).each { break }
      expect(closes.length).to eq(1)
    end
  end

  describe "concurrent #next across a chunk boundary (integration)", integration: true do
    include_context "keyed state integration"

    # More than SCAN_READY_CHUNK_SIZE (256) entries so a single scan spans at
    # least two native ready-chunk pulls, exercising the permit across a boundary.
    ENTRY_COUNT = 300

    it "drives concurrent #next with no duplicates or loss, then closes idempotently" do
      definition = Prosody.map(random_state_name("map"))
      count = ENTRY_COUNT
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition, count)
          @sink = sink
          @def = definition
          @count = count
        end

        def on_message(context, _message)
          map = context.state(@def)
          @count.times { |i| map.set(format("k%04d", i), i) }
          map.commit

          # Drive the raw native cursor (obtained directly from the native
          # handle via the surviving @native.scan seam) from N concurrent
          # sub-tasks so the one-token permit is exercised; reaching through the
          # ivar keeps this test-only with no new public surface.
          scan = map.instance_variable_get(:@native).scan(:forward)
          collected = []
          barrier = Async::Barrier.new
          4.times do
            barrier.async do
              loop do
                pair = scan.next
                break if pair.nil?
                collected << pair
              end
            end
          end
          barrier.wait

          # close is idempotent, and a #next after close returns nil.
          scan.close
          double_close_ok = begin
            scan.close
            true
          rescue
            false
          end
          after_close = scan.next

          @sink.push({
            count: collected.length,
            unique_keys: collected.map(&:first).uniq.length,
            sorted: collected.sort_by(&:first),
            double_close_ok: double_close_ok,
            after_close: after_close
          })
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition, count))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expected = Array.new(count) { |i| [format("k%04d", i), i] }
      expect(observation[:count]).to eq(count)
      expect(observation[:unique_keys]).to eq(count)
      expect(observation[:sorted]).to eq(expected)
      expect(observation[:double_close_ok]).to be(true)
      expect(observation[:after_close]).to be_nil
    end

    it "raises the terminated (transient) error for an enumerator leaked past the handler" do
      definition = Prosody.map(random_state_name("map"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

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
