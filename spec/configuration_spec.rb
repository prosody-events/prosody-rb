# frozen_string_literal: true

require "spec_helper"

RSpec.describe Prosody::Configuration do
  # Test instance of the configuration class
  subject(:config) { described_class.new }

  describe "default values" do
    # Tests that all configuration parameters have nil default values
    # when not explicitly set
    it "has nil for all unset fields" do
      expect(config.bootstrap_servers).to be_nil
      expect(config.mock).to be_nil
      expect(config.send_timeout).to be_nil
      expect(config.group_id).to be_nil
      expect(config.idempotence_cache_size).to be_nil
      expect(config.subscribed_topics).to be_nil
      expect(config.allowed_events).to be_nil
      expect(config.source_system).to be_nil
      expect(config.max_concurrency).to be_nil
      expect(config.max_uncommitted).to be_nil
      expect(config.stall_threshold).to be_nil
      expect(config.shutdown_timeout).to be_nil
      expect(config.poll_interval).to be_nil
      expect(config.commit_interval).to be_nil
      expect(config.mode).to be_nil
      expect(config.retry_base).to be_nil
      expect(config.max_retries).to be_nil
      expect(config.max_retry_delay).to be_nil
      expect(config.failure_topic).to be_nil
      expect(config.probe_port).to be_nil
      expect(config.cassandra_nodes).to be_nil
      expect(config.cassandra_keyspace).to be_nil
      expect(config.cassandra_datacenter).to be_nil
      expect(config.cassandra_rack).to be_nil
      expect(config.cassandra_user).to be_nil
      expect(config.cassandra_password).to be_nil
      expect(config.cassandra_retention).to be_nil
      # New QoS configuration
      expect(config.slab_size).to be_nil
      expect(config.timeout).to be_nil
      expect(config.scheduler_failure_weight).to be_nil
      expect(config.scheduler_max_wait).to be_nil
      expect(config.scheduler_wait_weight).to be_nil
      expect(config.scheduler_cache_size).to be_nil
      expect(config.monopolization_enabled).to be_nil
      expect(config.monopolization_threshold).to be_nil
      expect(config.monopolization_window).to be_nil
      expect(config.monopolization_cache_size).to be_nil
      expect(config.defer_enabled).to be_nil
      expect(config.defer_base).to be_nil
      expect(config.defer_max_delay).to be_nil
      expect(config.defer_failure_threshold).to be_nil
      expect(config.defer_failure_window).to be_nil
      expect(config.defer_cache_size).to be_nil
      expect(config.defer_store_cache_size).to be_nil
      expect(config.defer_seek_timeout).to be_nil
      expect(config.defer_discard_threshold).to be_nil
      expect(config.telemetry_topic).to be_nil
      expect(config.telemetry_enabled).to be_nil
      expect(config.idempotence_version).to be_nil
      expect(config.idempotence_ttl).to be_nil
      expect(config.message_spans).to be_nil
      expect(config.timer_spans).to be_nil
    end
  end

  describe "field accessors" do
    context "Vec[String] fields" do
      # Tests string array parameters that accept both single strings
      # and arrays of strings, converting them appropriately
      %i[bootstrap_servers subscribed_topics allowed_events cassandra_nodes].each do |field|
        it "sets and gets #{field}" do
          # Test single string gets converted to array
          config.public_send(:"#{field}=", "one")
          expect(config.public_send(field)).to eq(["one"])

          # Test array remains array
          config.public_send(:"#{field}=", %w[one two])
          expect(config.public_send(field)).to eq(%w[one two])

          # Test nil is allowed
          config.public_send(:"#{field}=", nil)
          expect(config.public_send(field)).to be_nil
        end
      end
    end

    context "boolean field" do
      # Tests the mock parameter which accepts boolean values
      it "sets and gets mock" do
        config.mock = true
        expect(config.mock).to eq(true)

        config.mock = false
        expect(config.mock).to eq(false)

        config.mock = nil
        expect(config.mock).to be_nil
      end
    end

    context "duration fields" do
      # Tests configuration parameters that represent time durations,
      # converting them to floating-point seconds
      {
        send_timeout: 5.0,
        stall_threshold: 2.0,
        shutdown_timeout: 3.0,
        poll_interval: 4.0,
        commit_interval: 5.0,
        retry_base: 0.1,
        max_retry_delay: 1.0,
        cassandra_retention: 2_592_000.0,
        slab_size: 3600.0,
        timeout: 30.0,
        scheduler_max_wait: 120.0,
        monopolization_window: 300.0,
        defer_base: 1.0,
        defer_max_delay: 86400.0,
        defer_failure_window: 300.0,
        defer_seek_timeout: 30.0,
        idempotence_ttl: 604_800.0
      }.each do |field, value|
        it "sets and gets #{field}" do
          config.public_send(:"#{field}=", value)
          expect(config.public_send(field)).to eq(value)

          config.public_send(:"#{field}=", nil)
          expect(config.public_send(field)).to be_nil
        end
      end
    end

    context "numeric fields" do
      # Tests integer configuration parameters
      {
        idempotence_cache_size: 10,
        max_concurrency: 3,
        max_uncommitted: 2,
        max_retries: 3,
        scheduler_cache_size: 8192,
        monopolization_cache_size: 8192,
        defer_cache_size: 1024,
        defer_store_cache_size: 8192,
        defer_discard_threshold: 100
      }.each do |field, value|
        it "sets and gets #{field}" do
          config.public_send(:"#{field}=", value)
          expect(config.public_send(field)).to eq(value)

          config.public_send(:"#{field}=", nil)
          expect(config.public_send(field)).to be_nil
        end
      end
    end

    context "float fields" do
      # Tests float configuration parameters (non-duration)
      {
        scheduler_failure_weight: 0.3,
        scheduler_wait_weight: 200.0,
        monopolization_threshold: 0.9,
        defer_failure_threshold: 0.9
      }.each do |field, value|
        it "sets and gets #{field}" do
          config.public_send(:"#{field}=", value)
          expect(config.public_send(field)).to eq(value)

          config.public_send(:"#{field}=", nil)
          expect(config.public_send(field)).to be_nil
        end
      end
    end

    context "boolean fields for middleware" do
      # Tests boolean parameters for middleware configuration
      %i[monopolization_enabled defer_enabled telemetry_enabled].each do |field|
        it "sets and gets #{field}" do
          config.public_send(:"#{field}=", true)
          expect(config.public_send(field)).to eq(true)

          config.public_send(:"#{field}=", false)
          expect(config.public_send(field)).to eq(false)

          config.public_send(:"#{field}=", nil)
          expect(config.public_send(field)).to be_nil
        end
      end
    end

    context "string fields" do
      # Tests string configuration parameters
      {
        group_id: "group1",
        source_system: "system1",
        failure_topic: "fail_topic",
        cassandra_keyspace: "test_keyspace",
        cassandra_datacenter: "dc1",
        cassandra_rack: "rack1",
        cassandra_user: "cassandra_user",
        cassandra_password: "cassandra_password",
        telemetry_topic: "prosody.telemetry-events",
        idempotence_version: "2",
        message_spans: "child",
        timer_spans: "follows_from"
      }.each do |field, value|
        it "sets and gets #{field}" do
          config.public_send(:"#{field}=", value)
          expect(config.public_send(field)).to eq(value)

          config.public_send(:"#{field}=", nil)
          expect(config.public_send(field)).to be_nil
        end
      end
    end

    context "mode field" do
      # Tests the mode parameter which accepts specific symbols or strings,
      # normalizing them to standardized symbols
      it "accepts valid mode symbols" do
        config.mode = :pipeline
        expect(config.mode).to eq(:pipeline)

        config.mode = :low_latency
        expect(config.mode).to eq(:low_latency)

        config.mode = :best_effort
        expect(config.mode).to eq(:best_effort)
      end

      it "accepts string versions of modes" do
        config.mode = "pipeline"
        expect(config.mode).to eq(:pipeline)

        config.mode = "low_latency"
        expect(config.mode).to eq(:low_latency)

        config.mode = "best_effort"
        expect(config.mode).to eq(:best_effort)
      end

      it "accepts alternate mode forms" do
        # Tests case-insensitive mode normalization
        config.mode = "PIPELINE"
        expect(config.mode).to eq(:pipeline)

        config.mode = "Low_Latency"
        expect(config.mode).to eq(:low_latency)

        config.mode = "BEST_EFFORT"
        expect(config.mode).to eq(:best_effort)
      end

      it "raises an error for an invalid mode" do
        expect { config.mode = :invalid }.to raise_error(ArgumentError)
      end

      it "can be reset to nil" do
        config.mode = :pipeline
        expect(config.mode).not_to be_nil
        config.mode = nil
        expect(config.mode).to be_nil
      end
    end

    context "probe_port field" do
      # Tests the probe_port parameter which has special handling for
      # enabling/disabling health probe functionality
      it "returns nil when probe_port is unset" do
        config.probe_port = nil
        expect(config.probe_port).to be_nil
      end

      it "sets and gets a valid port number" do
        config.probe_port = 8080
        expect(config.probe_port).to eq(8080)
      end

      it "disables probe_port using false or :disabled" do
        config.probe_port = false
        expect(config.probe_port).to eq(false)

        config.probe_port = :disabled
        expect(config.probe_port).to eq(false)
      end

      it "accepts string version of :disabled" do
        config.probe_port = "disabled"
        expect(config.probe_port).to eq(false)
      end

      it "raises an error for invalid probe_port values" do
        expect { config.probe_port = :invalid }.to raise_error(TypeError)
        expect { config.probe_port = "not_a_port" }.to raise_error(ArgumentError)
        expect { config.probe_port = -1 }.to raise_error(ArgumentError)
        expect { config.probe_port = 70000 }.to raise_error(ArgumentError)
      end
    end
  end

  describe "initialization with keyword arguments" do
    # Tests initializing a configuration with multiple parameters at once
    it "sets all fields from the keyword hash" do
      config = described_class.new(
        bootstrap_servers: "server1",
        mock: true,
        send_timeout: 5.0,
        group_id: "group1",
        idempotence_cache_size: 100,
        idempotence_version: "2",
        idempotence_ttl: 3600.0,
        subscribed_topics: %w[topic1 topic2],
        allowed_events: ["event1"],
        source_system: "system1",
        max_concurrency: 10,
        max_uncommitted: 5,
        stall_threshold: 2.0,
        shutdown_timeout: 3.0,
        poll_interval: 4.0,
        commit_interval: 5.0,
        mode: :pipeline,
        retry_base: 0.1,
        max_retries: 3,
        max_retry_delay: 1.0,
        failure_topic: "fail_topic",
        probe_port: 1234,
        cassandra_nodes: ["cassandra1", "cassandra2"],
        cassandra_keyspace: "test_keyspace",
        cassandra_datacenter: "dc1",
        cassandra_rack: "rack1",
        cassandra_user: "test_user",
        cassandra_password: "test_password",
        cassandra_retention: 2_592_000.0,
        message_spans: "child",
        timer_spans: "follows_from"
      )

      # Verify all parameters were correctly set and transformed
      expect(config.bootstrap_servers).to eq(["server1"])
      expect(config.mock).to eq(true)
      expect(config.send_timeout).to eq(5.0)
      expect(config.group_id).to eq("group1")
      expect(config.idempotence_cache_size).to eq(100)
      expect(config.idempotence_version).to eq("2")
      expect(config.idempotence_ttl).to eq(3600.0)
      expect(config.subscribed_topics).to eq(%w[topic1 topic2])
      expect(config.allowed_events).to eq(["event1"])
      expect(config.source_system).to eq("system1")
      expect(config.max_concurrency).to eq(10)
      expect(config.max_uncommitted).to eq(5)
      expect(config.stall_threshold).to eq(2.0)
      expect(config.shutdown_timeout).to eq(3.0)
      expect(config.poll_interval).to eq(4.0)
      expect(config.commit_interval).to eq(5.0)
      expect(config.mode).to eq(:pipeline)
      expect(config.retry_base).to eq(0.1)
      expect(config.max_retries).to eq(3)
      expect(config.max_retry_delay).to eq(1.0)
      expect(config.failure_topic).to eq("fail_topic")
      expect(config.probe_port).to eq(1234)
      expect(config.cassandra_nodes).to eq(["cassandra1", "cassandra2"])
      expect(config.cassandra_keyspace).to eq("test_keyspace")
      expect(config.cassandra_datacenter).to eq("dc1")
      expect(config.cassandra_rack).to eq("rack1")
      expect(config.cassandra_user).to eq("test_user")
      expect(config.cassandra_password).to eq("test_password")
      expect(config.cassandra_retention).to eq(2_592_000.0)
      expect(config.message_spans).to eq("child")
      expect(config.timer_spans).to eq("follows_from")
    end

    # Tests that the configuration can be modified via a block
    it "yields the configuration to a block if provided" do
      yielded_config = nil
      config = described_class.new(bootstrap_servers: "server1") do |c|
        yielded_config = c
        c.mock = true
      end

      expect(yielded_config).to eq(config)
      expect(config.bootstrap_servers).to eq(["server1"])
      expect(config.mock).to eq(true)
    end
  end

  describe "#to_hash" do
    # Tests the to_hash method which converts configuration to a native-compatible hash
    it "returns a hash with only non-nil values" do
      config.bootstrap_servers = "server1"
      config.mode = :pipeline
      config.send_timeout = 5.0

      native_hash = config.to_hash

      # Verify only non-nil values are included
      expect(native_hash).to include(
        bootstrap_servers: ["server1"],
        mode: :pipeline,
        send_timeout: 5.0
      )
      expect(native_hash).not_to have_key(:mock)
      expect(native_hash).not_to have_key(:group_id)
    end

    it "returns an empty hash when all values are nil" do
      expect(config.to_hash).to eq({})
    end
  end
end
