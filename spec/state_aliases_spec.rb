# frozen_string_literal: true

require "spec_helper"

# Infra-free unit tests for the idiomatic Ruby aliases and conveniences layered
# over the canonical keyed-state ops. The handles are constructed directly over
# fake native handles backed by a plain Ruby Hash/Array, so these assert the
# wrapper behavior (delegation, Hash/Array-parity semantics, the null-ban =>
# nil-means-absent conveniences) without Kafka/Cassandra.
RSpec.describe "keyed-state idiomatic aliases" do
  # Fake native handles: only the methods the wrappers delegate to, backed by
  # real Ruby containers so behavior (not just delegation) is verified.
  # Writes return nil, exactly as the real native FFI seam does (it returns
  # qnil), so a passthrough alias would surface nil — the wrappers' explicit
  # return values are what these specs assert.
  let(:value_native) do
    Class.new do
      def initialize = (@value = nil)
      def get = @value

      def set(v)
        @value = v
        nil
      end

      def clear = (@value = nil)
      def commit = nil
      def rollback = nil
    end
  end

  let(:cursor) do
    Class.new do
      def initialize(items)
        @items = items
        @index = 0
      end

      def next
        return nil if @index >= @items.length

        item = @items[@index]
        @index += 1
        item
      end

      def close = nil
    end
  end

  let(:map_native) do
    cursor_class = cursor
    Class.new do
      define_method(:initialize) { |hash = {}| @hash = hash }
      def get(key) = @hash[key]
      def contains_key(key) = @hash.key?(key)
      def get_many(keys) = keys.map { |key| @hash[key] }

      def set(key, value)
        @hash[key] = value
        nil
      end

      def remove(key)
        @hash.delete(key)
        nil
      end

      def clear
        @hash.clear
        nil
      end

      def commit = nil
      def rollback = nil

      define_method(:scan) do |direction|
        pairs = @hash.sort_by { |key, _| key }
        pairs = pairs.reverse if direction == "backward"
        cursor_class.new(pairs.map { |key, value| [key, value] })
      end

      define_method(:keys) do |direction|
        keys = @hash.keys.sort
        keys = keys.reverse if direction == "backward"
        cursor_class.new(keys)
      end
    end
  end

  let(:deque_native) do
    cursor_class = cursor
    Class.new do
      define_method(:initialize) { |array = []| @array = array }
      def len = @array.length
      def is_empty = @array.empty?
      def get(index) = @array[index]

      def push_back(value)
        @array.push(value)
        nil
      end

      def push_front(value)
        @array.unshift(value)
        nil
      end

      def peek_front = @array.first
      def peek_back = @array.last
      def pop_back = @array.pop
      def pop_front = @array.shift

      def clear
        @array.clear
        nil
      end

      def commit = nil
      def rollback = nil

      define_method(:scan) do |direction|
        items = (direction == "backward") ? @array.reverse : @array.dup
        cursor_class.new(items)
      end
    end
  end

  describe Prosody::ValueState do
    subject(:value) { Prosody::ValueState.new(value_native.new) }

    it "aliases #value to #get and #value= to #set" do
      expect(value.value).to be_nil
      value.value = {"a" => 1}
      expect(value.value).to eq({"a" => 1})
      expect(value.get).to eq({"a" => 1})
    end

    it "returns the assigned value from #value= like a Ruby attribute writer" do
      expect(value.value = 42).to eq(42)
    end
  end

  describe Prosody::MapState do
    subject(:map) { Prosody::MapState.new(map_native.new({"a" => 1, "b" => 2})) }

    it "aliases #[] to #get and #[]=/#store to #set" do
      expect(map["a"]).to eq(1)
      expect(map["z"]).to be_nil
      map["c"] = 3
      expect(map["c"]).to eq(3)
      map.store("d", 4)
      expect(map["d"]).to eq(4)
    end

    it "returns the assigned value from #[]= (Ruby forces it) and #store" do
      expect(map["c"] = 3).to eq(3)
      # #store is called normally, so its explicit return matters: it must be
      # the stored value, not the native write's nil.
      expect(map.store("e", 5)).to eq(5)
    end

    it "reads several keys positionally with #values_at, nil for absent" do
      expect(map.values_at("a", "z", "b")).to eq([1, nil, 2])
    end

    it "returns present keys as a Hash with #slice, omitting absent keys" do
      expect(map.slice("a", "z", "b")).to eq({"a" => 1, "b" => 2})
      expect(map.slice("z")).to eq({})
    end

    describe "#fetch_values" do
      it "returns every value in order when all keys are present" do
        expect(map.fetch_values("b", "a")).to eq([2, 1])
      end

      it "raises KeyError for the first absent key with no block" do
        expect { map.fetch_values("a", "z") }.to raise_error(KeyError, /z/)
      end

      it "substitutes the block result for absent keys" do
        expect(map.fetch_values("a", "z") { |k| "missing #{k}" }).to eq([1, "missing z"])
      end
    end

    describe "#fetch" do
      it "returns the value when present" do
        expect(map.fetch("a")).to eq(1)
      end

      it "returns the default when absent" do
        expect(map.fetch("z", :default)).to eq(:default)
      end

      it "calls the block when absent, block winning over default" do
        expect(map.fetch("z", :default) { |k| "missing #{k}" }).to eq("missing z")
      end

      it "raises KeyError when absent with no default or block" do
        expect { map.fetch("z") }.to raise_error(KeyError, /z/)
      end

      it "populates the KeyError's #key and #receiver like Hash#fetch" do
        map.fetch("z")
      rescue KeyError => e
        expect(e.key).to eq("z")
        expect(e.receiver).to equal(map)
      end

      it "raises ArgumentError for more than one default" do
        expect { map.fetch("z", 1, 2) }.to raise_error(ArgumentError)
      end
    end

    it "answers presence via #contains_key (not #get) with #key? and aliases" do
      native = map_native.new({"a" => 1})
      presence = Prosody::MapState.new(native)
      # The cheap presence path must never decode a value: assert it never reads.
      expect(native).not_to receive(:get)
      %i[key? has_key? include? member?].each do |predicate|
        expect(presence.public_send(predicate, "a")).to be(true)
        expect(presence.public_send(predicate, "z")).to be(false)
      end
    end

    it "yields keys in order with #each_key and reversed with #reverse_each_key" do
      native = map_native.new({"a" => 1, "b" => 2, "c" => 3})
      keys = Prosody::MapState.new(native)
      # The key scan must use the key-only cursor, never the value scan.
      expect(native).not_to receive(:scan)
      forward = []
      keys.each_key { |key| forward << key }
      expect(forward).to eq(%w[a b c])
      backward = []
      keys.reverse_each_key { |key| backward << key }
      expect(backward).to eq(%w[c b a])
    end

    it "returns an Enumerator from #each_key / #reverse_each_key without a block" do
      expect(map.each_key).to be_a(Enumerator)
      expect(map.each_key.to_a).to eq(%w[a b])
      expect(map.reverse_each_key.to_a).to eq(%w[b a])
    end

    it "digs into a nested value with #dig" do
      nested = Prosody::MapState.new(map_native.new({"outer" => {"inner" => 7}}))
      expect(nested.dig("outer", "inner")).to eq(7)
      expect(nested.dig("missing", "inner")).to be_nil
      expect(nested.dig("outer")).to eq({"inner" => 7})
    end

    it "raises TypeError digging past a scalar, like Hash#dig" do
      scalar = Prosody::MapState.new(map_native.new({"n" => 1}))
      expect { scalar.dig("n", "deeper") }.to raise_error(TypeError, /dig/)
    end

    it "aliases #each to #each_pair, yielding [key, value] pairs Hash-style" do
      two_param = []
      map.each { |k, v| two_param << [k, v] }
      expect(two_param).to eq([["a", 1], ["b", 2]])

      one_param = []
      map.each { |pair| one_param << pair }
      expect(one_param).to eq([["a", 1], ["b", 2]])

      expect(map.each).to be_a(Enumerator)
      expect(map.each.to_a).to eq([["a", 1], ["b", 2]])
      expect(map.method(:each)).to eq(map.method(:each_pair))
    end
  end

  describe Prosody::DequeState do
    subject(:deque) { Prosody::DequeState.new(deque_native.new([10, 20, 30])) }

    it "does not define Array's #[] or #at (remote, forward-only deque)" do
      # These would imply negative indices and ranges the remote deque cannot
      # honor; explicit #get / #first / #last are the honest accessors.
      expect(deque).not_to respond_to(:[])
      expect(deque).not_to respond_to(:at)
      expect(deque.get(2)).to eq(30)
    end

    it "appends with #append and prepends with #prepend, returning self" do
      expect(deque.append(40)).to equal(deque)
      expect(deque.prepend(0)).to equal(deque)
      expect(deque.get(0)).to eq(0)
      expect(deque.get(4)).to eq(40)
    end

    it "appends with #<< and returns self for chaining" do
      expect(deque << 40).to equal(deque)
      deque << 50 << 60
      expect(deque.get(5)).to eq(60)
    end

    it "reads the ends with #first and #last via peeks (no len or get)" do
      native = deque_native.new([10, 20, 30])
      ends = Prosody::DequeState.new(native)
      # Endpoint peeks take one round trip each: no length read, no indexed get.
      expect(native).not_to receive(:len)
      expect(native).not_to receive(:get)
      expect(ends.first).to eq(10)
      expect(ends.last).to eq(30)
      empty = Prosody::DequeState.new(deque_native.new([]))
      expect(empty.first).to be_nil
      expect(empty.last).to be_nil
    end

    describe "Array-style negative indexing" do
      it "resolves a negative #get against the length, nil before the front" do
        expect(deque.get(-1)).to eq(30)
        expect(deque.get(-3)).to eq(10)
        expect(deque.get(-4)).to be_nil
      end

      it "resolves a negative #fetch, defaulting or raising before the front" do
        expect(deque.fetch(-1)).to eq(30)
        expect(deque.fetch(-4, :default)).to eq(:default)
        expect { deque.fetch(-4) }.to raise_error(IndexError, /-4/)
      end

      it "fast-paths -1 through peek_back with no length read" do
        native = deque_native.new([10, 20, 30])
        fast = Prosody::DequeState.new(native)
        expect(native).not_to receive(:len)
        expect(native).not_to receive(:get)
        expect(fast.get(-1)).to eq(30)
        expect(fast.fetch(-1)).to eq(30)
      end
    end

    describe "#fetch" do
      it "returns the element when in range" do
        expect(deque.fetch(1)).to eq(20)
      end

      it "returns the default past the end" do
        expect(deque.fetch(9, :default)).to eq(:default)
      end

      it "calls the block past the end, block winning over default" do
        expect(deque.fetch(9, :default) { |i| "past #{i}" }).to eq("past 9")
      end

      it "raises IndexError past the end with no default or block" do
        expect { deque.fetch(9) }.to raise_error(IndexError, /9/)
      end

      it "raises TransientStateError for a non-Integer index (negatives now valid)" do
        expect { deque.fetch(1.5) }.to raise_error(Prosody::TransientStateError)
        expect { deque.fetch("x") }.to raise_error(Prosody::TransientStateError)
      end

      it "raises ArgumentError for more than one default" do
        expect { deque.fetch(9, 1, 2) }.to raise_error(ArgumentError)
      end
    end
  end
end
