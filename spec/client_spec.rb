# frozen_string_literal: true

require "spec_helper"

RSpec.describe Prosody::NativeClient do
  client = described_class.new(Prosody::Configuration.new(
    source_system: "test-source",
    probe_port: :disabled
  ))

  describe "production" do
    it "sends" do
      10.times do |i|
        client.send_message("test-topic", "test-ruby-key-#{i}", {"hello" => "world"})
      end
    end
  end
end

class MyHandler < Prosody::EventHandler
  def on_message(context, message)
    puts "got message: #{message.payload}! Sleeping for 1 seconds"
    sleep 1
    puts "done processing"
  end
end
