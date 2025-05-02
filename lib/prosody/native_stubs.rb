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

    # NOTE: Additional context methods may be added in the future to provide
    # more control over message processing, such as manual acknowledgement.
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
  end

  # Internal processor for executing tasks asynchronously.
  # This class is used internally by the native code.
  #
  # @private
  class AsyncTaskProcessor
    # @private
    def initialize(logger = Logger.new($stdout))
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
