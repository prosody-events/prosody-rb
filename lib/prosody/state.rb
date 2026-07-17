# frozen_string_literal: true

module Prosody
  # Base class for errors raised by keyed-state operations that will not
  # succeed on retry (an unregistered collection name, an identity mismatch, a
  # duplicate registration, an invalid TTL).
  #
  # It subclasses {PermanentError} so a rethrown state error is classified as
  # permanent by the result bridge's `#permanent?` path with no bridge change.
  #
  # @see PermanentError
  class PermanentStateError < PermanentError; end

  # Base class for errors raised by keyed-state operations that may succeed on
  # retry. Every caller/input mistake (a null write, a wrong item shape, an
  # invalid index, an invalid direction token, an unrepresentable value) is
  # transient so the message retries and stays visible rather than being
  # discarded.
  #
  # It subclasses {TransientError} so a rethrown state error is classified as
  # transient by the result bridge's `#permanent?` path with no bridge change.
  #
  # @see TransientError
  class TransientStateError < TransientError; end

  # Raised when a JSON `null` is written to a collection. `null` is not a
  # storable value (it is indistinguishable from absence), so the write is
  # rejected and the stored value is left untouched. Use `clear`/`delete` to
  # express deletion instead.
  #
  # It is transient (a caller mistake), so it retries and stays visible.
  #
  # @see TransientStateError
  class NullValueError < TransientStateError; end

  # An immutable keyed-state collection definition.
  #
  # Definitions are frozen value objects produced by the {Prosody.value},
  # {Prosody.map}, {Prosody.deque}, and their `message_*` siblings. A definition
  # both serializes into `Configuration#state_collections` (via
  # {#to_state_config}) so the collection is registered before subscribe, and
  # drives {Prosody::Context#state} to vend the matching typed handle.
  StateDefinition = Data.define(:name, :kind, :payload, :ttl_seconds, :read_uncommitted, :keyset_limit) do
    # Serializes this definition into the native-registration hash, omitting
    # unset optionals so they fall back to the core defaults.
    #
    # @return [Hash] the registration hash for the native layer
    def to_state_config
      config = {name: name, kind: kind, payload: payload}
      config[:ttl_seconds] = ttl_seconds unless ttl_seconds.nil?
      config[:read_uncommitted] = read_uncommitted unless read_uncommitted.nil?
      config[:keyset_limit] = keyset_limit unless keyset_limit.nil?
      config
    end
  end

  # Defines a single-value JSON collection.
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @return [StateDefinition] a frozen definition
  def self.value(name, ttl: nil, read_uncommitted: nil)
    StateDefinition.new(name: name.to_s, kind: "value", payload: "json",
      ttl_seconds: ttl, read_uncommitted: read_uncommitted, keyset_limit: nil)
  end

  # Defines a `String`-keyed ordered map JSON collection.
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param keyset_limit [Integer, nil] optional map-only keyset bound (`0..=4096`)
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @return [StateDefinition] a frozen definition
  def self.map(name, ttl: nil, keyset_limit: nil, read_uncommitted: nil)
    StateDefinition.new(name: name.to_s, kind: "map", payload: "json",
      ttl_seconds: ttl, read_uncommitted: read_uncommitted, keyset_limit: keyset_limit)
  end

  # Defines a deque JSON collection.
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @return [StateDefinition] a frozen definition
  def self.deque(name, ttl: nil, read_uncommitted: nil)
    StateDefinition.new(name: name.to_s, kind: "deque", payload: "json",
      ttl_seconds: ttl, read_uncommitted: read_uncommitted, keyset_limit: nil)
  end

  # Defines a single-value Kafka-message collection (items are full messages).
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @return [StateDefinition] a frozen definition
  def self.message_value(name, ttl: nil, read_uncommitted: nil)
    StateDefinition.new(name: name.to_s, kind: "value", payload: "message",
      ttl_seconds: ttl, read_uncommitted: read_uncommitted, keyset_limit: nil)
  end

  # Defines a `String`-keyed ordered map Kafka-message collection.
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param keyset_limit [Integer, nil] optional map-only keyset bound (`0..=4096`)
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @return [StateDefinition] a frozen definition
  def self.message_map(name, ttl: nil, keyset_limit: nil, read_uncommitted: nil)
    StateDefinition.new(name: name.to_s, kind: "map", payload: "message",
      ttl_seconds: ttl, read_uncommitted: read_uncommitted, keyset_limit: keyset_limit)
  end

  # Defines a deque Kafka-message collection.
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @return [StateDefinition] a frozen definition
  def self.message_deque(name, ttl: nil, read_uncommitted: nil)
    StateDefinition.new(name: name.to_s, kind: "deque", payload: "message",
      ttl_seconds: ttl, read_uncommitted: read_uncommitted, keyset_limit: nil)
  end

  # Internal routing tables shared by the state wrappers.
  module State
    # Maps a definition's `[kind, payload]` to the native vend method and the
    # public wrapper class that wraps the vended native handle.
    VEND = {
      %w[value json] => [:value_state, :ValueState],
      %w[map json] => [:map_state, :MapState],
      %w[deque json] => [:deque_state, :DequeState],
      %w[value message] => [:message_value_state, :ValueState],
      %w[map message] => [:message_map_state, :MapState],
      %w[deque message] => [:message_deque_state, :DequeState]
    }.freeze

    # Adds keyed-state vending to the native context. Included into
    # {Prosody::Context}; kept as a module so the routing can be exercised
    # against a stand-in receiver.
    module Vending
      # Vends the typed keyed-state handle for `definition`.
      #
      # Handles are cached per context by kind, payload, and name, so repeated
      # vends within one handler invocation return the same wrapper.
      #
      # @param definition [StateDefinition] a frozen collection definition
      # @return [ValueState, MapState, DequeState] the typed handle
      # @raise [TransientStateError] if the definition's kind/payload is unknown
      # @raise [PermanentStateError] if the collection name is unregistered or
      #   its registered identity mismatches
      def state(definition)
        cache = (@state_handles ||= {})
        cache_key = "#{definition.kind}:#{definition.payload}:#{definition.name}"
        return cache[cache_key] if cache.key?(cache_key)

        vend_method, wrapper = VEND.fetch([definition.kind, definition.payload]) do
          raise TransientStateError,
            "state: unknown collection kind/payload #{[definition.kind, definition.payload].inspect}"
        end
        native = public_send(vend_method, definition.name)
        cache[cache_key] = Prosody.const_get(wrapper).new(native)
      end
    end

    # Shared cursor-driving for the explicit-traversal handles. Folds the
    # identical native-scan open/close/exhaustion loop; each handle supplies
    # only the per-item yield shape through the block. Kept private (mixed into
    # the handle classes) since it is not part of the public surface.
    module Scanning
      private

      # Opens a native scan in `direction`, yields each item, and closes the
      # scan via `ensure` on stop or exception. Direction validity is enforced
      # by the native layer (an invalid token is rejected transient there); the
      # public traversal methods only ever pass `:forward`/`:backward`.
      def scan_each(direction)
        scan = @native.scan(direction.to_s)
        begin
          # `nil` is the exhaustion sentinel (unambiguous under the null ban);
          # terminate on it explicitly rather than on falsiness, so a legal
          # stored `false` (or a `[key, false]` pair, always a truthy Array)
          # does not stop iteration and drop the tail after it.
          until (item = scan.next).nil?
            yield item
          end
        ensure
          scan.close
        end
      end
    end
  end

  # A single-value keyed-state handle.
  #
  # Reads return the stored JSON value (or a {Prosody::Message} for message
  # collections), or `nil` when the value is absent. Writes are buffered and
  # made durable by {#commit}. All operations are fiber-yield async: they look
  # blocking but never block the thread.
  class ValueState
    # @param native [Prosody::NativeValueState] the vended native handle
    def initialize(native)
      @native = native
    end

    # Reads the current value.
    #
    # @return [Object, nil] the stored value, or `nil` when absent
    def get = @native.get

    # Buffers a write of the value.
    #
    # @param value [Object] the value to store (JSON, or a message)
    # @return [void]
    # @raise [NullValueError] if `value` is `nil` (use {#clear} to delete)
    def set(value) = @native.set(value)

    # Buffers a clear of the value.
    #
    # @return [void]
    def clear = @native.clear

    # Durably commits the buffered operations mid-handler.
    #
    # @return [nil] the erased FFI seam drops the applied/no-op outcome
    def commit = @native.commit

    # Discards the buffered uncommitted operations.
    #
    # @return [nil]
    def rollback = @native.rollback

    # Reads the current value. Idiomatic alias of {#get}.
    #
    # @return [Object, nil]
    alias_method :value, :get

    # Buffers a write of the value. Idiomatic alias of {#set}. As with any Ruby
    # writer, `state.value = x` evaluates to `x` regardless of the return.
    #
    # @param value [Object]
    # @return [void]
    alias_method :value=, :set
  end

  # A `String`-keyed ordered-map keyed-state handle.
  #
  # Traversal is explicit: {#each_pair}/{#reverse_each_pair} yield `key, value`
  # pairs over a native scan, closing the scan via `ensure`. No aggregate-mixin
  # methods are provided — they would silently materialize an unbounded remote
  # collection.
  class MapState
    include State::Scanning

    # @param native [Prosody::NativeMapState] the vended native handle
    def initialize(native)
      @native = native
    end

    # Reads the value for `key`.
    #
    # @param key [String] the map key
    # @return [Object, nil] the value, or `nil` when the key is absent
    def get(key) = @native.get(key)

    # Reads several keys in a single isolated batch.
    #
    # @param keys [Array<String>] the keys to read, in order
    # @return [Array<Object, nil>] one result per input key; `nil` for absent keys
    def get_many(keys) = @native.get_many(keys)

    # Inserts or overwrites `key`.
    #
    # @param key [String] the map key
    # @param value [Object] the value to store (JSON, or a message)
    # @return [void]
    # @raise [NullValueError] if `value` is `nil` (use {#delete} to remove)
    def set(key, value) = @native.set(key, value)

    # Removes `key`.
    #
    # Documented divergence from `Hash#delete`: this returns `nil`, never the
    # removed value (the erased FFI seam does not surface it).
    #
    # @param key [String] the map key
    # @return [nil]
    def delete(key)
      @native.remove(key)
      nil
    end

    # Removes every entry.
    #
    # @return [void]
    def clear = @native.clear

    # Durably commits the buffered operations mid-handler.
    #
    # @return [nil] the erased FFI seam drops the applied/no-op outcome
    def commit = @native.commit

    # Discards the buffered uncommitted operations.
    #
    # @return [nil]
    def rollback = @native.rollback

    # Traverses the live entries in key order, yielding `key, value`.
    #
    # Without a block, returns an {Enumerator} over the native scan. Each step
    # fiber-yields; the scan is closed via `ensure` on stop or exception. The
    # enumerator is valid only within the current handler invocation.
    #
    # @yieldparam key [String]
    # @yieldparam value [Object]
    # @return [Enumerator, void]
    def each_pair(&block) = traverse(:forward, &block)

    # Traverses the live entries in reverse key order, yielding `key, value`.
    #
    # @yieldparam key [String]
    # @yieldparam value [Object]
    # @return [Enumerator, void]
    def reverse_each_pair(&block) = traverse(:backward, &block)

    # --- idiomatic Hash-style aliases and conveniences ------------------
    # Each is composed from the canonical ops above and adds no capability
    # the naming matrix lacks. Bounded reads only: there is deliberately no
    # +keys+/+values+/+to_h+/+count+ or +Enumerable+, which would materialize
    # the whole (potentially unbounded) remote collection.

    # Reads +key+. Idiomatic alias of {#get} (mirrors +Hash#[]+).
    alias_method :[], :get

    # Writes +key+. Idiomatic alias of {#set} (mirrors +Hash#[]=+). As with any
    # Ruby +[]=+, `map[key] = value` evaluates to +value+ regardless of return.
    alias_method :[]=, :set

    # Writes +key+, returning the stored +value+ (mirrors +Hash#store+). A
    # wrapper, not an alias: unlike +[]=+, +store+ is called normally, so its
    # return is observed — and the native write returns +nil+.
    #
    # @param key [String]
    # @param value [Object]
    # @return [Object] the stored +value+
    def store(key, value)
      set(key, value)
      value
    end

    # Traverses live entries in key order. Idiomatic alias of {#each_pair}
    # (mirrors +Hash#each+).
    alias_method :each, :each_pair

    # Reads several keys positionally (mirrors +Hash#values_at+).
    #
    # @param keys [Array<String>] the keys to read
    # @return [Array<Object, nil>] one result per key; +nil+ for absent keys
    def values_at(*keys) = get_many(keys)

    # Reads +key+, raising or defaulting when absent (mirrors +Hash#fetch+).
    # Performs a single read; a +nil+ result is unambiguously "absent" under
    # the null ban.
    #
    # @param key [String]
    # @param default [Object] returned when +key+ is absent
    # @yieldparam key [String] called (instead of +default+) when +key+ is absent
    # @return [Object]
    # @raise [KeyError] when +key+ is absent and no default or block is given
    def fetch(key, *default, &block)
      if default.length > 1
        raise ArgumentError, "wrong number of arguments (given #{default.length + 1}, expected 1..2)"
      end
      warn "warning: block supersedes default value argument" if block && !default.empty?

      value = @native.get(key)
      return value unless value.nil?
      return block.call(key) if block
      return default.first unless default.empty?

      raise KeyError.new("key not found: #{key.inspect}", key: key, receiver: self)
    end

    # Whether +key+ has a live value (mirrors +Hash#key?+). Performs one read.
    #
    # @param key [String]
    # @return [Boolean]
    def key?(key) = !@native.get(key).nil?
    alias_method :has_key?, :key?
    alias_method :include?, :key?
    alias_method :member?, :key?

    # Reads +key+ and digs into the nested value (mirrors +Hash#dig+). A single
    # bounded read; digging continues in the returned local value.
    #
    # @param key [String]
    # @return [Object, nil]
    # @raise [TypeError] if a nested value does not respond to +dig+
    def dig(key, *rest)
      value = @native.get(key)
      return value if rest.empty? || value.nil?

      unless value.respond_to?(:dig)
        raise TypeError, "#{value.class} does not have #dig method"
      end

      value.dig(*rest)
    end

    # Reads +keys+ as a single bounded batch, returning a +Hash+ of only the
    # keys that are present (mirrors +Hash#slice+). Absent keys are omitted.
    #
    # @param keys [Array<String>] the keys to read
    # @return [Hash{String => Object}] present keys mapped to their values
    def slice(*keys)
      result = {}
      keys.zip(get_many(keys)) do |key, value|
        result[key] = value unless value.nil?
      end
      result
    end

    # Reads +keys+ as a single bounded batch, requiring every key to be present
    # (mirrors +Hash#fetch_values+). Without a block, a missing key raises
    # {KeyError}; with a block, the block is called with each missing key and
    # its result substituted.
    #
    # @param keys [Array<String>] the keys to read, in order
    # @yieldparam key [String] called for each absent key
    # @return [Array<Object>] one value per key, in order
    # @raise [KeyError] when a key is absent and no block is given
    def fetch_values(*keys, &block)
      keys.zip(get_many(keys)).map do |key, value|
        next value unless value.nil?
        next block.call(key) if block

        raise KeyError.new("key not found: #{key.inspect}", key: key, receiver: self)
      end
    end

    private

    def traverse(direction)
      return enum_for(:traverse, direction) unless block_given?

      # Yield the [key, value] pair as a single Array, matching Hash#each_pair:
      # a two-parameter block auto-splats it (|k, v|), a one-parameter block
      # receives the pair (|pair|), and the no-block Enumerator yields pairs.
      scan_each(direction) { |pair| yield pair }
    end
  end

  # A deque keyed-state handle.
  #
  # Traversal is explicit: {#each}/{#reverse_each} yield single elements over a
  # native scan, closing the scan via `ensure`. No aggregate-mixin methods are
  # provided.
  class DequeState
    include State::Scanning

    # @param native [Prosody::NativeDequeState] the vended native handle
    def initialize(native)
      @native = native
    end

    # Appends an element at the back.
    #
    # @param value [Object] the element (JSON, or a message)
    # @return [void]
    # @raise [NullValueError] if `value` is `nil`
    def push(value) = @native.push_back(value)

    # Prepends an element at the front.
    #
    # @param value [Object] the element (JSON, or a message)
    # @return [void]
    # @raise [NullValueError] if `value` is `nil`
    def unshift(value) = @native.push_front(value)

    # Removes and returns the back element.
    #
    # @return [Object, nil] the removed element, or `nil` when empty
    def pop = @native.pop_back

    # Removes and returns the front element.
    #
    # @return [Object, nil] the removed element, or `nil` when empty
    def shift = @native.pop_front

    # The number of live elements.
    #
    # @return [Integer]
    def length = @native.len

    alias_method :size, :length

    # Whether the deque holds no live elements.
    #
    # @return [Boolean]
    def empty? = @native.is_empty

    # Removes every element.
    #
    # @return [void]
    def clear = @native.clear

    # Durably commits the buffered operations mid-handler.
    #
    # @return [nil] the erased FFI seam drops the applied/no-op outcome
    def commit = @native.commit

    # Discards the buffered uncommitted operations.
    #
    # @return [nil]
    def rollback = @native.rollback

    # Reads the element at front-relative position `index`.
    #
    # @param index [Integer] the zero-based position from the front
    # @return [Object, nil] the element, or `nil` past the end
    # @raise [TransientStateError] if `index` is not a non-negative Integer
    def get(index)
      unless index.is_a?(Integer) && index >= 0
        raise TransientStateError, "get: index must be a non-negative Integer, got #{index.inspect}"
      end
      @native.get(index)
    end

    # Traverses the live elements in index order.
    #
    # Without a block, returns an {Enumerator} over the native scan. Each step
    # fiber-yields; the scan is closed via `ensure` on stop or exception.
    #
    # @yieldparam element [Object]
    # @return [Enumerator, void]
    def each(&block) = traverse(:forward, &block)

    # Traverses the live elements in reverse index order.
    #
    # @yieldparam element [Object]
    # @return [Enumerator, void]
    def reverse_each(&block) = traverse(:backward, &block)

    # --- idiomatic Array-style conveniences -----------------------------
    # Composed from the canonical ops above; bounded reads only (no +to_a+,
    # +map+, +sort+, or +Enumerable+ that would materialize the whole deque).
    #
    # Deliberately NOT provided: +[]+ and +at+. This is a remote, forward-only
    # deque — it supports a single non-negative +Integer+ index and neither
    # negative indices nor ranges. Wearing +Array+'s +[]+/+at+ would invite
    # +deque[-1]+/+deque[0..2]+, which cannot be honored; use the explicit
    # {#get}, or {#first}/{#last} for the ends.

    # Prepends +value+, returning +self+ for chaining (mirrors +Array#prepend+).
    # A wrapper, not an alias: the native write returns +nil+.
    #
    # @param value [Object]
    # @return [self]
    def prepend(value)
      unshift(value)
      self
    end

    # Appends +value+, returning +self+ for chaining (mirrors +Array#append+).
    # A wrapper, not an alias: the native write returns +nil+.
    #
    # @param value [Object]
    # @return [self]
    def append(value)
      push(value)
      self
    end

    # Appends +value+ at the back and returns +self+ for chaining
    # (mirrors +Array#<<+).
    #
    # @param value [Object]
    # @return [self]
    def <<(value)
      push(value)
      self
    end

    # The front element, or +nil+ when empty (mirrors +Array#first+).
    #
    # @return [Object, nil]
    def first = @native.get(0)

    # The back element, or +nil+ when empty (mirrors +Array#last+). Performs
    # two reads (length, then the last element); they are consistent because
    # keyed state is single-owner within one handler attempt, so no concurrent
    # writer can mutate the deque between them.
    #
    # @return [Object, nil]
    def last
      count = @native.len
      return nil if count.zero?

      @native.get(count - 1)
    end

    # Reads the element at +index+, raising or defaulting past the end
    # (mirrors +Array#fetch+). A +nil+ result is unambiguously "past the end"
    # under the null ban.
    #
    # Divergence from +Array#fetch+: +index+ must be a single non-negative
    # +Integer+ (this is a remote, forward-only deque). A negative or fractional
    # index raises {TransientStateError}, matching {#get}'s domain — not the
    # negative-from-the-end lookup +Array#fetch+ performs.
    #
    # @param index [Integer] zero-based front-relative position
    # @param default [Object] returned when +index+ is out of range
    # @yieldparam index [Integer] called (instead of +default+) when out of range
    # @return [Object]
    # @raise [IndexError] when out of range and no default or block is given
    # @raise [TransientStateError] if +index+ is not a non-negative Integer
    def fetch(index, *default, &block)
      if default.length > 1
        raise ArgumentError, "wrong number of arguments (given #{default.length + 1}, expected 1..2)"
      end
      unless index.is_a?(Integer) && index >= 0
        raise TransientStateError, "fetch: index must be a non-negative Integer, got #{index.inspect}"
      end
      warn "warning: block supersedes default value argument" if block && !default.empty?

      value = @native.get(index)
      return value unless value.nil?
      return block.call(index) if block
      return default.first unless default.empty?

      raise IndexError, "index #{index} outside deque bounds"
    end

    private

    def traverse(direction)
      return enum_for(:traverse, direction) unless block_given?

      scan_each(direction) { |item| yield item }
    end
  end

  # Reopens the native context class to add keyed-state vending.
  class Context
    include State::Vending
  end
end
