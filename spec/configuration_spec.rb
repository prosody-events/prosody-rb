# frozen_string_literal: true

require "spec_helper"

RSpec.describe Prosody::Configuration do
  subject(:config) { described_class.new }

  describe "default values" do
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
      expect(config.max_enqueued_per_key).to be_nil
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
    end
  end

  describe "field accessors" do
    context "Vec[String] fields" do
      %i[bootstrap_servers subscribed_topics allowed_events].each do |field|
        it "sets and gets #{field}" do
          config.public_send(:"#{field}=", "one")
          expect(config.public_send(field)).to eq(["one"])

          config.public_send(:"#{field}=", %w[one two])
          expect(config.public_send(field)).to eq(%w[one two])

          config.public_send(:"#{field}=", nil)
          expect(config.public_send(field)).to be_nil
        end
      end
    end

    context "boolean field" do
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
      {
        send_timeout: 5.0,
        stall_threshold: 2.0,
        shutdown_timeout: 3.0,
        poll_interval: 4.0,
        commit_interval: 5.0,
        retry_base: 0.1,
        max_retry_delay: 1.0
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
      {
        idempotence_cache_size: 10,
        max_concurrency: 3,
        max_uncommitted: 2,
        max_enqueued_per_key: 4,
        max_retries: 3
      }.each do |field, value|
        it "sets and gets #{field}" do
          config.public_send(:"#{field}=", value)
          expect(config.public_send(field)).to eq(value)

          config.public_send(:"#{field}=", nil)
          expect(config.public_send(field)).to be_nil
        end
      end
    end

    context "string fields" do
      {
        group_id: "group1",
        source_system: "system1",
        failure_topic: "fail_topic"
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
    it "sets all fields from the keyword hash" do
      config = described_class.new(
        bootstrap_servers: "server1",
        mock: true,
        send_timeout: 5.0,
        group_id: "group1",
        idempotence_cache_size: 100,
        subscribed_topics: %w[topic1 topic2],
        allowed_events: ["event1"],
        source_system: "system1",
        max_concurrency: 10,
        max_uncommitted: 5,
        max_enqueued_per_key: 3,
        stall_threshold: 2.0,
        shutdown_timeout: 3.0,
        poll_interval: 4.0,
        commit_interval: 5.0,
        mode: :pipeline,
        retry_base: 0.1,
        max_retries: 3,
        max_retry_delay: 1.0,
        failure_topic: "fail_topic",
        probe_port: 1234
      )

      expect(config.bootstrap_servers).to eq(["server1"])
      expect(config.mock).to eq(true)
      expect(config.send_timeout).to eq(5.0)
      expect(config.group_id).to eq("group1")
      expect(config.idempotence_cache_size).to eq(100)
      expect(config.subscribed_topics).to eq(%w[topic1 topic2])
      expect(config.allowed_events).to eq(["event1"])
      expect(config.source_system).to eq("system1")
      expect(config.max_concurrency).to eq(10)
      expect(config.max_uncommitted).to eq(5)
      expect(config.max_enqueued_per_key).to eq(3)
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
    end

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
    it "returns a hash with only non-nil values" do
      config.bootstrap_servers = "server1"
      config.mode = :pipeline
      config.send_timeout = 5.0

      native_hash = config.to_hash

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
