# frozen_string_literal: true

require "spec_helper"

RSpec.describe Prosody::NativeClient do
  client = described_class.new(Prosody::Configuration.new(
    source_system: "test-source",
    group_id: "test-consumer",
    subscribed_topics: "test-topic",
    probe_port: :disabled
  ))

  describe "production" do
    it "sends" do
      10.times do |i|
        client.send_message("test-topic", "test-ruby-key-#{i}", {"hello" => "world"})
      end
    end

    it "subscribes" do
      puts "subscribing"
      client.subscribe(MyHandler.new)

      puts "sleeping for 15 seconds"
      sleep 15

      puts "unsubscribing"
      client.unsubscribe
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
