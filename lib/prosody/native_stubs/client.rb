# frozen_string_literal: true

# Native stubs for {Prosody::Client}: sending, subscribing, and opening
# published-state readers.

module Prosody
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
    def assigned_partition_count
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Checks if the consumer is stalled.
    #
    # A stalled consumer is one that has stopped processing messages due to
    # errors or reaching processing limits. This can be used to detect unhealthy
    # consumers that need attention.
    #
    # @return [Boolean] true if the consumer is stalled, false otherwise
    def stalled?
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

    private

    # @private
    def published_value(subsystem, name, read_cache, read_cache_disabled)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # @private
    def published_map(subsystem, name, read_cache, read_cache_disabled)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # @private
    def published_set(subsystem, name, read_cache, read_cache_disabled)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # @private
    def published_deque(subsystem, name, read_cache, read_cache_disabled)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end
end
