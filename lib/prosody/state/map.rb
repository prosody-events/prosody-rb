# frozen_string_literal: true

# The map collection: the owned {Prosody::MapState} handle and the
# {Prosody::PublishedMap} reader.

module Prosody
  # A `String`-keyed ordered-map keyed-state handle.
  #
  # Traversal is explicit: {#each_pair}/{#reverse_each_pair} yield `key, value`
  # pairs over a native scan, closing the scan via `ensure`. No aggregate-mixin
  # methods are provided — they would silently materialize an unbounded remote
  # collection.
  class MapState
    include State::Scanning

    # @param native [Prosody::NativeJsonMapState, Prosody::NativeMessageMapState] the native handle
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

    # Tests several keys for presence in a single batch. Like {#key?}, it
    # decodes no values.
    #
    # @param keys [Array<String>] the keys to test, in order
    # @return [Array<Boolean>] one result per input key
    def contains_many(keys) = @native.contains_many(keys)

    # Whether the map holds no live entries (mirrors +Hash#empty?+).
    #
    # @return [Boolean]
    def empty? = @native.is_empty

    # Inserts or overwrites `key`.
    #
    # @param key [String] the map key
    # @param value [Object] the value to store (JSON, or a message)
    # @return [void]
    # @raise [PermanentStateError] if `value` is `nil` (use {#delete} to remove)
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
    # @return [Symbol] +:applied+ when buffered operations were written, or
    #   +:no_op+ when nothing was buffered
    def commit = @native.commit

    # Discards the buffered uncommitted operations.
    #
    # @return [Symbol] +:applied+ when buffered operations were discarded, or
    #   +:no_op+ when nothing was buffered
    def rollback = @native.rollback

    # Traverses the live entries in key order, yielding `key, value`.
    #
    # Without a block, returns an {Enumerator} over the native scan. Each step
    # fiber-yields; the scan is closed via `ensure` on stop or exception. The
    # enumerator is valid only within the current handler invocation.
    #
    # Every map traversal accepts the query keywords documented on
    # {State::Scanning}. For keyset paging, pass the last key of the previous
    # page as `after:` and the page size as `limit:`.
    #
    # @param query [Hash] optional `from:`, `after:`, `to:`, `before:`,
    #   `range:`, `prefix:`, and `limit:` keywords
    # @yieldparam key [String]
    # @yieldparam value [Object]
    # @return [Enumerator, void]
    # @raise [ArgumentError, TypeError] if a query keyword is invalid
    def each_pair(**query, &block) = traverse(:forward, query, &block)

    # Traverses the live entries in reverse key order, yielding `key, value`.
    #
    # @param query [Hash] optional query keywords, as on {#each_pair}
    # @yieldparam key [String]
    # @yieldparam value [Object]
    # @return [Enumerator, void]
    def reverse_each_pair(**query, &block) = traverse(:backward, query, &block)

    # Traverses the live keys in key order, yielding each key (mirrors
    # +Hash#each_key+). The key scan skips value decode and the resolver — a
    # message-backed map yields keys with zero Kafka fetches, though not
    # zero-I/O. Without a block, returns a demand-driven {Enumerator}; there is
    # deliberately no eager +keys+ array (it would materialize the whole remote
    # keyset). Mirrors {#each_pair}'s block-form return (+nil+), not stdlib's
    # +self+, for in-repo sibling consistency.
    #
    # @param query [Hash] optional query keywords, as on {#each_pair}
    # @yieldparam key [String]
    # @return [Enumerator, void]
    def each_key(**query, &block) = traverse_keys(:forward, query, &block)

    # Traverses the live keys in reverse key order, yielding each key.
    #
    # @param query [Hash] optional query keywords, as on {#each_pair}
    # @yieldparam key [String]
    # @return [Enumerator, void]
    def reverse_each_key(**query, &block) = traverse_keys(:backward, query, &block)

    # Traverses the live values in key order (mirrors +Hash#each_value+).
    #
    # @param query [Hash] optional query keywords, as on {#each_pair}
    # @yieldparam value [Object]
    # @return [Enumerator, void]
    def each_value(**query, &block) = traverse_values(:forward, query, &block)

    # Traverses the live values in reverse key order.
    #
    # @param query [Hash] optional query keywords, as on {#each_pair}
    # @yieldparam value [Object]
    # @return [Enumerator, void]
    def reverse_each_value(**query, &block) = traverse_values(:backward, query, &block)

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
      raise KeyError.new("key not found: #{key.inspect}", key: key, receiver: self) if default.empty?

      # Steep merges the overloads, so it cannot type the default as D.
      default.fetch(0) #: untyped
    end

    # Whether +key+ has a live value (mirrors +Hash#key?+). A presence check:
    # no value decode and no resolver run (not no-I/O). A message-backed map
    # answers presence with zero Kafka fetches — +true+ even for an entry
    # whose message cannot be fetched — though a cache miss may still touch
    # the store.
    #
    # @param key [String]
    # @return [Boolean]
    def key?(key) = @native.contains_key(key)
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
    def slice(*keys) = keys.zip(get_many(keys)).to_h.compact

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
        case value
        when nil
          next block.call(key) if block

          raise KeyError.new("key not found: #{key.inspect}", key: key, receiver: self)
        else
          value
        end
      end
    end

    private

    def traverse(direction, query)
      return enum_for(:traverse, direction, query) unless block_given?

      # Yield the [key, value] pair as a single Array, matching Hash#each_pair:
      # a two-parameter block auto-splats it (|k, v|), a one-parameter block
      # receives the pair (|pair|), and the no-block Enumerator yields pairs.
      scan_each(direction, query) { |pair| yield pair }
    end

    def traverse_keys(direction, query)
      return enum_for(:traverse_keys, direction, query) unless block_given?

      scan_each(direction, query, :keys) { |key| yield key }
    end

    def traverse_values(direction, query)
      return enum_for(:traverse_values, direction, query) unless block_given?

      scan_each(direction, query) { |entry| yield entry[1] }
    end
  end

  # A read-only view of a published map, opened by
  # +client.state(subsystem, definition)+. Each read takes the user key and
  # sees only committed state. A read raises +RuntimeError+ when it fails or
  # when it runs in a forked child process.
  class PublishedMap
    include State::Scanning

    # @param native [Prosody::NativePublishedMap] the native reader
    def initialize(native) = @native = native

    # Reads the committed entry for +map_key+ in the map for +key+.
    #
    # @return [Object, nil] the value, or +nil+ when the entry is absent
    def get(key, map_key) = @native.get(key.to_s, map_key.to_s)

    # Reads several entries of the map for +key+ in one batch.
    #
    # @return [Array<Object, nil>] one result per map key; +nil+ for an absent entry
    def get_many(key, map_keys) = @native.get_many(key.to_s, map_keys.map(&:to_s))

    # Whether the map for +key+ has a committed entry for +map_key+.
    #
    # @return [Boolean]
    def key?(key, map_key) = @native.contains_key(key.to_s, map_key.to_s)
    alias_method :has_key?, :key?
    alias_method :include?, :key?
    alias_method :member?, :key?

    # Tests several entries of the map for +key+ in one batch.
    #
    # @return [Array<Boolean>] one result per map key
    def contains_many(key, map_keys) = @native.contains_many(key.to_s, map_keys.map(&:to_s))

    # Whether the map for +key+ has no committed entries.
    #
    # @return [Boolean]
    def empty?(key) = @native.is_empty(key.to_s)

    # Traverses the committed entries for +key+ in key order, yielding one
    # +[map_key, value]+ pair for each entry. Each traversal accepts the query
    # keywords documented on {State::Scanning}.
    #
    # @return [Enumerator, void]
    def each_pair(key, **query, &block) = traverse(key, :forward, query, &block)

    # Traverses the committed entries for +key+ in reverse key order.
    #
    # @return [Enumerator, void]
    def reverse_each_pair(key, **query, &block) = traverse(key, :backward, query, &block)

    # Traverses the committed map keys for +key+ in key order.
    #
    # @return [Enumerator, void]
    def each_key(key, **query, &block) = traverse_keys(key, :forward, query, &block)

    # Traverses the committed map keys for +key+ in reverse key order.
    #
    # @return [Enumerator, void]
    def reverse_each_key(key, **query, &block) = traverse_keys(key, :backward, query, &block)

    # Traverses the committed values for +key+ in key order.
    #
    # @return [Enumerator, void]
    def each_value(key, **query, &block) = traverse_values(key, :forward, query, &block)

    # Traverses the committed values for +key+ in reverse key order.
    #
    # @return [Enumerator, void]
    def reverse_each_value(key, **query, &block) = traverse_values(key, :backward, query, &block)
    alias_method :each, :each_pair

    private

    def traverse(key, direction, query)
      return enum_for(:traverse, key, direction, query) unless block_given?

      scan_items(@native.scan(key.to_s, direction, query)) { |entry| yield entry }
    end

    def traverse_keys(key, direction, query)
      return enum_for(:traverse_keys, key, direction, query) unless block_given?

      scan_items(@native.keys(key.to_s, direction, query)) { |map_key| yield map_key }
    end

    def traverse_values(key, direction, query)
      return enum_for(:traverse_values, key, direction, query) unless block_given?

      scan_items(@native.scan(key.to_s, direction, query)) { |entry| yield entry[1] }
    end
  end
end
