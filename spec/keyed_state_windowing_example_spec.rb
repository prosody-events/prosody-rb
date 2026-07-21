# frozen_string_literal: true

require "spec_helper"

RSpec.describe "keyed-state windowing example", :source_tree do
  example_path = File.expand_path("../examples/keyed_state_windowing.rb", __dir__)

  it "parses as valid Ruby" do
    expect { RubyVM::InstructionSequence.compile(File.read(example_path)) }
      .not_to raise_error
  end

  it "defines the collections shown in the README" do
    load example_path
    config = Prosody::Configuration.new
    config.state_collections = [ACTIVITY_WINDOW, PENDING_ACTIVITIES]

    expect(config.to_hash[:state_collections]).to include(
      hash_including(name: "activity-window", kind: "value", payload: "json"),
      hash_including(
        name: "pending-activities",
        kind: "deque",
        payload: "message",
        capacity: 100
      )
    )
  end
end
