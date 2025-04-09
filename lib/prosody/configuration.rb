# frozen_string_literal: true

module Prosody
  class Configuration
    # DSL helper to define configuration parameters.
    # It defines a getter that checks key existence (so false is returned if set)
    # and a setter that applies a converter.
    def self.config_param(name, converter: ->(v) { v }, default: nil)
      name_sym = name.to_sym
      define_method(name) { @config.key?(name_sym) ? @config[name_sym] : default }
      define_method(:"#{name}=") { |value| @config[name_sym] = value.nil? ? nil : converter.call(value) }
    end

    # Helper to convert a duration to float seconds.
    def self.duration_converter(v)
      if defined?(ActiveSupport::Duration) && v.is_a?(ActiveSupport::Duration)
        v.to_f
      elsif v.is_a?(Numeric)
        v.to_f
      else
        raise ArgumentError, "Invalid type for duration: #{v.inspect}"
      end
    end

    def initialize(kwargs = {})
      @config = {}
      kwargs.each { |k, v| public_send(:"#{k}=", v) }
      yield self if block_given?
    end

    # Vec[String] fields
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

    # Boolean field
    config_param :mock, converter: ->(v) { v.nil? ? nil : !!v }

    # Duration fields (stored as float seconds)
    config_param :send_timeout, converter: ->(v) { duration_converter(v) }
    config_param :stall_threshold, converter: ->(v) { duration_converter(v) }
    config_param :shutdown_timeout, converter: ->(v) { duration_converter(v) }
    config_param :poll_interval, converter: ->(v) { duration_converter(v) }
    config_param :commit_interval, converter: ->(v) { duration_converter(v) }
    config_param :retry_base, converter: ->(v) { duration_converter(v) }
    config_param :max_retry_delay, converter: ->(v) { duration_converter(v) }

    # Numeric fields
    config_param :idempotence_cache_size, converter: ->(v) { Integer(v) }
    config_param :max_concurrency, converter: ->(v) { Integer(v) }
    config_param :max_uncommitted, converter: ->(v) { Integer(v) }
    config_param :max_enqueued_per_key, converter: ->(v) { Integer(v) }
    config_param :max_retries, converter: ->(v) { Integer(v) }

    # String fields
    config_param :group_id, converter: lambda(&:to_s)
    config_param :source_system, converter: lambda(&:to_s)
    config_param :failure_topic, converter: lambda(&:to_s)

    # Mode field – accepts symbols or strings (with alternate forms), stored as symbols.
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

    # Probe port field – accepts nil, false, :disabled, or a port number.
    # When disabled, the getter should return false.
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

    # Returns a Ruby hash with only non-nil values, matching the schema for Rust deserialization.
    def to_hash
      @config.dup.compact
    end
  end
end
