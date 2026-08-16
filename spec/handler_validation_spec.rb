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

  it "rejects an error wrapper over a missing handler" do
    handler_class = Class.new(Prosody::EventHandler) do
      permanent :on_excise, StandardError
      define_method(:on_message) { |_, _| nil }
      define_method(:on_timer) { |_, _| nil }
    end

    expect { Prosody::EventHandler.validate_handler!(handler_class.new) }
      .to raise_error(ArgumentError, "handler must implement #on_excise")
  end

  it "rejects a handler with the wrong arity" do
    handler_class = Class.new(Prosody::EventHandler) do
      define_method(:on_message) { |_, _| nil }
      define_method(:on_excise) { |_| nil }
      define_method(:on_timer) { |_, _| nil }
    end

    expect { Prosody::EventHandler.validate_handler!(handler_class.new) }
      .to raise_error(ArgumentError, "handler #on_excise must accept two parameters")
  end
end
