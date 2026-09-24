# frozen_string_literal: true

module Prosody
  # A presence-only ordered set of String members, vended by
  # +context.state(Prosody.set(...))+. It mirrors Ruby's +Set+.
  #
  # Writes are buffered and made durable when the handler succeeds or by
  # {#commit}. Every operation yields the fiber, never the thread. There is
  # deliberately no +Enumerable+ or +to_a+: they would read the whole remote
  # set. Traverse members with {#each} or {#reverse_each}.
  class SetState
    include State::Scanning

    # @param native [Prosody::NativeSetState] the native handle
    def initialize(native)
      @native = native
    end

    # Adds +member+ (mirrors +Set#add+).
    #
    # @param member [String]
    # @return [self]
    def add(member)
      @native.insert(member)
      self
    end

    alias_method :<<, :add

    # Removes +member+ when present (mirrors +Set#delete+).
    #
    # @param member [String]
    # @return [self]
    def delete(member)
      @native.remove(member)
      self
    end

    # Whether +member+ belongs to the set (mirrors +Set#include?+).
    #
    # @param member [String]
    # @return [Boolean]
    def include?(member) = @native.contains(member)

    alias_method :member?, :include?

    # Tests several members in a single batch.
    #
    # @param members [Array<String>] the members to test, in order
    # @return [Array<Boolean>] one result per input member
    def contains_many(members) = @native.contains_many(members)

    # Whether the set has no live members (mirrors +Set#empty?+).
    #
    # @return [Boolean]
    def empty? = @native.is_empty

    # Removes every member (mirrors +Set#clear+).
    #
    # @return [self]
    def clear
      @native.clear
      self
    end

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

    # Traverses the live members in ascending order.
    #
    # Without a block, returns an {Enumerator} over the native scan. Accepts
    # the query keywords documented on {State::Scanning}, including
    # +prefix:+.
    #
    # @param query [Hash] optional query keywords
    # @yieldparam member [String]
    # @return [Enumerator, void]
    # @raise [ArgumentError, TypeError] if a query keyword is invalid
    def each(**query, &block) = traverse(:forward, query, &block)

    # Traverses the live members in descending order.
    #
    # @param query [Hash] optional query keywords, as on {#each}
    # @yieldparam member [String]
    # @return [Enumerator, void]
    def reverse_each(**query, &block) = traverse(:backward, query, &block)

    private

    def traverse(direction, query)
      return enum_for(:traverse, direction, query) unless block_given?

      scan_each(direction, query, :keys) { |member| yield member }
    end
  end

  # A read-only view of a published set, opened by
  # +client.state(subsystem, definition)+. Each read takes the user key.
  class PublishedSet
    include State::Scanning

    # @param native [Prosody::NativePublishedSet] the native reader
    def initialize(native)
      @native = native
    end

    # Whether +member+ belongs to the committed set for +key+.
    #
    # @return [Boolean]
    def include?(key, member) = @native.contains(key.to_s, member.to_s)

    alias_method :member?, :include?

    # Tests several members of the committed set for +key+ in one batch.
    #
    # @return [Array<Boolean>] one result per input member
    def contains_many(key, members) = @native.contains_many(key.to_s, members.map(&:to_s))

    # Whether the committed set for +key+ has no members.
    #
    # @return [Boolean]
    def empty?(key) = @native.is_empty(key.to_s)

    # Traverses the committed members for +key+ in ascending order. Accepts
    # the query keywords documented on {State::Scanning}.
    #
    # @return [Enumerator, void]
    def each(key, **query, &block) = traverse(key, :forward, query, &block)

    # Traverses the committed members for +key+ in descending order.
    #
    # @return [Enumerator, void]
    def reverse_each(key, **query, &block) = traverse(key, :backward, query, &block)

    private

    def traverse(key, direction, query)
      return enum_for(:traverse, key, direction, query) unless block_given?

      scan_items(@native.keys(key.to_s, direction, query)) { |member| yield member }
    end
  end
end
