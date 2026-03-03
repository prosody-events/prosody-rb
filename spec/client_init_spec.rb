# frozen_string_literal: true

require "spec_helper"

RSpec.describe Prosody::Client, integration: true do
  it "initializes a client when given minimal producer configuration as hash" do
    # Create the client with a hash directly
    client = Prosody::Client.new(
      bootstrap_servers: TestConfig::BOOTSTRAP_SERVERS,
      source_system: "init-test-system"
    )

    # The client should be instantiated successfully
    expect(client).to be_a(Prosody::Client)
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
    client = Prosody::Client.new(config)

    # The client should be instantiated successfully
    expect(client).to be_a(Prosody::Client)
  end
end
