# frozen_string_literal: true

# The value collection: the owned {Prosody::ValueState} handle and the
# {Prosody::PublishedValue} reader.

module Prosody
  # A single-value keyed-state handle.
  #
  # Reads return the stored JSON value (or a {Prosody::Message} for message
  # collections), or `nil` when the value is absent. Writes are buffered and
  # made durable by {#commit}. All operations are fiber-yield async: they look
  # blocking but never block the thread.
  class ValueState
    # @param native [Prosody::NativeJsonValueState, Prosody::NativeMessageValueState] the native handle
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
    # @raise [PermanentStateError] if `value` is `nil` (use {#clear} to delete)
    def set(value) = @native.set(value)

    # Buffers a clear of the value.
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

  # A read-only view of a published value, opened by
  # +client.state(subsystem, definition)+. Each read takes the user key and
  # sees only committed state. A failed read raises the same state errors as
  # an owned handle. A read in a forked child process raises +RuntimeError+.
  class PublishedValue
    # @param native [Prosody::NativePublishedValue] the native reader
    def initialize(native) = @native = native

    # Reads the committed value for +key+.
    #
    # @param key [#to_s] the user key
    # @return [Object, nil] the value, or +nil+ when it is absent
    def get(key) = @native.get(key.to_s)
  end
end
