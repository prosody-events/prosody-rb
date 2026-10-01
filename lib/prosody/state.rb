# frozen_string_literal: true

module Prosody
  # Base class for errors raised by keyed-state operations that will not
  # succeed on retry (an unregistered collection name, an identity mismatch, a
  # duplicate registration, an invalid TTL, a JSON `null` write). A `null` is
  # not a storable value. Use `clear` (value, deque) or `delete` (map) to
  # remove one.
  #
  # It subclasses {PermanentError} so a rethrown state error is classified as
  # permanent by the result bridge's `#permanent?` path with no bridge change.
  #
  # @see PermanentError
  class PermanentStateError < PermanentError; end

  # Base class for errors raised by keyed-state operations that may succeed on
  # retry. Every caller/input mistake that the client detects (a wrong item
  # shape, an invalid index, an invalid direction token, an unrepresentable
  # value) is transient so the message retries and stays visible rather than
  # being discarded.
  #
  # It subclasses {TransientError} so a rethrown state error is classified as
  # transient by the result bridge's `#permanent?` path with no bridge change.
  #
  # @see TransientError
  class TransientStateError < TransientError; end

  # An immutable keyed-state collection definition.
  #
  # The {Prosody.value}, {Prosody.map}, {Prosody.set}, and {Prosody.deque}
  # constructors and their `message_*` siblings return frozen definitions. A
  # definition serializes into `Configuration#state_collections` through
  # {#to_state_config}, so the collection is registered before subscribe.
  # {Prosody::Context#state} uses it to open the matching typed handle, and
  # {Prosody::Client#state} uses it to open a published reader.
  StateDefinition = Data.define(:name, :kind, :payload, :ttl_seconds, :read_uncommitted,
    :published, :read_cache, :keyset_limit, :capacity, :access) do
    # Every option defaults to +nil+, so a constructor passes only the
    # options its kind takes.
    def initialize(name:, kind:, access:, payload: nil, ttl_seconds: nil, read_uncommitted: nil,
      published: nil, read_cache: nil, keyset_limit: nil, capacity: nil)
      super
    end

    # Serializes this definition into the native-registration hash, omitting
    # unset optionals so they fall back to the core defaults. A set has no
    # payload, so its hash has no payload key.
    #
    # @return [Hash] the registration hash for the native layer
    def to_state_config
      to_h.slice(:name, :kind, :payload, :ttl_seconds, :read_uncommitted, :published, :keyset_limit,
        :capacity).compact
    end
  end

  # Defines a single-value JSON collection.
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @param published [Boolean, nil] allow read-only access from other consumer groups
  # @param read_cache [Numeric, false, nil] cache TTL in seconds for the readers
  #   that +client.state+ opens, +false+ to bypass the cache, or +nil+ to
  #   inherit the client default
  # @return [StateDefinition] a frozen definition
  def self.value(name, ttl: nil, read_uncommitted: nil, published: nil, read_cache: nil)
    StateDefinition.new(name: name.to_s, kind: "value", payload: "json", ttl_seconds: ttl,
      read_uncommitted: read_uncommitted, published: published, read_cache: read_cache, access: VALUE_ACCESS)
  end

  # Defines a `String`-keyed ordered map JSON collection.
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param keyset_limit [Integer, nil] optional map-only keyset bound (`0..=4096`)
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @param published [Boolean, nil] allow read-only access from other consumer groups
  # @param read_cache [Numeric, false, nil] cache TTL in seconds for the readers
  #   that +client.state+ opens, +false+ to bypass the cache, or +nil+ to
  #   inherit the client default
  # @return [StateDefinition] a frozen definition
  def self.map(name, ttl: nil, keyset_limit: nil, read_uncommitted: nil, published: nil, read_cache: nil)
    StateDefinition.new(name: name.to_s, kind: "map", payload: "json", ttl_seconds: ttl,
      keyset_limit: keyset_limit, read_uncommitted: read_uncommitted, published: published,
      read_cache: read_cache, access: MAP_ACCESS)
  end

  # Defines an ordered set of String members. A set stores membership only,
  # so it has no payload.
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param keyset_limit [Integer, nil] optional keyset bound (`0..=4096`)
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @param published [Boolean, nil] allow read-only access from other consumer groups
  # @param read_cache [Numeric, false, nil] cache TTL in seconds for the readers
  #   that +client.state+ opens, +false+ to bypass the cache, or +nil+ to
  #   inherit the client default
  # @return [StateDefinition] a frozen definition
  def self.set(name, ttl: nil, keyset_limit: nil, read_uncommitted: nil, published: nil, read_cache: nil)
    StateDefinition.new(name: name.to_s, kind: "set", ttl_seconds: ttl, keyset_limit: keyset_limit,
      read_uncommitted: read_uncommitted, published: published, read_cache: read_cache, access: SET_ACCESS)
  end

  # Defines a deque JSON collection.
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param capacity [Integer, nil] optional window bound (at least 1); the
  #   deque keeps at most this many slots, enforced lazily on push. Runtime-only
  #   and mutable across deploys, never persisted (see {DequeState#push}).
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @param published [Boolean, nil] allow read-only access from other consumer groups
  # @param read_cache [Numeric, false, nil] cache TTL in seconds for the readers
  #   that +client.state+ opens, +false+ to bypass the cache, or +nil+ to
  #   inherit the client default
  # @return [StateDefinition] a frozen definition
  def self.deque(name, ttl: nil, capacity: nil, read_uncommitted: nil, published: nil, read_cache: nil)
    StateDefinition.new(name: name.to_s, kind: "deque", payload: "json", ttl_seconds: ttl,
      capacity: capacity, read_uncommitted: read_uncommitted, published: published,
      read_cache: read_cache, access: DEQUE_ACCESS)
  end

  # Defines a single-value Kafka-message collection (items are full messages).
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @return [StateDefinition] a frozen definition
  def self.message_value(name, ttl: nil, read_uncommitted: nil)
    StateDefinition.new(name: name.to_s, kind: "value", payload: "message", ttl_seconds: ttl,
      read_uncommitted: read_uncommitted, access: MESSAGE_VALUE_ACCESS)
  end

  # Defines a `String`-keyed ordered map Kafka-message collection.
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param keyset_limit [Integer, nil] optional map-only keyset bound (`0..=4096`)
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @return [StateDefinition] a frozen definition
  def self.message_map(name, ttl: nil, keyset_limit: nil, read_uncommitted: nil)
    StateDefinition.new(name: name.to_s, kind: "map", payload: "message", ttl_seconds: ttl,
      keyset_limit: keyset_limit, read_uncommitted: read_uncommitted, access: MESSAGE_MAP_ACCESS)
  end

  # Defines a deque Kafka-message collection.
  #
  # @param name [#to_s] the collection name (unique within the client)
  # @param ttl [Integer, nil] optional per-write TTL in whole seconds
  # @param capacity [Integer, nil] optional window bound (at least 1); the
  #   deque keeps at most this many slots, enforced lazily on push. Runtime-only
  #   and mutable across deploys, never persisted (see {DequeState#push}).
  # @param read_uncommitted [Boolean, nil] optional opt-out of transactional staging
  # @return [StateDefinition] a frozen definition
  def self.message_deque(name, ttl: nil, capacity: nil, read_uncommitted: nil)
    StateDefinition.new(name: name.to_s, kind: "deque", payload: "message", ttl_seconds: ttl,
      capacity: capacity, read_uncommitted: read_uncommitted, access: MESSAGE_DEQUE_ACCESS)
  end

  # Shared state wrapper behavior.
  module State
    # The base class of the owned handles. It holds the native handle and gives
    # the commit and rollback that every collection shares.
    #
    # @api private
    class Handle
      # @param native [Object] the native handle
      def initialize(native)
        @native = native
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
    end

    module Reading
      # Opens a read-only view of a published JSON or set collection.
      #
      # @raise [TransientStateError, PermanentStateError] if Prosody cannot
      #   open the reader, for example for a zero +read_cache+
      def state(subsystem, definition)
        open, reader = definition.access.reader
        raise ArgumentError, "published state readers support JSON and set collections only" unless open

        cache = definition.read_cache
        cache = Float(cache) unless cache.nil? || cache == true || cache == false
        reader.new(send(open, subsystem.to_s, definition.name, cache))
      end
    end

    # Adds keyed-state vending to the native context. Included into
    # {Prosody::Context}; kept as a module so the routing can be exercised
    # against a stand-in receiver.
    module Vending
      # Vends the typed keyed-state handle for `definition`.
      #
      # Handles are cached per context by definition, so repeated vends within
      # one handler invocation return the same wrapper.
      #
      # @param definition [StateDefinition] a frozen collection definition
      # @return [ValueState, MapState, SetState, DequeState] the typed handle
      # @raise [PermanentStateError] if the collection name is unregistered or
      #   its registered identity mismatches
      def state(definition)
        (@state_handles ||= {})[definition] ||=
          definition.access.wrapper.new(send(definition.access.vend_method, definition.name))
      end
    end

    # Shared cursor-driving for the explicit-traversal handles. Folds the
    # identical native-scan open/close/exhaustion loop; each handle supplies
    # only the per-item yield shape through the block. Kept private (mixed into
    # the handle classes) since it is not part of the public surface.
    #
    # Every traversal method accepts optional query keywords. Map and set
    # traversals take `from:`, `after:`, `to:`, `before:`, `range:`,
    # `prefix:`, and `limit:` over String keys. Deque traversals take the same
    # keywords without `prefix:`, over non-negative positions from the front.
    # `from:`/`after:` start and `to:`/`before:` stop in iteration order, so a
    # reverse traversal starts at the high end. `range:` takes a Ruby `Range`
    # (inclusive, exclusive, beginless, or endless) in ascending order and
    # applies in either direction. A descending `Range` is empty. Every
    # keyword narrows the selection. `limit:` counts yielded
    # items. The native layer translates the keywords and raises
    # `ArgumentError` or `TypeError` for a bad one.
    module Scanning
      private

      # Opens the native cursor `opener` (`:scan` or `:keys`) with `args`,
      # yields each item, and closes the cursor via `ensure` on stop or
      # exception. Without a block, returns an Enumerator. A map scan yields
      # each `[key, value]` pair as one Array, as `Hash#each_pair` does.
      def traverse(opener, *args, &block)
        return enum_for(:traverse, opener, *args) unless block

        scan = @native.public_send(opener, *args)
        while (chunk = scan.next_chunk)
          chunk.each(&block)
        end
      ensure
        scan&.close
      end

      # Traverses a map scan like {#traverse} and yields only each value.
      def traverse_values(*args)
        return enum_for(:traverse_values, *args) unless block_given?

        traverse(:scan, *args) { |entry| yield entry[1] }
      end
    end
  end

  class Client
    include State::Reading

    private :published_value, :published_map, :published_set, :published_deque
  end

  # Reopens the native context class to add keyed-state vending.
  class Context
    include State::Vending

    private :value_state, :map_state, :set_state, :deque_state,
      :message_value_state, :message_map_state, :message_deque_state
  end
