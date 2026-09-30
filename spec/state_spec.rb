# frozen_string_literal: true

require "spec_helper"

# Infra-free unit/mock coverage for host-value mapping, the error-class
# hierarchy, definition serialization and routing, and Ruby index and direction
# behavior.
RSpec.describe "Prosody keyed state" do
  # Builds a mock client and returns the raised exception, or nil on success.
  # Registration validation runs while building the consumer configuration,
  # before HighLevelClient::new, so every error path here is infra-free.
  #
  # A default `bootstrap_servers` is supplied so the accept cases (which reach
  # past state validation into producer-config validation) do not depend on an
  # ambient `PROSODY_BOOTSTRAP_SERVERS`; mock mode never connects to it. Callers
  # may override it via `options`.
  def client_error(**options)
    client = Prosody::Client.new(mock: true, group_id: "state-spec", bootstrap_servers: "localhost:9094", **options)
    client.shutdown
    nil
  rescue => e
    e
  end

  describe "host-value mapping" do
    value = {name: "c", kind: "value", payload: "json"}
    map = {name: "m", kind: "map", payload: "json"}
    deque = {name: "d", kind: "deque", payload: "json"}

    # Each row is [label, client options, expected message], where a nil
    # message means the options are valid.
    [
      ["a single bare registration hash", {state_collections: value}, nil],
      ["a set with a TTL and a keyset limit", {state_collections: [Prosody.set("s", ttl: 60, keyset_limit: 16)]}, nil],
      ["keyset_limit 0 on a map", {state_collections: [map.merge(keyset_limit: 0)]}, nil],
      ["a positive capacity on a deque", {state_collections: [deque.merge(capacity: 100)]}, nil],
      ["a fractional TTL", {state_collections: [value.merge(ttl_seconds: 30.5)]}, /expected u32/],
      ["a Float TTL", {state_collections: [value.merge(ttl_seconds: 5.0)]}, /expected u32/],
      ["a negative TTL", {state_collections: [value.merge(ttl_seconds: -5)]}, /expected u32/],
      ["a NaN TTL", {state_collections: [value.merge(ttl_seconds: Float::NAN)]}, /expected u32/],
      ["an infinite TTL", {state_collections: [value.merge(ttl_seconds: Float::INFINITY)]}, /expected u32/],
      ["a TTL above the u32 ceiling", {state_collections: [value.merge(ttl_seconds: 2**32)]}, /expected u32/],
      ["a fractional keyset_limit", {state_collections: [map.merge(keyset_limit: 128.5)]}, /expected usize/],
      ["a negative keyset_limit", {state_collections: [map.merge(keyset_limit: -1)]}, /expected usize/],
      ["an infinite keyset_limit", {state_collections: [map.merge(keyset_limit: Float::INFINITY)]}, /expected usize/],
      ["keyset_limit on a value", {state_collections: [value.merge(keyset_limit: 128)]}, /keyset_limit.*only valid for map and set/],
      ["a set with a json payload", {state_collections: [{name: "s", kind: "set", payload: "json"}]}, /state_collections\[0\]\.payload/],
      ["a set with a presence payload", {state_collections: [{name: "s", kind: "set", payload: "presence"}]}, /state_collections\[0\]\.payload/],
      ["a map without a payload", {state_collections: [{name: "m", kind: "map"}]}, /state_collections\[0\]\.payload: required/],
      ["capacity on a map", {state_collections: [map.merge(capacity: 100)]}, /capacity.*only valid for deque/],
      ["a zero capacity", {state_collections: [deque.merge(capacity: 0)]}, /expected a nonzero usize/],
      ["a negative capacity", {state_collections: [deque.merge(capacity: -1)]}, /expected a nonzero usize/],
      ["a fractional capacity", {state_collections: [deque.merge(capacity: 1.5)]}, /expected a nonzero usize/],
      ["a NaN capacity", {state_collections: [deque.merge(capacity: Float::NAN)]}, /expected a nonzero usize/],
      ["an infinite capacity", {state_collections: [deque.merge(capacity: Float::INFINITY)]}, /expected a nonzero usize/],
      ["an unknown kind token", {state_collections: [value.merge(kind: "tree")]}, /state_collections\[0\]\.kind.*expected/],
      ["an unknown payload token", {state_collections: [value.merge(payload: "proto")]}, /state_collections\[0\]\.payload.*expected/],
      ["a zero in-memory block-cache size", {state_owned_cache_size: "0"}, /state_owned_cache_size/],
      ["a zero memtable size", {state_memtable_size: "0"}, /state_memtable_size/],
      ["a zero published-read cache size", {state_read_cache_size: "0"}, /state_read_cache_size/]
    ].each do |label, options, expected|
      it "#{expected ? "rejects" : "accepts"} #{label}" do
        error = client_error(**options)
        expect(error&.message).to(expected ? match(expected) : be_nil)
      end
    end

    it "rejects an ambiguous published-read cache policy" do
      error = client_error(state_read_cache: true)
      expect(error.message).to match(/state_read_cache.*ambiguous/)

      client = Prosody::Client.new(mock: true, group_id: "state-spec", bootstrap_servers: "localhost:9094")
      expect { client.state(:accounts, Prosody.value("v", read_cache: true)) }
        .to raise_error(ArgumentError, /read_cache.*ambiguous/)
      expect(client.state(:accounts, Prosody.value("v", read_cache: Rational(1, 2)))).to be_a(Prosody::PublishedValue)
    ensure
      client&.shutdown
    end
  end

  describe "error hierarchy" do
    it "classifies PermanentStateError as a permanent Prosody error" do
      error = Prosody::PermanentStateError.new("boom")
      expect(error).to be_a(Prosody::PermanentError)
      expect(error).to be_a(Prosody::Error)
      expect(error.permanent?).to be(true)
    end

    it "classifies TransientStateError as a transient Prosody error" do
      error = Prosody::TransientStateError.new("boom")
      expect(error).to be_a(Prosody::TransientError)
      expect(error.permanent?).to be(false)
    end
  end

  describe "definition constructors" do
    it "builds a frozen value definition" do
      definition = Prosody.value("cart", ttl: 30)
      expect(definition).to be_frozen
      expect(definition.name).to eq("cart")
      expect(definition.kind).to eq("value")
      expect(definition.payload).to eq("json")
      expect(definition.ttl_seconds).to eq(30)
    end

    it "builds a map definition with a keyset limit" do
      definition = Prosody.map("sessions", keyset_limit: 256)
      expect(definition.kind).to eq("map")
      expect(definition.payload).to eq("json")
      expect(definition.keyset_limit).to eq(256)
    end

    it "builds a set definition with no payload" do
      definition = Prosody.set("tags", ttl: 60, keyset_limit: 16, read_uncommitted: true, published: true, read_cache: 2)
      expect(definition).to be_frozen
      expect(definition.payload).to be_nil
      expect(definition.to_state_config).to eq({
        name: "tags", kind: "set", ttl_seconds: 60,
        read_uncommitted: true, published: true, keyset_limit: 16
      })
      expect(definition.read_cache).to eq(2)
      expect { Prosody.set("tags", capacity: 1) }.to raise_error(ArgumentError)
    end

    it "builds a deque definition" do
      expect(Prosody.deque("events").kind).to eq("deque")
    end

    it "uses one descriptor for owned and published access" do
      definition = Prosody.value("cart", published: true, read_cache: false)
      expect(definition.to_state_config).to include(published: true)
      expect(definition.read_cache).to be(false)
    end

    it "carries a deque window capacity on deque and message_deque" do
      expect(Prosody.deque("d", capacity: 100).capacity).to eq(100)
      expect(Prosody.message_deque("d", capacity: 50).capacity).to eq(50)
    end

    it "serializes capacity only when set" do
      expect(Prosody.deque("d", capacity: 100).to_state_config).to include(capacity: 100)
      expect(Prosody.deque("d").to_state_config).not_to include(:capacity)
    end

    it "rejects capacity on non-deque constructors (no such kwarg)" do
      expect { Prosody.value("v", capacity: 10) }.to raise_error(ArgumentError)
      expect { Prosody.map("m", capacity: 10) }.to raise_error(ArgumentError)
    end

    it "builds message-payload definitions" do
      expect(Prosody.message_value("v").payload).to eq("message")
      expect(Prosody.message_map("m").payload).to eq("message")
      expect(Prosody.message_deque("d").payload).to eq("message")
    end

    it "serializes into state_collections, omitting unset optionals" do
      config = Prosody::Configuration.new
      config.state_collections = [Prosody.value("cart", ttl: 30), Prosody.deque("d")]
      expect(config.to_hash[:state_collections]).to eq([
        {name: "cart", kind: "value", payload: "json", ttl_seconds: 30},
        {name: "d", kind: "deque", payload: "json"}
      ])
    end
  end

  describe "Prosody::State::Reading#state" do
    it "uses every JSON and set descriptor's typed published-state access strategy" do
      calls = []
      native = Object.new
      reader = Object.new.extend(Prosody::State::Reading)
      %i[published_value published_map published_set published_deque].each do |vend_method|
        reader.define_singleton_method(vend_method) do |*args|
          calls << [vend_method, *args]
          native
        end
      end

      cases = [
        [Prosody.value("cart", published: true, read_cache: 2), :published_value, Prosody::PublishedValue],
        [Prosody.map("sessions", published: true, read_cache: 2), :published_map, Prosody::PublishedMap],
        [Prosody.set("tags", published: true, read_cache: 2), :published_set, Prosody::PublishedSet],
        [Prosody.deque("jobs", published: true, read_cache: 2), :published_deque, Prosody::PublishedDeque]
      ]

      cases.each do |definition, vend_method, wrapper|
        expect(reader.state(:accounts, definition)).to be_a(wrapper)
        expect(calls.last).to eq([vend_method, "accounts", definition.name, 2])
      end
    end

    it "rejects message collections" do
      reader = Object.new.extend(Prosody::State::Reading)
      [Prosody.message_value("v"), Prosody.message_map("m"), Prosody.message_deque("d")].each do |definition|
        expect { reader.state(:accounts, definition) }
          .to raise_error(ArgumentError, "published state readers support JSON and set collections only")
      end
    end
  end

  describe "Prosody::State::Vending#state" do
    # A stand-in that mixes in the vending module (as the native Context does)
    # and records vend calls, so routing can be exercised without a real context.
    def build_fake_context(calls)
      fake = Object.new.extend(Prosody::State::Vending)
      sentinel = Object.new
      %i[value_state map_state set_state deque_state message_value_state message_map_state message_deque_state].each do |vend|
        fake.define_singleton_method(vend) do |name|
          calls << [vend, name]
          sentinel
        end
      end
      fake
    end

    it "is included in the native Context" do
      expect(Prosody::Context.include?(Prosody::State::Vending)).to be(true)
    end

    # The typed handles are the public API. The native vend methods return raw
    # handles, so neither the Context nor the Client exposes them.
    it "keeps the native vend methods private" do
      owned = %i[value_state map_state set_state deque_state message_value_state message_map_state message_deque_state]
      published = %i[published_value published_map published_set published_deque]
      expect(owned.reject { |name| Prosody::Context.private_method_defined?(name) }).to be_empty
      expect(published.reject { |name| Prosody::Client.private_method_defined?(name) }).to be_empty
    end

    it "uses every descriptor's typed owned-state access strategy" do
      cases = [
        [Prosody.value("value"), :value_state, Prosody::ValueState],
        [Prosody.map("map"), :map_state, Prosody::MapState],
        [Prosody.set("set"), :set_state, Prosody::SetState],
        [Prosody.deque("deque"), :deque_state, Prosody::DequeState],
        [Prosody.message_value("message-value"), :message_value_state, Prosody::ValueState],
        [Prosody.message_map("message-map"), :message_map_state, Prosody::MapState],
        [Prosody.message_deque("message-deque"), :message_deque_state, Prosody::DequeState]
      ]

      cases.each do |definition, vend_method, wrapper|
        calls = []
        handle = build_fake_context(calls).state(definition)
        expect(handle).to be_a(wrapper)
        expect(calls).to eq([[vend_method, definition.name]])
      end
    end

    it "caches vended handles per definition" do
      calls = []
      fake = build_fake_context(calls)
      first = fake.state(Prosody.value("cart"))
      second = fake.state(Prosody.value("cart"))
      expect(second).to equal(first)
      expect(calls).to eq([[:value_state, "cart"]])
    end
  end

  describe "Ruby-side guards" do
    # A stand-in native deque that returns nil for reads and a no-op scan, so the
    # Ruby guards fire before any real native call. `len`/`peek_back` back the
    # Array-style negative-index resolution (both yield "empty").
    def fake_deque
      native = Object.new
      native.define_singleton_method(:get) { |_index| nil }
      native.define_singleton_method(:len) { 0 }
      native.define_singleton_method(:peek_back) { nil }
      native.define_singleton_method(:scan) do |_direction, _query = {}|
        scan = Object.new
        scan.define_singleton_method(:next) { nil }
        scan.define_singleton_method(:close) { nil }
        scan
      end
      native
    end

    it "resolves a negative deque index Array-style (no longer rejected)" do
      # -1 fast-paths peek_back; both reach the native handle rather than raising.
      expect(Prosody::DequeState.new(fake_deque).get(-1)).to be_nil
      expect(Prosody::DequeState.new(fake_deque).get(-2)).to be_nil
    end

    it "rejects a fractional deque index" do
      expect { Prosody::DequeState.new(fake_deque).get(1.5) }
        .to raise_error(Prosody::TransientStateError, /index/)
    end

    it "rejects a non-integer deque index" do
      expect { Prosody::DequeState.new(fake_deque).get("x") }
        .to raise_error(Prosody::TransientStateError, /index/)
    end

    it "passes a valid index through to the native handle" do
      expect(Prosody::DequeState.new(fake_deque).get(0)).to be_nil
    end
  end

  describe "traversal over falsy items" do
    # A stand-in native handle whose scan replays `items` and then returns nil
    # (the exhaustion sentinel), so traversal can be exercised without a vended
    # native handle.
    def fake_scanning_native(items)
      native = Object.new
      native.define_singleton_method(:scan) do |_direction, _query = {}|
        remaining = items.dup
        scan = Object.new
        scan.define_singleton_method(:next) { remaining.empty? ? nil : remaining.shift }
        scan.define_singleton_method(:close) { nil }
        scan
      end
      native
    end

    it "yields a stored false and everything after it in a deque" do
      collected = []
      Prosody::DequeState.new(fake_scanning_native([1, false, 2])).each { |item| collected << item }
      expect(collected).to eq([1, false, 2])
    end

    it "yields a map pair whose value is false without dropping the tail" do
      collected = []
      native = fake_scanning_native([["a", false], ["b", 2]])
      Prosody::MapState.new(native).each_pair { |key, value| collected << [key, value] }
      expect(collected).to eq([["a", false], ["b", 2]])
    end

    it "uses symbols for owned and published scan directions" do
      directions = []
      native = fake_scanning_native([])
      original_scan = native.method(:scan)
      native.define_singleton_method(:scan) do |*args|
        directions << args.grep(Symbol).last
        original_scan.call(args.grep(Symbol).last)
      end

      Prosody::MapState.new(native).reverse_each_pair.to_a
      Prosody::PublishedMap.new(native).reverse_each_pair("user-1").to_a

      expect(directions).to eq([:backward, :backward])
    end

    it "gives published maps the owned read operations" do
      native = fake_scanning_native([["a", 1], ["b", 2]])
      scan = native.method(:scan)
      native.define_singleton_method(:scan) { |_key, direction, _query| scan.call(direction) }
      native.define_singleton_method(:contains_key) { |key, map_key| [key, map_key] == ["user-1", "a"] }
      key_scan = []
      key_native = fake_scanning_native(["b", "a"])
      native.define_singleton_method(:keys) do |key, direction, _query|
        key_scan << [key, direction]
        key_native.scan(direction)
      end

      state = Prosody::PublishedMap.new(native)
      expect(state.key?("user-1", "a")).to be(true)
      expect(state.reverse_each_key("user-1").to_a).to eq(["b", "a"])
      expect(state.each_value("user-1").to_a).to eq([1, 2])
      expect(key_scan).to eq([["user-1", :backward]])
    end

    it "gives published deques the owned read operations" do
      native = Object.new
      native.define_singleton_method(:length) { |_key| 2 }
      native.define_singleton_method(:is_empty) { |_key| false }
      native.define_singleton_method(:peek_front) { |_key| "first" }
      native.define_singleton_method(:peek_back) { |_key| "last" }
      native.define_singleton_method(:get) { |_key, index| ["first", "last"][index] }

      state = Prosody::PublishedDeque.new(native)
      expect(state.size("user-1")).to eq(2)
      expect(state).not_to be_empty("user-1")
      expect(state.first("user-1")).to eq("first")
      expect(state.last("user-1")).to eq("last")
      expect(state.get("user-1", -1)).to eq("last")
    end
  end
end
