# frozen_string_literal: true

require "spec_helper"

RSpec.describe "handler validation" do
  %i[on_message on_excise on_timer].each do |missing|
    it "rejects a missing #{missing} before subscription" do
      client = Prosody::Client.new(
        mock: true,
        group_id: "handler-validation",
        bootstrap_servers: "localhost:9094",
        subscribed_topics: "events"
      )
      handler_class = Class.new(Prosody::EventHandler)
      (%i[on_message on_excise on_timer] - [missing]).each do |method_name|
        handler_class.define_method(method_name) { |*_args| nil }
      end

      expect { client.subscribe(handler_class.new) }
        .to raise_error(ArgumentError, "handler must implement ##{missing}")
      expect(client.consumer_state).to eq(:configured)
    ensure
      client&.shutdown
    end
  end
end
