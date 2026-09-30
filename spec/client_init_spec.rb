# frozen_string_literal: true

require "spec_helper"

RSpec.describe Prosody::Client, integration: true do
  after { @client&.shutdown }

  it "initializes a client when given minimal producer configuration as hash" do
    # Create the client with a hash directly
    @client = Prosody::Client.new(
      bootstrap_servers: TestConfig::BOOTSTRAP_SERVERS,
      source_system: "init-test-system"
    )

    expect(@client).to be_a(Prosody::Client)
  end

  it "permits repeated shutdown calls" do
    @client = Prosody::Client.new(
      bootstrap_servers: TestConfig::BOOTSTRAP_SERVERS,
      source_system: "init-test-system"
    )

    @client.shutdown
    @client.shutdown
  end

  it "initializes a client when given a Configuration object" do
    # Build a Configuration object explicitly
    config = Prosody::Configuration.new(
      bootstrap_servers: TestConfig::BOOTSTRAP_SERVERS,
      source_system: "init-test-system"
    )

    # Ensure the configuration converted the string to an array
    expect(config.bootstrap_servers).to eq([TestConfig::BOOTSTRAP_SERVERS])

    # Create the client with the Configuration object
    @client = Prosody::Client.new(config)

    expect(@client).to be_a(Prosody::Client)
  end

  describe "duration options" do
    options = %i[
      send_timeout stall_threshold shutdown_timeout poll_interval commit_interval statistics_interval
      retry_base max_retry_delay idempotence_ttl peer_registration_ttl
      cassandra_retention slab_size scheduler_max_wait monopolization_window
      defer_base defer_max_delay defer_failure_window loader_seek_timeout timeout
    ]

    # Every duration option converts to a Rust Duration. A value that has no
    # Duration form raises ArgumentError and never ends the process.
    options.product([-1, Float::NAN, Float::INFINITY, 1e40]).each do |option, value|
      it "rejects #{option} = #{value}" do
        config = {mock: true, bootstrap_servers: TestConfig::BOOTSTRAP_SERVERS}.merge(option => value)
        expect { Prosody::Client.new(config) }.to raise_error(ArgumentError, /#{option}: must be a finite, non-negative number of seconds/)
      end
    end
  end

  describe "options that reach core" do
    def mock_client(**options)
      @client = Prosody::Client.new(mock: true, bootstrap_servers: TestConfig::BOOTSTRAP_SERVERS,
        source_system: TestConfig::SOURCE_NAME, group_id: TestConfig.unique_group_id, **options)
    end

    # Core validates the interval when the consumer starts. A zero interval
    # fails only when the option reaches the consumer configuration.
    it "passes statistics_interval to the consumer" do
      client = mock_client(statistics_interval: 0, subscribed_topics: "statistics")
      expect { client.subscribe(CompleteHandler.new) }.to raise_error(RuntimeError, /statistics_interval/)
    end
  end
end
