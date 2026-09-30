# frozen_string_literal: true

require "spec_helper"
require "securerandom"

# Prosody::AdminClient against a real Kafka broker.
RSpec.describe "Prosody::AdminClient", integration: true do
  let(:admin) { Prosody.const_get(:AdminClient).new([TestConfig::BOOTSTRAP_SERVERS]) }
  let(:topic) { "admin-test-#{SecureRandom.hex(4)}" }

  after do
    admin.delete_topic(topic)
  rescue
    # The topic was never created.
  end

  it "creates a topic with a cleanup policy and a retention" do
    expect { admin.create_topic(topic, 1, 1, cleanup_policy: "compact", retention: 3600) }.not_to raise_error
  end

  # The broker rejects an unknown cleanup policy, so the error proves that the
  # keyword reaches the broker.
  it "passes the cleanup policy to the broker" do
    expect { admin.create_topic(topic, 1, 1, cleanup_policy: "bogus") }.to raise_error(RuntimeError)
  end

  it "rejects a retention with no duration form" do
    expect { admin.create_topic(topic, 1, 1, retention: -1) }
      .to raise_error(ArgumentError, /retention: must be a finite, non-negative number of seconds/)
  end
end
