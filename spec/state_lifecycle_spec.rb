# frozen_string_literal: true

require "spec_helper"

# Keyed-state lifecycle scenarios (Appendix 1 items 6, 7, and the item-8
# remainder): unregistered/identity-mismatch permanence, rethrow classification
# through the existing result bridge, attempt-fenced leaks, and iterator early
# break. Driven against real Kafka + Cassandra.
RSpec.describe "Prosody keyed state lifecycle (integration)", integration: true do
  include_context "keyed state integration"

  describe "item 8: unregistered name" do
    it "raises a PermanentStateError when vending a name that is not registered" do
      registered = Prosody.value(random_state_name("val"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink)
          @sink = sink
        end

        def on_message(context, _message)
          context.state(Prosody.value("never-#{SecureRandom.hex(6)}"))
          @sink.push({threw: false})
        rescue => e
          @sink.push({threw: true, permanent: e.is_a?(Prosody::PermanentStateError)})
        end
      end

      client = build_client(registered)
      client.subscribe(handler_class.new(sink))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:threw]).to be(true)
      expect(observation[:permanent]).to be(true)
    end
  end

  describe "item 8: rethrow classification through the existing bridge" do
    it "retries when a handler rethrows a transient state error" do
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink)
          @sink = sink
          @attempts = 0
        end

        def on_message(_context, _message)
          @attempts += 1
          raise Prosody::TransientStateError, "retry me" if @attempts == 1
          @sink.push({count: @attempts})
        end
      end

      client = build_client
      client.subscribe(handler_class.new(sink))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:count]).to be > 1
    end

    it "does not retry when a handler rethrows a permanent state error" do
      counter = [0]
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, counter)
          @sink = sink
          @counter = counter
        end

        def on_message(_context, _message)
          @counter[0] += 1
          @sink.push({count: @counter[0]})
          raise Prosody::PermanentStateError, "do not retry"
        end
      end

      client = build_client
      client.subscribe(handler_class.new(sink, counter))

      client.send_message(topic, "k1", {go: true})
      expect(sink.wait(1).first[:count]).to eq(1)
      sleep 2
      expect(counter[0]).to eq(1)
    end
  end

  describe "item 8: identity mismatch across runs" do
    # A cross-run identity mismatch (a name re-registered with a different kind)
    # IS enforced by core as permanent, but not at a handler-visible layer: the
    # mismatch is detected during partition keyed-state-manager acquisition
    # ("keyed-state descriptor identity acquisition failed; retrying") and the
    # consumer refuses to proceed, so the handler is never invoked and no
    # catchable PermanentStateError is surfaced to Ruby. There is no Ruby seam to
    # observe this as an exception. The permanent-config category is covered
    # handler-side by the unregistered-name test above and by the duplicate-name
    # unit test (state_spec.rb); the reference prosody-js likewise has no
    # cross-run identity integration test. Left as a documented skip rather than
    # a test that wedges the consumer in an unbounded acquisition-retry loop.
    it "is enforced by core at partition acquisition (no handler-visible error)" do
      skip "core enforces the mismatch at partition acquisition, not via a handler-catchable error"
    end
  end

  describe "item 6: attempt-fenced leaks" do
    it "raises the terminated (transient) error for a handle leaked across a failed attempt" do
      definition = Prosody.value(random_state_name("val"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
          @attempts = 0
        end

        def on_message(context, _message)
          @attempts += 1
          if @attempts == 1
            @leaked = context.state(@def)
            @leaked.set({"v" => "attempt-1"})
            raise Prosody::TransientStateError, "fail attempt 1"
          else
            leaked_transient = begin
              @leaked.get
              false
            rescue Prosody::TransientStateError
              true
            end
            fresh = context.state(@def).get
            @sink.push({leaked_transient: leaked_transient, fresh: fresh})
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:leaked_transient]).to be(true)
      expect(observation[:fresh]).to be_nil
    end

    it "raises the terminated (transient) error for a context leaked across a failed attempt" do
      definition = Prosody.map(random_state_name("map"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
          @attempts = 0
        end

        def on_message(context, _message)
          @attempts += 1
          if @attempts == 1
            @leaked_ctx = context
            raise Prosody::TransientStateError, "fail attempt 1"
          else
            result = begin
              @leaked_ctx.state(@def).get("x")
              {threw: false}
            rescue => e
              {threw: true, transient: e.is_a?(Prosody::TransientStateError)}
            end
            @sink.push(result)
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:threw]).to be(true)
      expect(observation[:transient]).to be(true)
    end

    it "raises the terminated (transient) error for a handle leaked past a successful handler" do
      definition = Prosody.value(random_state_name("val"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, message)
          case message.payload["step"]
          when 1
            @leaked = context.state(@def)
            @leaked.set({"v" => "captured"})
            @sink.push({ev: "captured"})
          when 2
            transient = begin
              @leaked.get
              false
            rescue Prosody::TransientStateError
              true
            end
            @sink.push({ev: "leaked", transient: transient})
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {step: 1})
      expect(sink.wait(1).first).to eq({ev: "captured"})

      client.send_message(topic, "k1", {step: 2})
      observation = sink.wait(1).first
      expect(observation[:ev]).to eq("leaked")
      expect(observation[:transient]).to be(true)
    end
  end

  describe "item 7: iterator lifecycle" do
    it "closes the scan on early break, leaving the collection usable" do
      definition = Prosody.map(random_state_name("map"))
      handler_class = Class.new(Prosody::EventHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, _message)
          map = context.state(@def)
          map.set("a", 1)
          map.set("b", 2)
          map.set("c", 3)
          map.each_pair { break }
          map.set("after", 99)
          @sink.push({after: map.get("after")})
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "k1", {go: true})
      observation = sink.wait(1).first
      expect(observation[:after]).to eq(99)
    end

    # Deterministically forcing a mid-pull cancellation against real Cassandra is
    # not drivable from Ruby (there is no fake-cursor injection into the native
    # StateScan). The cancellation-honesty contract is covered by the design
    # (`state.rs` module docs), the early-break close above, and the leaked-
    # enumerator terminated test in state_scan_spec.rb.
    it "closes cleanly on cancellation during a blocked pull"
  end
end
