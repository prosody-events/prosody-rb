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
  end

  # Represents a Kafka message with its metadata and payload.
  #
  # Instances of this class are created by the native code and passed to your
  # EventHandler's #on_message method.
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
    # @return [Object] The message content
    def payload
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
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
    # The consumer can be in one of three states:
    # - `:unconfigured` - The consumer has not been configured yet
    # - `:configured` - The consumer is configured but not running
    # - `:running` - The consumer is actively consuming messages
    #
    # @return [Symbol] The current consumer state
    def consumer_state
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
    # @param payload [Object] The message payload (will be serialized to JSON)
    # @return [void]
    # @raise [RuntimeError] If the message cannot be sent
    #
    # @example Sending a simple message
    #   client.send_message("my-topic", "user-123", { event: "login", timestamp: Time.now })
    def send_message(topic, key, payload)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Subscribes to Kafka topics using the provided handler.
    # The handler must implement an `on_message(context, message)` method.
    #
    # @param handler [EventHandler] A handler object that processes messages
    # @return [void]
    # @raise [RuntimeError] If subscription fails
    #
    # @example Subscribing with a handler
    #   class MyHandler < Prosody::EventHandler
    #     def on_message(context, message)
    #       puts "Received message: #{message.payload}"
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
    # @example Shutting down a consumer
    #   client.unsubscribe
    def unsubscribe
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
    def submit(task_id, carrier, callback, &block)
      # Actual implementation is in lib/prosody/processor.rb
    end
  end
end
