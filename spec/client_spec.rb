# frozen_string_literal: true

require "spec_helper"
require "opentelemetry/sdk"

RSpec.describe Prosody::Client do
  client = described_class.new(Prosody::Configuration.new(
    source_system: "test-source",
    group_id: "test-group",
    subscribed_topics: "test-topic",
    probe_port: :disabled
  ))

  describe "production" do
    OpenTelemetry::SDK.configure do |c|
      c.service_name = "ruby-test"
    end

    tracer = OpenTelemetry.tracer_provider.tracer("test")

    it "sends" do
      tracer.in_span("test_span") do |span|
        32.times do |i|
          client.send_message("test-topic", "test-ruby-key-#{i}", {hello: "world"})
        end
      end
    end

    # it "subscribes" do
    #   puts "subscribing"
    #   client.subscribe(MyHandler.new)
    #
    #   puts "sleeping for 5 seconds"
    #   sleep 5
    #
    #   puts "unsubscribing"
    #   client.unsubscribe
    # end
  end
end

class MyHandler < Prosody::EventHandler
  def on_message(context, message)
    puts "got message: #{message.payload}! Sleeping for 1 seconds"
    sleep 1
    puts "done processing"
  end
end
