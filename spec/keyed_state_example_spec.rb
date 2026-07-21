# frozen_string_literal: true

require "spec_helper"

RSpec.describe "keyed-state example", :source_tree do
  example_path = File.expand_path("../examples/keyed_state.rb", __dir__)

  it "parses as valid Ruby" do
    expect { RubyVM::InstructionSequence.compile(File.read(example_path)) }
      .not_to raise_error
  end

  it "defines definitions that serialize into a client config" do
    load example_path
    config = Prosody::Configuration.new
    config.state_collections = [CART, TOTALS, BACKLOG]
    serialized = config.to_hash[:state_collections]
    expect(serialized).to include(
      hash_including(name: "cart", kind: "value", payload: "json", ttl_seconds: 2_592_000),
      hash_including(name: "totals", kind: "map", payload: "json"),
      hash_including(name: "backlog", kind: "deque", payload: "message", capacity: 100)
    )
  end
end
