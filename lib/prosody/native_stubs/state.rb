# frozen_string_literal: true

# Native stubs for the owned keyed-state handles and their scan cursors. The
# Ruby handles in lib/prosody/state wrap these classes.

module Prosody
  # Native single-value keyed-state handle, vended by the context and wrapped by
  # {Prosody::ValueState}. Every operation is fiber-yield async: it crosses the
  # bridge and yields the fiber while the Rust core drives the operation.
  #
  # @see ext/prosody/src/handler/state/mod.rs for implementation
  module NativeValueOperations
    # @private
    def initialize
      raise NotImplementedError, "This class is implemented natively in Rust"
    end

    # Reads the current value.
    #
    # @return [Object, nil] the stored value, or nil when absent
    def get
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Buffers a write of the value.
    #
    # @param value [Object] the value to store
    # @return [void]
    # @raise [NullValueError] if value is nil
    # @raise [TransientStateError] if value cannot be represented
    def set(value)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Buffers a clear of the value.
    #
    # @return [void]
    def clear
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Durably commits the buffered operations mid-handler.
    #
    # @return [Symbol] +:applied+ or +:no_op+
    def commit
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Discards the buffered uncommitted operations.
    #
    # @return [Symbol] +:applied+ or +:no_op+
    def rollback
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  class NativeJsonValueState
    include NativeValueOperations
  end

  class NativeMessageValueState
    include NativeValueOperations
  end

  # Native String-keyed ordered-map keyed-state handle, vended by the context and
  # wrapped by {Prosody::MapState}. Every operation is fiber-yield async, except
  # +#scan+, which opens the cursor synchronously; each native cursor pull
  # yields the fiber.
  #
  # @see ext/prosody/src/handler/state/mod.rs for implementation
  module NativeMapOperations
    # @private
    def initialize
      raise NotImplementedError, "This class is implemented natively in Rust"
    end

    # Reads the value for a key.
    #
    # @param key [String] the map key
    # @return [Object, nil] the value, or nil when the key is absent
    def get(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Answers whether a stored cell exists for a key. No value decode and no
    # resolver run (a message-backed map answers with zero Kafka fetches), but
    # not no-I/O: a cache miss still reads the store.
    #
    # @param key [String] the map key
    # @return [Boolean] whether a live cell exists for the key
    def contains_key(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Whether the map holds no live entries.
    #
    # @return [Boolean]
    def is_empty
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Reads several keys in a single isolated batch.
    #
    # @param keys [Array<String>] the keys to read, in order
    # @return [Array<Object, nil>] one result per input key
    def get_many(keys)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Tests several keys for presence in a single batch, without value decode.
    #
    # @param keys [Array<String>] the keys to test, in order
    # @return [Array<Boolean>] one result per input key
    def contains_many(keys)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Inserts or overwrites a key.
    #
    # @param key [String] the map key
    # @param value [Object] the value to store
    # @return [void]
    # @raise [NullValueError] if value is nil
    # @raise [TransientStateError] if value cannot be represented
    def set(key, value)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Removes a key.
    #
    # @param key [String] the map key
    # @return [void]
    def remove(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Removes every entry.
    #
    # @return [void]
    def clear
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Opens a native ordered scan over the live entries.
    #
    # @param direction [Symbol] +:forward+ or +:backward+
    # @param query [Hash] optional query keywords (see {Prosody::State::Scanning})
    # @return [Object] the native cursor
    # @raise [TransientStateError] if direction is not +:forward+ or +:backward+
    # @raise [ArgumentError, TypeError] if a query keyword is invalid
    def scan(direction, query = {})
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Opens a native ordered scan over the live keys only, yielding bare keys.
    # Skips value decode and the resolver (a message-backed map enumerates keys
    # with zero Kafka fetches), though not no-I/O.
    #
    # @param direction [Symbol] +:forward+ or +:backward+
    # @param query [Hash] optional query keywords (see {Prosody::State::Scanning})
    # @return [NativeMapKeyScan] the native key cursor
    # @raise [TransientStateError] if direction is not +:forward+ or +:backward+
    # @raise [ArgumentError, TypeError] if a query keyword is invalid
    def keys(direction, query = {})
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Durably commits the buffered operations mid-handler.
    #
    # @return [Symbol] +:applied+ or +:no_op+
    def commit
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Discards the buffered uncommitted operations.
    #
    # @return [Symbol] +:applied+ or +:no_op+
    def rollback
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  class NativeJsonMapState
    include NativeMapOperations
  end

  class NativeMessageMapState
    include NativeMapOperations
  end

  # Native presence-only set handle, vended by the context and wrapped by
  # {Prosody::SetState}. Every operation is fiber-yield async, except +#keys+,
  # which opens the cursor synchronously; each native cursor pull yields the
  # fiber.
  #
  # @see ext/prosody/src/handler/state/set.rs for implementation
  class NativeSetState
    # @private
    def initialize
      raise NotImplementedError, "This class is implemented natively in Rust"
    end

    # Whether a member belongs to the set.
    #
    # @param member [String]
    # @return [Boolean]
    def contains(member)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Tests several members in a single batch.
    #
    # @param members [Array<String>] the members to test, in order
    # @return [Array<Boolean>] one result per input member
    def contains_many(members)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Whether the set has no live members.
    #
    # @return [Boolean]
    def is_empty
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Buffers an insert of a member.
    #
    # @param member [String]
    # @return [void]
    def insert(member)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Buffers a removal of a member. Removing an absent member does nothing.
    #
    # @param member [String]
    # @return [void]
    def remove(member)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Removes every member.
    #
    # @return [void]
    def clear
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Opens a native ordered scan over the live members.
    #
    # @param direction [Symbol] +:forward+ or +:backward+
    # @param query [Hash] optional query keywords (see {Prosody::State::Scanning})
    # @return [NativeMapKeyScan] the native member cursor
    # @raise [TransientStateError] if direction is not +:forward+ or +:backward+
    # @raise [ArgumentError, TypeError] if a query keyword is invalid
    def keys(direction, query = {})
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Durably commits the buffered operations mid-handler.
    #
    # @return [Symbol] +:applied+ or +:no_op+
    def commit
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Discards the buffered uncommitted operations.
    #
    # @return [Symbol] +:applied+ or +:no_op+
    def rollback
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  # Native deque keyed-state handle, vended by the context and wrapped by
  # {Prosody::DequeState}. Every operation is fiber-yield async, except +#scan+,
  # which opens the cursor synchronously; each native cursor pull yields the
  # fiber.
  #
  # @see ext/prosody/src/handler/state/mod.rs for implementation
  module NativeDequeOperations
    # @private
    def initialize
      raise NotImplementedError, "This class is implemented natively in Rust"
    end

    # The number of live elements.
    #
    # @return [Integer]
    def len
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Whether the deque holds no live elements.
    #
    # @return [Boolean]
    def is_empty
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Reads the element at front-relative position.
    #
    # @param index [Integer] the zero-based position from the front
    # @return [Object, nil] the element, or nil past the end
    def get(index)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Reads the front endpoint slot, or nil when empty. One round trip, no
    # length read; an expired endpoint slot under a TTL yields nil even when
    # live interior elements remain (a peek never searches inward).
    #
    # @return [Object, nil] the front element, or nil when empty
    def peek_front
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Reads the back endpoint slot, or nil when empty. Same endpoint-slot
    # semantics as #peek_front.
    #
    # @return [Object, nil] the back element, or nil when empty
    def peek_back
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Appends an element at the back.
    #
    # @param value [Object] the element
    # @return [void]
    # @raise [NullValueError] if value is nil
    # @raise [TransientStateError] if value cannot be represented
    def push_back(value)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Prepends an element at the front.
    #
    # @param value [Object] the element
    # @return [void]
    # @raise [NullValueError] if value is nil
    # @raise [TransientStateError] if value cannot be represented
    def push_front(value)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Removes and returns the front element.
    #
    # @return [Object, nil] the removed element, or nil when empty
    def pop_front
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Removes and returns the back element.
    #
    # @return [Object, nil] the removed element, or nil when empty
    def pop_back
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Removes every element.
    #
    # @return [void]
    def clear
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Opens a native scan over the live elements.
    #
    # @param direction [Symbol] +:forward+ or +:backward+
    # @param query [Hash] optional position keywords (see {Prosody::State::Scanning})
    # @return [Object] the native cursor
    # @raise [TransientStateError] if direction is not +:forward+ or +:backward+
    # @raise [ArgumentError, TypeError] if a query keyword is invalid
    def scan(direction, query = {})
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Durably commits the buffered operations mid-handler.
    #
    # @return [Symbol] +:applied+ or +:no_op+
    def commit
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Discards the buffered uncommitted operations.
    #
    # @return [Symbol] +:applied+ or +:no_op+
    def rollback
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  class NativeJsonDequeState
    include NativeDequeOperations
  end

  class NativeMessageDequeState
    include NativeDequeOperations
  end

  # Native cursor over a keyed-state collection, driven one chunk at a time
  # by the {Prosody::MapState} / {Prosody::DequeState} traversal methods. Each
  # pull crosses the bridge and yields the fiber; +close+ is idempotent.
  #
  # @see ext/prosody/src/handler/state/scan.rs for implementation
  module NativeScanOperations
    # @private
    def initialize
      raise NotImplementedError, "This class is implemented natively in Rust"
    end

    # Pulls the next item from the cursor.
    #
    # @return [Object, nil] the next item, or nil when the cursor is exhausted
    def next
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Closes the native cursor. Idempotent.
    #
    # @return [void]
    def close
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  class NativeJsonDequeScan
    include NativeScanOperations
  end

  class NativeJsonMapScan
    include NativeScanOperations
  end

  class NativeMessageDequeScan
    include NativeScanOperations
  end

  class NativeMessageMapScan
    include NativeScanOperations
  end

  class NativeMapKeyScan
    include NativeScanOperations
  end
end
