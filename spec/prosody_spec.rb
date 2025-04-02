# frozen_string_literal: true

RSpec.describe Prosody do
  it "has a version number" do
    expect(Prosody::VERSION).not_to be nil
  end

  it "bridges" do
    bridge = Prosody::Bridge::DynamicBridge.new

    5.times do
      bridge.feed
    end
  end

end