end

require_relative "state/value"
require_relative "state/map"
require_relative "state/set"
require_relative "state/deque"

module Prosody
  # How a definition opens its handle and, for a published collection, its reader.
  StateAccess = Data.define(:vend_method, :wrapper, :reader)
  private_constant :StateAccess
  VALUE_ACCESS = StateAccess.new(:value_state, ValueState, [:published_value, PublishedValue])
  MAP_ACCESS = StateAccess.new(:map_state, MapState, [:published_map, PublishedMap])
  SET_ACCESS = StateAccess.new(:set_state, SetState, [:published_set, PublishedSet])
  DEQUE_ACCESS = StateAccess.new(:deque_state, DequeState, [:published_deque, PublishedDeque])
  MESSAGE_VALUE_ACCESS = StateAccess.new(:message_value_state, ValueState, nil)
  MESSAGE_MAP_ACCESS = StateAccess.new(:message_map_state, MapState, nil)
  MESSAGE_DEQUE_ACCESS = StateAccess.new(:message_deque_state, DequeState, nil)
  private_constant :VALUE_ACCESS, :MAP_ACCESS, :SET_ACCESS, :DEQUE_ACCESS,
    :MESSAGE_VALUE_ACCESS, :MESSAGE_MAP_ACCESS, :MESSAGE_DEQUE_ACCESS
end
