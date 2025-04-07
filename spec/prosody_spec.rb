# frozen_string_literal: true

RSpec.describe Prosody do
  it "has a version number" do
    expect(Prosody::VERSION).not_to be nil
  end

  it "creates a client" do
    expect { Prosody::NativeClient.new }.not_to raise_error
  end
end
