# frozen_string_literal: true

require "spec_helper"

RSpec.describe Prosody do
  it "has a version number" do
    expect(Prosody::VERSION).not_to be nil
  end
end
