# frozen_string_literal: true

require "spec_helper"
require "sentry-ruby"
require "sentry/test_helper"

RSpec.describe Prosody::SentryIntegration do
  include Sentry::TestHelper

  before do
    Sentry.init do |config|
      config.dsn = Sentry::TestHelper::DUMMY_DSN
    end
    setup_sentry_test
  end

  after { teardown_sentry_test }

  describe ".enabled?" do
    context "when Sentry is initialized" do
      it "returns true" do
        expect(described_class.enabled?).to be true
      end
    end

    context "when Sentry is not initialized" do
      it "returns false" do
        allow(Sentry).to receive(:initialized?).and_return(false)
        expect(described_class.enabled?).to be false
      end
    end
  end

  describe ".capture_exception" do
    let(:exception) { RuntimeError.new("test error") }

    context "when enabled" do
      it "captures the exception" do
        described_class.capture_exception(exception)
        expect(sentry_events).not_to be_empty
      end

      it "sets the prosody context" do
        context_data = {key: "value", event_type: "some_event"}
        described_class.capture_exception(exception, context_data)
        expect(last_sentry_event.contexts["prosody"]).to include(key: "value", event_type: "some_event")
      end

      it "sets the prosody.event_type tag" do
        described_class.capture_exception(exception, {event_type: "my_event"})
        expect(last_sentry_event.tags["prosody.event_type"]).to eq("my_event")
      end

      it "does not set the prosody.event_type tag when event_type is nil" do
        described_class.capture_exception(exception, {key: "value"})
        expect(last_sentry_event.tags).not_to have_key("prosody.event_type")
      end
    end

    context "when disabled" do
      it "does not capture any events" do
        allow(Sentry).to receive(:initialized?).and_return(false)
        expect(Sentry).not_to receive(:capture_exception)
        described_class.capture_exception(exception)
      end
    end
  end
end
