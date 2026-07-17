# frozen_string_literal: true

require "spec_helper"

# Infra-free unit/mock coverage for the keyed-state surface: the shared
# registration validation table (raised through Client.new in mock mode, before
# any network/fjall), the error-class hierarchy, the definition constructors and
# their serialization/routing, and the Ruby-side index and direction guards.
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
    Prosody::Client.new(mock: true, group_id: "state-spec", bootstrap_servers: "localhost:9094", **options)
    nil
  rescue => e
    e
  end

  describe "registration validation table" do
    it "rejects an empty collection name" do
      error = client_error(state_collections: [{name: "", kind: "value", payload: "json"}])
      expect(error).to be_a(ArgumentError)
      expect(error.message).to match(/state_collections\[0\]\.name.*empty/)
    end

    it "rejects a duplicate collection name" do
      error = client_error(state_collections: [
        {name: "dup", kind: "value", payload: "json"},
        {name: "dup", kind: "deque", payload: "json"}
      ])
      expect(error).to be_a(ArgumentError)
      expect(error.message).to match(/state_collections\[1\]\.name.*duplicate/)
    end

    it "accepts a single bare registration hash" do
      error = client_error(state_collections: {name: "cart", kind: "value", payload: "json"})
      expect(error).to be_nil
    end

    it "rejects a zero TTL" do
      error = client_error(state_collections: [{name: "c", kind: "value", payload: "json", ttl_seconds: 0}])
      expect(error.message).to match(/state_collections\[0\]\.ttl_seconds.*whole number/)
    end

    it "rejects a fractional TTL" do
      error = client_error(state_collections: [{name: "c", kind: "value", payload: "json", ttl_seconds: 30.5}])
      expect(error.message).to match(/state_collections\[0\]\.ttl_seconds.*whole number/)
    end

    it "rejects a negative TTL" do
      error = client_error(state_collections: [{name: "c", kind: "value", payload: "json", ttl_seconds: -5}])
      expect(error.message).to match(/state_collections\[0\]\.ttl_seconds.*whole number/)
    end

    it "rejects a NaN TTL" do
      error = client_error(state_collections: [{name: "c", kind: "value", payload: "json", ttl_seconds: Float::NAN}])
      expect(error.message).to match(/state_collections\[0\]\.ttl_seconds.*whole number/)
    end

    it "rejects an infinite TTL" do
      error = client_error(state_collections: [{name: "c", kind: "value", payload: "json", ttl_seconds: Float::INFINITY}])
      expect(error.message).to match(/state_collections\[0\]\.ttl_seconds.*whole number/)
    end

    it "rejects a TTL above the u32 ceiling" do
      error = client_error(state_collections: [{name: "c", kind: "value", payload: "json", ttl_seconds: 2**32}])
      expect(error.message).to match(/state_collections\[0\]\.ttl_seconds.*whole number/)
    end

    it "rejects a keyset_limit above 4096 on a map" do
      error = client_error(state_collections: [{name: "m", kind: "map", payload: "json", keyset_limit: 5000}])
      expect(error.message).to match(/keyset_limit.*0..=4096/)
    end

    it "rejects a fractional keyset_limit on a map" do
      error = client_error(state_collections: [{name: "m", kind: "map", payload: "json", keyset_limit: 128.5}])
      expect(error.message).to match(/keyset_limit.*0..=4096/)
    end

    it "rejects a negative keyset_limit on a map" do
      error = client_error(state_collections: [{name: "m", kind: "map", payload: "json", keyset_limit: -1}])
      expect(error.message).to match(/keyset_limit.*0..=4096/)
    end

    it "rejects an infinite keyset_limit on a map" do
      error = client_error(state_collections: [{name: "m", kind: "map", payload: "json", keyset_limit: Float::INFINITY}])
      expect(error.message).to match(/keyset_limit.*0..=4096/)
    end

    it "rejects keyset_limit on a non-map collection" do
      error = client_error(state_collections: [{name: "v", kind: "value", payload: "json", keyset_limit: 128}])
      expect(error.message).to match(/keyset_limit.*only valid for map/)
    end

    it "accepts keyset_limit 0 on a map" do
      error = client_error(state_collections: [{name: "m", kind: "map", payload: "json", keyset_limit: 0}])
      expect(error).to be_nil
    end

    it "rejects an unknown kind token" do
      error = client_error(state_collections: [{name: "c", kind: "set", payload: "json"}])
      expect(error.message).to match(/state_collections\[0\]\.kind.*expected/)
    end

    it "rejects an unknown payload token" do
      error = client_error(state_collections: [{name: "c", kind: "value", payload: "proto"}])
      expect(error.message).to match(/state_collections\[0\]\.payload.*expected/)
    end

    it "rejects a zero recovery delay" do
      error = client_error(state_recovery_delay: 0)
      expect(error.message).to match(/state_recovery_delay.*whole number/)
    end

    it "rejects a fractional recovery delay" do
      error = client_error(state_recovery_delay: 0.5)
      expect(error.message).to match(/state_recovery_delay.*whole number/)
    end

    it "rejects a negative recovery delay" do
      error = client_error(state_recovery_delay: -1)
      expect(error.message).to match(/state_recovery_delay.*whole number/)
    end

    it "rejects a NaN recovery delay" do
      error = client_error(state_recovery_delay: Float::NAN)
      expect(error.message).to match(/state_recovery_delay.*whole number/)
    end

    it "rejects an infinite recovery delay" do
      error = client_error(state_recovery_delay: Float::INFINITY)
      expect(error.message).to match(/state_recovery_delay.*whole number/)
    end

    it "rejects an empty cache dir" do
      error = client_error(state_cache_dir: "")
      expect(error.message).to match(/state_cache_dir.*empty/)
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

    it "classifies NullValueError as a transient state error" do
      error = Prosody::NullValueError.new("boom")
      expect(error).to be_a(Prosody::TransientStateError)
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

    it "builds a deque definition" do
      expect(Prosody.deque("events").kind).to eq("deque")
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

  describe "Prosody::State::VEND routing table" do
    {
      %w[value json] => [:value_state, :ValueState],
      %w[map json] => [:map_state, :MapState],
      %w[deque json] => [:deque_state, :DequeState],
      %w[value message] => [:message_value_state, :ValueState],
      %w[map message] => [:message_map_state, :MapState],
      %w[deque message] => [:message_deque_state, :DequeState]
    }.each do |key, expected|
      it "routes #{key.inspect} to #{expected.inspect}" do
        expect(Prosody::State::VEND[key]).to eq(expected)
      end
    end
  end

  describe "Prosody::State::Vending#state" do
    # A stand-in that mixes in the vending module (as the native Context does)
    # and records vend calls, so routing can be exercised without a real context.
    def build_fake_context(calls)
      fake = Object.new.extend(Prosody::State::Vending)
      sentinel = Object.new
      %i[value_state map_state deque_state message_value_state message_map_state message_deque_state].each do |vend|
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

    it "routes a value definition to value_state and wraps it in ValueState" do
      calls = []
      handle = build_fake_context(calls).state(Prosody.value("cart"))
      expect(handle).to be_a(Prosody::ValueState)
      expect(calls).to eq([[:value_state, "cart"]])
    end

    it "caches vended handles per kind/payload/name" do
      calls = []
      fake = build_fake_context(calls)
      first = fake.state(Prosody.value("cart"))
      second = fake.state(Prosody.value("cart"))
      expect(second).to equal(first)
      expect(calls).to eq([[:value_state, "cart"]])
    end

    it "routes a message map definition to message_map_state and wraps it in MapState" do
      calls = []
      handle = build_fake_context(calls).state(Prosody.message_map("sessions"))
      expect(handle).to be_a(Prosody::MapState)
      expect(calls).to eq([[:message_map_state, "sessions"]])
    end

    it "raises for an unknown kind/payload pair" do
      fake = build_fake_context([])
      definition = Prosody::StateDefinition.new(
        name: "x", kind: "set", payload: "json",
        ttl_seconds: nil, read_uncommitted: nil, keyset_limit: nil
      )
      expect { fake.state(definition) }.to raise_error(Prosody::TransientStateError, /unknown collection/)
    end
  end

  describe "Ruby-side guards" do
    # A stand-in native deque that returns nil for reads and a no-op scan, so the
    # Ruby guards fire before any real native call.
    def fake_deque
      native = Object.new
      native.define_singleton_method(:get) { |_index| nil }
      native.define_singleton_method(:scan) do |_direction|
        scan = Object.new
        scan.define_singleton_method(:next) { nil }
        scan.define_singleton_method(:close) { nil }
        scan
      end
      native
    end

    it "rejects a negative deque index" do
      expect { Prosody::DequeState.new(fake_deque).get(-1) }
        .to raise_error(Prosody::TransientStateError, /index/)
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
      native.define_singleton_method(:scan) do |_direction|
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
  end
end
