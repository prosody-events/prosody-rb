# frozen_string_literal: true

require "spec_helper"

# Demand mapping across the native boundary: the core demand of each dispatch
# reaches the handler as a Prosody::Demand, for messages and for timers.
RSpec.describe Prosody::Demand do
  it "answers the kind predicates" do
    normal = described_class.new(kind: :normal, retry: 0)
    failure = described_class.new(:failure, 2)
    expect([normal.normal?, normal.failure?, normal.retry]).to eq([true, false, 0])
    expect([failure.normal?, failure.failure?, failure.retry]).to eq([false, true, 2])
    expect(failure).to be_frozen
  end

  describe "context.demand", integration: true do
    include_context "keyed state integration"

    it "reports normal delivery first and the retry ordinal after a failure" do
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink)
          @sink = sink
          @message_attempts = 0
          @timer_attempts = 0
        end

        def on_message(context, _message)
          @message_attempts += 1
          @sink.push([:message, context.demand])
          raise Prosody::TransientError, "fail the first attempt" if @message_attempts == 1

          context.schedule(Time.now + 1)
        end

        def on_timer(context, _timer)
          @timer_attempts += 1
          @sink.push([:timer, context.demand])
          raise Prosody::TransientError, "fail the first attempt" if @timer_attempts == 1
        end
      end

      client = build_client
      client.subscribe(handler_class.new(sink))
      client.send_message(topic, "k1", {go: true})

      observations = sink.wait(4)
      expect(observations).to eq([
        [:message, described_class.new(kind: :normal, retry: 0)],
        [:message, described_class.new(kind: :failure, retry: 1)],
        [:timer, described_class.new(kind: :normal, retry: 0)],
        [:timer, described_class.new(kind: :failure, retry: 1)]
      ])
    end
  end
end
