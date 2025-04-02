# frozen_string_literal: true

RSpec.describe Prosody do
  it "has a version number" do
    expect(Prosody::VERSION).not_to be nil
  end

  it "bridge ruby exec" do
    bridge = Prosody::Bridge::DynamicBridge.new
    bridge.test_ruby_exec
  end

  it "bridge rust exec" do
    bridge = Prosody::Bridge::DynamicBridge.new
    puts bridge.test_rust_exec
  end

end
