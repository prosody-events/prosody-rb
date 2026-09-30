# frozen_string_literal: true

# The deque collection: the owned {Prosody::DequeState} handle and the
# {Prosody::PublishedDeque} reader.

module Prosody
  # A deque keyed-state handle.
  #
  # Traversal is explicit: {#each}/{#reverse_each} yield single elements over a
  # native scan, closing the scan via `ensure`. No aggregate-mixin methods are
  # provided.
  class DequeState
    include State::Scanning

    # @param native [Prosody::NativeJsonDequeState, Prosody::NativeMessageDequeState] the native handle
    def initialize(native)
      @native = native
    end

    # Appends an element at the back.
    #
    # @param value [Object] the element (JSON, or a message)
    # @return [void]
    # @raise [PermanentStateError] if `value` is `nil` (use {#clear} to delete)
    def push(value) = @native.push_back(value)

    # Prepends an element at the front.
    #
    # @param value [Object] the element (JSON, or a message)
    # @return [void]
    # @raise [PermanentStateError] if `value` is `nil` (use {#clear} to delete)
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
    # @return [Symbol] +:applied+ when buffered operations were written, or
    #   +:no_op+ when nothing was buffered
    def commit = @native.commit

    # Discards the buffered uncommitted operations.
    #
    # @return [Symbol] +:applied+ when buffered operations were discarded, or
    #   +:no_op+ when nothing was buffered
    def rollback = @native.rollback

    # Reads the element at `index`, resolving negatives Array-style (mirrors
    # +Array#[]+'s read domain, without the indexer). A non-negative index
    # reads from the front; `-1` is the back element, `-n` the nth from the end.
    # `-1` fast-paths through {#last} (no length read); other negatives resolve
    # against the current length (one length read + one element read),
    # consistent because the deque has a single writer per attempt.
    #
    # @param index [Integer] the position (negative counts from the back)
    # @return [Object, nil] the element, or `nil` outside the bounds
    # @raise [TransientStateError] if `index` is not an Integer
    def get(index)
      unless index.is_a?(Integer)
        raise TransientStateError, "get: index must be an Integer, got #{index.inspect}"
      end
      index.negative? ? at_negative(index) : @native.get(index)
    end

    # Traverses the live elements in index order.
    #
    # Without a block, returns an {Enumerator} over the native scan. Each step
    # fiber-yields; the scan is closed via `ensure` on stop or exception.
    #
    # Accepts the position keywords documented on {State::Scanning}. Positions
    # count from the front and must be non-negative. To read from the back,
    # use {#reverse_each} with `limit:`.
    #
    # @param query [Hash] optional `from:`, `after:`, `to:`, `before:`,
    #   `range:`, and `limit:` keywords
    # @yieldparam element [Object]
    # @return [Enumerator, void]
    # @raise [ArgumentError, TypeError] if a query keyword is invalid
    def each(**query, &block) = traverse(:scan, :forward, query, &block)

    # Traverses the live elements in reverse index order.
    #
    # @param query [Hash] optional position keywords, as on {#each}
    # @yieldparam element [Object]
    # @return [Enumerator, void]
    def reverse_each(**query, &block) = traverse(:scan, :backward, query, &block)

    # --- idiomatic Array-style conveniences -----------------------------
    # Composed from the canonical ops above; bounded reads only (no +to_a+,
    # +map+, +sort+, or +Enumerable+ that would materialize the whole deque).
    #
    # Deliberately NOT provided: +[]+ and +at+. This is a remote deque; +get+
    # and +fetch+ accept a single +Integer+ index (negatives resolve from the
    # back, Array-style), but wearing +Array+'s +[]+/+at+ would invite a range
    # read (+deque[0..2]+) that cannot be honored. Use the explicit {#get}, or
    # {#first}/{#last} for the ends.

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

    # Appends +value+ and returns +self+ (mirrors +Array#<<+). Alias of
    # {#append}.
    alias_method :<<, :append

    # The front element, or +nil+ when empty (mirrors +Array#first+). An
    # endpoint-slot read in one round trip (no length read). Under a TTL an
    # expired front slot yields +nil+ even when live interior elements remain —
    # a peek never searches inward.
    #
    # @return [Object, nil]
    def first = @native.peek_front

    # The back element, or +nil+ when empty (mirrors +Array#last+). An
    # endpoint-slot read in one round trip (no length read); same TTL-hole
    # semantics as {#first}.
    #
    # @return [Object, nil]
    def last = @native.peek_back

    # Reads the element at +index+, raising or defaulting when out of range
    # (mirrors +Array#fetch+). A +nil+ result is unambiguously "out of range"
    # under the null ban. A negative index counts from the back, as in {#get}.
    # A non-Integer index raises {TransientStateError}.
    #
    # @param index [Integer] the position (negative counts from the back)
    # @param default [Object] returned when +index+ is out of range
    # @yieldparam index [Integer] called (instead of +default+) when out of range
    # @return [Object]
    # @raise [IndexError] when out of range and no default or block is given
    # @raise [TransientStateError] if +index+ is not an Integer
    def fetch(index, *default, &block)
      if default.length > 1
        raise ArgumentError, "wrong number of arguments (given #{default.length + 1}, expected 1..2)"
      end
      unless index.is_a?(Integer)
        raise TransientStateError, "fetch: index must be an Integer, got #{index.inspect}"
      end
      warn "warning: block supersedes default value argument" if block && !default.empty?

      value = get(index)
      case value
      when nil
        return block.call(index) if block
        raise IndexError, "index #{index} outside deque bounds" if default.empty?

        # Steep merges the overloads, so it cannot type the default as D.
        default.fetch(0) #: untyped
      else
        value
      end
    end

    private

    # Resolves a negative Array-style index against the current length: +-1+
    # fast-paths through {#last} (no length read), other negatives read the
    # length and index from the front. Returns +nil+ when the index resolves
    # before the front (past the far end of the deque).
    def at_negative(index)
      return @native.peek_back if index == -1

      resolved = @native.len + index
      resolved.negative? ? nil : @native.get(resolved)
    end
  end

  # A read-only view of a published deque, opened by
  # +client.state(subsystem, definition)+. Each read takes the user key and
  # sees only committed state. A failed read raises the same state errors as
  # an owned handle. A read in a forked child process raises +RuntimeError+.
  class PublishedDeque
    include State::Scanning

    # @param native [Prosody::NativePublishedDeque] the native reader
    def initialize(native) = @native = native

    # Reads the committed element at +index+ in the deque for +key+. A
    # negative index counts from the back, as in {DequeState#get}.
    #
    # @param index [Integer] the position (negative counts from the back)
    # @return [Object, nil] the element, or +nil+ outside the bounds
    # @raise [TransientStateError] if +index+ is not an Integer
    def get(key, index)
      unless index.is_a?(Integer)
        raise TransientStateError, "get: index must be an Integer, got #{index.inspect}"
      end

      return @native.get(key, index) unless index.negative?
      # This repeats DequeState#at_negative, because a reader needs the key.
      return last(key) if index == -1

      resolved = length(key) + index
      resolved.negative? ? nil : @native.get(key, resolved)
    end

    # The number of committed elements in the deque for +key+.
    #
    # @return [Integer]
    def length(key) = @native.length(key)
    alias_method :size, :length

    # Whether the deque for +key+ has no committed elements.
    #
    # @return [Boolean]
    def empty?(key) = @native.is_empty(key)

    # The front element of the deque for +key+.
    #
    # @return [Object, nil] the element, or +nil+ when the deque is empty
    def first(key) = @native.peek_front(key)

    # The back element of the deque for +key+.
    #
    # @return [Object, nil] the element, or +nil+ when the deque is empty
    def last(key) = @native.peek_back(key)

    # Traverses the committed elements for +key+ from front to back. Each
    # traversal accepts the position keywords documented on {State::Scanning}.
    #
    # @return [Enumerator, void]
    def each(key, **query, &block) = traverse(:scan, key, :forward, query, &block)

    # Traverses the committed elements for +key+ from back to front.
    #
    # @return [Enumerator, void]
    def reverse_each(key, **query, &block) = traverse(:scan, key, :backward, query, &block)
  end
end
