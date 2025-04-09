# frozen_string_literal: true

require "spec_helper"

RSpec.describe Prosody::NativeClient do
  let(:configuration) { Prosody::Configuration.new(bootstrap_servers: "localhost:9094", source_system: "test") }
  subject(:client) { described_class.new(configuration) }

  describe "production" do
    it "sends" do
      client.send_message("test-topic", "test-ruby-key", {"hello" => "world"})
    end
  end
end
