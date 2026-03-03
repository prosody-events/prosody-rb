# frozen_string_literal: true

# Shared test configuration for Prosody integration tests
module TestConfig
  # Kafka configuration
  BOOTSTRAP_SERVERS = ENV.fetch("PROSODY_BOOTSTRAP_SERVERS", "localhost:9094")

  # Cassandra configuration
  CASSANDRA_NODES = ENV.fetch("PROSODY_CASSANDRA_NODES", "localhost:9042")

  # Test constants
  GROUP_NAME = "test-group"
  SOURCE_NAME = "test-source"
  MESSAGE_TIMEOUT = 5 # Time to wait for message delivery in seconds

  # Creates a test configuration for Prosody::Client
  # @param topic [String] The Kafka topic to subscribe to
  # @param additional_options [Hash] Additional configuration options
  # @return [Prosody::Configuration] Configured instance
  def self.create_configuration(topic, additional_options = {})
    Prosody::Configuration.new({
      bootstrap_servers: BOOTSTRAP_SERVERS,
      group_id: GROUP_NAME,
      source_system: SOURCE_NAME,
      subscribed_topics: topic,
      probe_port: false,
      mode: :pipeline,
      cassandra_nodes: CASSANDRA_NODES
    }.merge(additional_options))
  end
end
