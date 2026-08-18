# frozen_string_literal: true

module Prosody
  # = Native Interface Stubs
  #
  # This file contains stub definitions for native methods implemented in the
  # Prosody Rust extension. These stubs provide documentation and method
  # signatures for Ruby tooling like editors and documentation generators,
  # but the actual implementations are in the Rust extension.
  #
  # == Implementation Notes
  #
  # The actual implementations of these methods are in the Rust extension at:
  # ext/prosody/src/

  # Wrapper for dynamically-typed results returned from async operations.
  # This is an internal class used by the native code to transfer results
  # between Rust and Ruby.
  #
  # @private
  class DynamicResult
    # @private
    def initialize
      raise NotImplementedError, "This class is implemented natively in Rust"
    end
  end

  # Represents the context of a Kafka message, providing metadata and control
  # capabilities for message handling.
  #
  # Instances of this class are created by the native code and passed to your
  # EventHandler's #on_message method.
  #
  # @see ext/prosody/src/handler/context.rs for implementation
  class Context
    # @private
    def initialize
      raise NotImplementedError, "This class is implemented natively in Rust"
    end

    # Checks if cancellation has been requested.
    #
    # This method can be called within message handlers to detect when the
    # handler should exit. Cancellation includes message-level cancellation
    # (e.g., handler timeout) and partition shutdown. During shutdown,
    # cancellation is delayed until near the end of the shutdown timeout to
    # allow in-flight work to complete.
    #
    # @return [Boolean] true if cancellation has been requested, false otherwise
    #
    # @example Checking for cancellation in a loop
    #   def on_message(context, message)
    #     items = message.payload["items"]
    #     items.each do |item|
    #       return if context.should_cancel?
    #       process_item(item)
    #     end
    #   end
    def should_cancel?
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Blocks until cancellation is signaled.
    #
    # Cancellation includes message-level cancellation (e.g., handler timeout)
    # and partition shutdown. During shutdown, cancellation is delayed until near
    # the end of the shutdown timeout to allow in-flight work to complete.
    # This method is useful for long-running handlers that need to wait for
    # external events while remaining responsive to cancellation.
    #
    # @return [void]
    #
    # @example Waiting for cancellation
    #   def on_message(context, message)
    #     # Do some work, then wait for cancellation
    #     context.on_cancel
    #   end
    def on_cancel
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Schedules a timer to fire at the specified time.
    #
    # Timers allow you to delay execution or implement timeout behavior within
    # your message handlers. When a timer fires, your handler's #on_timer method
    # will be called with the timer object.
    #
    # @param time [Time] When the timer should fire
    # @return [void]
    # @raise [ArgumentError] If the time is invalid or outside the supported range (1970-2106)
    # @raise [RuntimeError] If timer scheduling fails
    #
    # @example Scheduling a delayed action
    #   def on_message(context, message)
    #     # Schedule a timer to fire in 30 seconds
    #     context.schedule(Time.now + 30)
    #   end
    #
    #   def on_timer(context, timer)
    #     puts "Timer fired for key: #{timer.key}"
    #   end
    def schedule(time)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Clears all scheduled timers and schedules a new one at the specified time.
    #
    # This is equivalent to calling clear_scheduled followed by schedule, but
    # performed atomically.
    #
    # @param time [Time] When the new timer should fire
    # @return [void]
    # @raise [ArgumentError] If the time is invalid or outside the supported range
    # @raise [RuntimeError] If timer operations fail
    #
    # @example Replacing all timers with a new one
    #   def on_message(context, message)
    #     # Clear any existing timers and schedule a new one
    #     context.clear_and_schedule(Time.now + 60)
    #   end
    def clear_and_schedule(time)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Unschedules a timer that was scheduled for the specified time.
    #
    # If multiple timers were scheduled for the same time, this will remove one
    # of them. If no timer exists for the specified time, this method does nothing.
    #
    # @param time [Time] The time for which to unschedule the timer
    # @return [void]
    # @raise [ArgumentError] If the time is invalid
    # @raise [RuntimeError] If timer unscheduling fails
    #
    # @example Canceling a specific timer
    #   def on_message(context, message)
    #     timer_time = Time.now + 30
    #     context.schedule(timer_time)
    #
    #     # Later, cancel that specific timer
    #     context.unschedule(timer_time)
    #   end
    def unschedule(time)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Clears all scheduled timers.
    #
    # After calling this method, no timers will be scheduled to fire for this
    # message context.
    #
    # @return [void]
    # @raise [RuntimeError] If clearing timers fails
    #
    # @example Canceling all timers
    #   def on_message(context, message)
    #     context.clear_scheduled
    #   end
    def clear_scheduled
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns all currently scheduled timer times.
    #
    # The returned array contains Time objects representing when each scheduled
    # timer will fire. The array may be empty if no timers are scheduled.
    #
    # @return [Array<Time>] Array of scheduled timer times
    # @raise [RuntimeError] If retrieving scheduled times fails
    #
    # @example Checking scheduled timers
    #   def on_message(context, message)
    #     scheduled_times = context.scheduled
    #     puts "#{scheduled_times.length} timers scheduled"
    #     scheduled_times.each do |time|
    #       puts "Timer will fire at: #{time}"
    #     end
    #   end
    def scheduled
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native single-value JSON state handle for the named collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeJsonValueState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def value_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native ordered-map JSON state handle for the named collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeJsonMapState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def map_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native deque JSON state handle for the named collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeJsonDequeState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def deque_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native single-value message state handle for the named
    # collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeMessageValueState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def message_value_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native ordered-map message state handle for the named
    # collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeMessageMapState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def message_map_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native deque message state handle for the named collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeMessageDequeState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def message_deque_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  # Represents a Kafka message with its metadata and payload.
  #
  # Instances of this class are created by the native code and passed to your
  # EventHandler's #on_message method. In RBS, +Message[Payload]+ carries the
  # statically declared payload shape; bare +Message+ defaults to
  # +Prosody::json_value+. This annotation does not add runtime validation.
  #
  # @see ext/prosody/src/handler/message.rs for implementation
  class Message
    # Returns the Kafka topic this message was published to.
    #
    # @return [String] The topic name
    def topic
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns the Kafka partition number for this message.
    #
    # @return [Integer] The partition number
    def partition
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns the Kafka offset of this message within its partition.
    #
    # @return [Integer] The message offset
    def offset
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns the message key used for partitioning.
    #
    # @return [String] The message key
    def key
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns the timestamp when the message was created.
    #
    # @return [Time] The message timestamp
    def timestamp
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns the deserialized message payload.
    #
    # The payload is automatically deserialized from JSON to Ruby objects.
    #
    # @return [Payload] The message content
    def payload
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  # An excise record with Kafka metadata and no payload.
  class ExciseMessage
    # @return [String] The topic name
    def topic = raise NotImplementedError, "This method is implemented natively in Rust"

    # @return [Integer] The partition number
    def partition = raise NotImplementedError, "This method is implemented natively in Rust"

    # @return [Integer] The message offset
    def offset = raise NotImplementedError, "This method is implemented natively in Rust"

    # @return [String] The message key
    def key = raise NotImplementedError, "This method is implemented natively in Rust"

    # @return [Time] The record timestamp
    def timestamp = raise NotImplementedError, "This method is implemented natively in Rust"
  end

  # Represents a timer that was scheduled to fire at a specific time.
  #
  # Timer instances are created by the native code and passed to your
  # EventHandler's #on_timer method when a scheduled timer fires.
  #
  # @see ext/prosody/src/handler/trigger.rs for implementation
  class Timer
    # @private
    def initialize
      raise NotImplementedError, "This class is implemented natively in Rust"
    end

    # Returns the entity key identifying what this timer belongs to.
    #
    # The key is typically the same as the message key that was being processed
    # when the timer was scheduled.
    #
    # @return [String] The entity key
    def key
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns the time when this timer was scheduled to fire.
    #
    # Note: Due to CompactDateTime's second-level precision, the returned time
    # will have zero nanoseconds even if the original scheduled time had
    # sub-second precision.
    #
    # @return [Time] The scheduled execution time
    def time
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  # Main client for interacting with the Prosody messaging system.
  # Provides methods for sending messages and subscribing to Kafka topics.
  #
  # @see ext/prosody/src/client/mod.rs for implementation
  class Client
    # Creates a new Prosody client with the given configuration.
    #
    # @param config [Hash, Configuration] Client configuration
    # @return [Client] A new client instance
    # @raise [ArgumentError] If the configuration is invalid
    # @raise [RuntimeError] If client initialization fails
    #
    # @example Creating a client with a Configuration object
    #   config = Prosody::Configuration.new do |c|
    #     c.bootstrap_servers = "localhost:9092"
    #     c.group_id = "my-consumer-group"
    #   end
    #   client = Prosody::Client.new(config)
    #
    # @example Creating a client with a hash
    #   client = Prosody::Client.new(
    #     bootstrap_servers: "localhost:9092",
    #     group_id: "my-consumer-group"
    #   )
    def self.new(config)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns the current state of the consumer.
    #
    # The consumer can be in one of four states:
    # - `:shut_down` - The client is shut down
    # - `:unconfigured` - The consumer has not been configured yet
    # - `:configured` - The consumer is configured but not running
    # - `:running` - The consumer is actively consuming messages
    #
    # @return [Symbol] The current consumer state
    def consumer_state
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def native_request(_request)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns the number of Kafka partitions currently assigned to this consumer.
    #
    # This method can be used to monitor the consumer's workload and ensure
    # proper load balancing across multiple consumer instances.
    #
    # @return [Integer] The number of assigned partitions
    def assigned_partitions
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Checks if the consumer is stalled.
    #
    # A stalled consumer is one that has stopped processing messages due to
    # errors or reaching processing limits. This can be used to detect unhealthy
    # consumers that need attention.
    #
    # @return [Boolean] true if the consumer is stalled, false otherwise
    def is_stalled?
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Sends a message to the specified Kafka topic.
    #
    # @param topic [String] The destination topic name
    # @param key [String] The message key for partitioning
    # @param payload [Prosody::json_value] The JSON-compatible message payload
    # @return [void]
    # @raise [RuntimeError] If the message cannot be sent
    #
    # @example Sending a simple message
    #   client.send_message("my-topic", "user-123", {
    #     "event" => "login", "timestamp" => Time.now.to_i
    #   })
    def send_message(topic, key, payload)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Sends an excise record to the specified Kafka topic.
    #
    # @param topic [String] The destination topic name
    # @param key [String] The message key for partitioning
    # @return [void]
    # @raise [RuntimeError] If the excise record cannot be sent
    def excise(topic, key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Subscribes to Kafka topics using the provided handler.
    # The handler must implement `on_message`, `on_excise`, and `on_timer`.
    #
    # @param handler [EventHandler] A handler object that processes messages
    # @return [void]
    # @raise [ArgumentError] If a required handler method is missing
    # @raise [RuntimeError] If subscription fails
    #
    # @example Subscribing with a handler
    #   class MyHandler < Prosody::EventHandler
    #     def on_message(context, message)
    #       puts "Received message: #{message.payload}"
    #     end
    #
    #     def on_excise(_context, message)
    #       puts "Excised key: #{message.key}"
    #     end
    #
    #     def on_timer(_context, _timer)
    #     end
    #   end
    #
    #   client.subscribe(MyHandler.new)
    def subscribe(handler)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Unsubscribes from all topics, stopping message processing.
    #
    # This method gracefully shuts down the consumer, completing any in-flight
    # messages before stopping.
    #
    # @return [void]
    # @raise [RuntimeError] If unsubscription fails
    #
    def unsubscribe
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Shuts down the consumer and all client services.
    #
    # @return [void]
    # @raise [RuntimeError] If shutdown fails
    #
    # @example Shutting down a client
    #   client.shutdown
    def shutdown
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns the configured source system identifier.
    #
    # The source system is used to identify the originating service or
    # component in produced messages, enabling loop detection.
    #
    # @return [String] The source system identifier
    #
    # @example Getting the source system
    #   puts client.source_system  # => "my-service"
    def source_system
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # @private
    def published_value(subsystem, name, read_cache, read_cache_disabled)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # @private
    def published_map(subsystem, name, read_cache, read_cache_disabled)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # @private
    def published_deque(subsystem, name, read_cache, read_cache_disabled)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

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
    # @return [nil]
    def commit
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Discards the buffered uncommitted operations.
    #
    # @return [nil]
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

    # Reads several keys in a single isolated batch.
    #
    # @param keys [Array<String>] the keys to read, in order
    # @return [Array<Object, nil>] one result per input key
    def get_many(keys)
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
    # @return [Object] the native cursor
    # @raise [TransientStateError] if direction is not +:forward+ or +:backward+
    def scan(direction)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Opens a native ordered scan over the live keys only, yielding bare keys.
    # Skips value decode and the resolver (a message-backed map enumerates keys
    # with zero Kafka fetches), though not no-I/O.
    #
    # @param direction [Symbol] +:forward+ or +:backward+
    # @return [NativeMapKeyScan] the native key cursor
    # @raise [TransientStateError] if direction is not +:forward+ or +:backward+
    def keys(direction)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Durably commits the buffered operations mid-handler.
    #
    # @return [nil]
    def commit
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Discards the buffered uncommitted operations.
    #
    # @return [nil]
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
    # @return [Object] the native cursor
    # @raise [TransientStateError] if direction is not +:forward+ or +:backward+
    def scan(direction)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Durably commits the buffered operations mid-handler.
    #
    # @return [nil]
    def commit
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Discards the buffered uncommitted operations.
    #
    # @return [nil]
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

  # @private
  class NativePublishedValue
    def get(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  # @private
  class NativePublishedMap
    def get(key, map_key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def get_many(key, map_keys)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def contains_key(key, map_key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def scan(key, direction)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def keys(key, direction)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  # @private
  class NativePublishedDeque
    def get(key, index)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def length(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def is_empty(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def peek_front(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def peek_back(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def scan(key, direction)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  # Internal processor for executing tasks asynchronously.
  # This class is used internally by the native code.
  #
  # @private
  class AsyncTaskProcessor
    # @private
    def initialize(logger = Prosody.logger)
      # Actual implementation is in lib/prosody/processor.rb
    end

    # @private
    def start
      # Actual implementation is in lib/prosody/processor.rb
    end

    # @private
    def stop
      # Actual implementation is in lib/prosody/processor.rb
    end

    # @private
    def submit(task_id, carrier, event_context, callback, &block)
      # Actual implementation is in lib/prosody/processor.rb
    end
  end
end
