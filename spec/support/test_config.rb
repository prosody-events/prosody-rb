# frozen_string_literal: true

require "securerandom"

# Shared test configuration for Prosody integration tests
module TestConfig
  # Kafka configuration
  BOOTSTRAP_SERVERS = ENV.fetch("PROSODY_BOOTSTRAP_SERVERS", "localhost:9094")

  # Cassandra configuration
  CASSANDRA_NODES = ENV.fetch("PROSODY_CASSANDRA_NODES", "localhost:9042")

  # Test constants
  GROUP_PREFIX = "test-group"
  SOURCE_NAME = "test-source"
  MESSAGE_TIMEOUT = 30 # Time to wait for message delivery in seconds

  # A unique consumer group for one spec example.
  #
  # A group shared across examples, or with another suite's concurrent run,
  # stalls every new member's partition assignment until a stopped member's
  # session expires, which can outlast MESSAGE_TIMEOUT.
  def self.unique_group_id
    "#{GROUP_PREFIX}-#{SecureRandom.hex(6)}"
  end

  # Creates a test configuration for Prosody::Client
  # @param topic [String] The Kafka topic to subscribe to
  # @param additional_options [Hash] Additional configuration options
  # @return [Prosody::Configuration] Configured instance
  def self.create_configuration(topic, additional_options = {})
    Prosody::Configuration.new({
      bootstrap_servers: BOOTSTRAP_SERVERS,
      group_id: unique_group_id,
      source_system: SOURCE_NAME,
      subscribed_topics: topic,
      probe_port: false,
      mode: :pipeline,
      cassandra_nodes: CASSANDRA_NODES
    }.merge(additional_options))
  end
end
