# frozen_string_literal: true

module Prosody
  # Configuration class for the Prosody messaging client.
  #
  # This class provides a flexible configuration system for the Prosody client,
  # with strong typing, validation, and convenient default values. It handles
  # various parameter types including strings, arrays, durations, integers,
  # and special case configurations like operation mode and health probe settings.
  #
  # @example Basic usage
  #   config = Prosody::Configuration.new(
  #     bootstrap_servers: "kafka:9092",
  #     group_id: "my-consumer-group"
  #   )
  #
  # @example With block syntax
  #   config = Prosody::Configuration.new do |c|
  #     c.bootstrap_servers = ["kafka1:9092", "kafka2:9092"]
  #     c.group_id = "my-consumer-group"
  #     c.max_concurrency = 10
  #   end
  class Configuration
    # Defines a configuration parameter with type conversion and optional default value.
    #
    # This DSL helper creates getter and setter methods for a configuration parameter,
    # with the setter applying type conversion. The getter returns the default value
    # when the parameter hasn't been set.
    #
    # @param name [Symbol, String] The parameter name
    # @param converter [Proc] A lambda that converts and validates the input value
    # @param default [Object, nil] The default value returned when parameter is unset
    # @return [void]
    def self.config_param(name, converter: ->(v) { v }, default: nil)
      name_sym = name.to_sym
      define_method(name) { @config.key?(name_sym) ? @config[name_sym] : default }
      define_method(:"#{name}=") { |value| @config[name_sym] = value.nil? ? nil : converter.call(value) }
    end

    # Converts a duration value to float seconds.
    #
    # Handles ActiveSupport::Duration objects (if available) and numeric values,
    # converting them to floating-point seconds.
    #
    # @param v [Numeric, ActiveSupport::Duration] The duration value to convert
    # @return [Float] The duration in seconds as a float
    # @raise [ArgumentError] If the input is not a valid duration type
    def self.duration_converter(v)
      if defined?(ActiveSupport::Duration) && v.is_a?(ActiveSupport::Duration)
        v.to_f
      elsif v.is_a?(Numeric)
        v.to_f
      else
        raise ArgumentError, "Invalid type for duration: #{v.inspect}"
      end
    end

    # Initializes a new Configuration instance.
    #
    # @param kwargs [Hash] Initial configuration parameters as keyword arguments
    # @yield [self] Yields self for block-based configuration
    # @return [Configuration] The new configuration instance
    def initialize(kwargs = {})
      @config = {}
      kwargs.each { |k, v| public_send(:"#{k}=", v) }
      yield self if block_given?
    end

    # List of Kafka bootstrap server addresses.
    # Accepts a string (single server) or array of strings (multiple servers).
    config_param :bootstrap_servers,
      converter: lambda { |v|
        if v.is_a?(String)
          [v.to_s]
        elsif v.respond_to?(:to_a)
          v.to_a.map(&:to_s)
        else
          raise ArgumentError, "Invalid type for bootstrap_servers"
        end
      }

    # List of Kafka topics to subscribe to.
    # Accepts a string (single topic) or array of strings (multiple topics).
    config_param :subscribed_topics,
      converter: lambda { |v|
        if v.is_a?(String)
          [v.to_s]
        elsif v.respond_to?(:to_a)
          v.to_a.map(&:to_s)
        else
          raise ArgumentError, "Invalid type for subscribed_topics"
        end
      }

    # List of event types the consumer is allowed to process.
    # Accepts a string (single event type) or array of strings (multiple event types).
    config_param :allowed_events,
      converter: lambda { |v|
        if v.is_a?(String)
          [v.to_s]
        elsif v.respond_to?(:to_a)
          v.to_a.map(&:to_s)
        else
          raise ArgumentError, "Invalid type for allowed_events"
        end
      }

    # Whether to use mock mode (for testing).
    # When enabled, creates a mock Kafka implementation instead of connecting to real servers.
    config_param :mock, converter: ->(v) { v.nil? ? nil : !!v }

    # Maximum time to wait for a send operation to complete (in seconds).
    config_param :send_timeout, converter: ->(v) { duration_converter(v) }

    # Threshold in seconds after which a stalled consumer is detected.
    config_param :stall_threshold, converter: ->(v) { duration_converter(v) }

    # Maximum time to wait for a clean shutdown (in seconds).
    config_param :shutdown_timeout, converter: ->(v) { duration_converter(v) }

    # Interval between Kafka poll operations (in seconds).
    config_param :poll_interval, converter: ->(v) { duration_converter(v) }

    # Interval between offset commit operations (in seconds).
    config_param :commit_interval, converter: ->(v) { duration_converter(v) }

    # Base delay for retry operations (in seconds).
    config_param :retry_base, converter: ->(v) { duration_converter(v) }

    # Maximum delay between retries (in seconds).
    config_param :max_retry_delay, converter: ->(v) { duration_converter(v) }

    # Size of the cache used for message idempotence.
    config_param :idempotence_cache_size, converter: ->(v) { Integer(v) }

    # Maximum number of concurrent message processing tasks.
    config_param :max_concurrency, converter: ->(v) { Integer(v) }

    # Maximum number of messages to process before committing offsets.
    config_param :max_uncommitted, converter: ->(v) { Integer(v) }

    # Maximum number of messages to enqueue for a single key.
    config_param :max_enqueued_per_key, converter: ->(v) { Integer(v) }

    # Maximum number of retry attempts.
    config_param :max_retries, converter: ->(v) { Integer(v) }

    # Kafka consumer group ID.
    config_param :group_id, converter: lambda(&:to_s)

    # Identifier for the system producing messages.
    config_param :source_system, converter: lambda(&:to_s)

    # Topic to send failed messages to.
    config_param :failure_topic, converter: lambda(&:to_s)

    # List of Cassandra contact nodes (hostnames or IPs).
    # Accepts a string (single node) or array of strings (multiple nodes).
    config_param :cassandra_nodes,
      converter: lambda { |v|
        if v.is_a?(String)
          [v.to_s]
        elsif v.respond_to?(:to_a)
          v.to_a.map(&:to_s)
        else
          raise ArgumentError, "Invalid type for cassandra_nodes"
        end
      }

    # Keyspace to use for storing timer data in Cassandra.
    config_param :cassandra_keyspace, converter: lambda(&:to_s), default: "prosody"

    # Preferred datacenter for Cassandra query routing.
    config_param :cassandra_datacenter, converter: lambda(&:to_s)

    # Preferred rack identifier for Cassandra topology-aware routing.
    config_param :cassandra_rack, converter: lambda(&:to_s)

    # Username for authenticating with Cassandra.
    config_param :cassandra_user, converter: lambda(&:to_s)

    # Password for authenticating with Cassandra.
    config_param :cassandra_password, converter: lambda(&:to_s)

    # Retention period for failed/unprocessed timer data in Cassandra.
    # Accepts duration objects or numeric values (in seconds).
    config_param :cassandra_retention, converter: ->(v) { duration_converter(v) }

    # Operation mode of the client.
    #
    # Valid values:
    # - "pipeline" or :pipeline - Prioritizes throughput and ordering
    # - "low_latency" or :low_latency - Prioritizes latency over throughput
    # - "best_effort" or :best_effort - Makes best effort tradeoffs
    config_param :mode,
      converter: lambda { |v|
        return nil if v.nil?

        s = v.to_s.downcase
        case s
        when "pipeline"
          :pipeline
        when "low_latency"
          :low_latency
        when "best_effort"
          :best_effort
        else
          raise ArgumentError, "Invalid mode: #{v}"
        end
      }

    # Configuration for the health probe port.
    #
    # Accepts:
    # - nil (uses default configuration)
    # - false or :disabled (explicitly disables the probe port)
    # - Port number (0-65535) to use for the health probe
    config_param :probe_port,
      converter: lambda { |v|
        if v.nil?
          nil
        elsif v == false
          false
        elsif (v.is_a?(Symbol) || v.is_a?(String)) && v.to_sym == :disabled
          false
        else
          port = Integer(v)
          raise ArgumentError, "Invalid port number" unless port.between?(0, 65_535)

          port
        end
      }

    # Returns a Ruby hash with only non-nil values, suitable for Rust deserialization.
    #
    # @return [Hash] Configuration hash with all nil values removed
    def to_hash
      @config.dup.compact
    end
  end
end
