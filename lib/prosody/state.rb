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
    # The scan directions accepted by traversal methods.
    SCAN_DIRECTIONS = %i[forward backward].freeze

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
  end

  # A `String`-keyed ordered-map keyed-state handle.
  #
  # Traversal is explicit: {#each_pair}/{#reverse_each_pair} yield `key, value`
  # pairs over a native scan, closing the scan via `ensure`. No aggregate-mixin
  # methods are provided — they would silently materialize an unbounded remote
  # collection.
  class MapState
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

    private

    def traverse(direction)
      return enum_for(:traverse, direction) unless block_given?

      scan = open_scan(direction)
      begin
        while (pair = scan.next)
          yield pair[0], pair[1]
        end
      ensure
        scan.close
      end
    end

    def open_scan(direction)
      unless State::SCAN_DIRECTIONS.include?(direction)
        raise TransientStateError, "scan: direction must be :forward or :backward, got #{direction.inspect}"
      end
      @native.scan(direction.to_s)
    end
  end

  # A deque keyed-state handle.
  #
  # Traversal is explicit: {#each}/{#reverse_each} yield single elements over a
  # native scan, closing the scan via `ensure`. No aggregate-mixin methods are
  # provided.
  class DequeState
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

    private

    def traverse(direction)
      return enum_for(:traverse, direction) unless block_given?

      scan = open_scan(direction)
      begin
        while (item = scan.next)
          yield item
        end
      ensure
        scan.close
      end
    end

    def open_scan(direction)
      unless State::SCAN_DIRECTIONS.include?(direction)
        raise TransientStateError, "scan: direction must be :forward or :backward, got #{direction.inspect}"
      end
      @native.scan(direction.to_s)
    end
  end

  # Reopens the native context class to add keyed-state vending.
  class Context
    include State::Vending
  end
end
