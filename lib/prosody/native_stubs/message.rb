# frozen_string_literal: true

# Native stubs for the event types that reach a handler: {Prosody::Message},
# {Prosody::ExciseMessage}, and {Prosody::Timer}.

module Prosody
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
end
