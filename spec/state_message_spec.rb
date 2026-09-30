# frozen_string_literal: true

require "spec_helper"

# Message collections against real Kafka and Cassandra: stored messages
# round-trip through value, deque, and map collections.
RSpec.describe "Prosody message state (integration)", integration: true do
  include_context "keyed state integration"

  describe "item 4: message collections" do
    it "round-trips a stored message through a message value collection" do
      definition = Prosody.message_value(random_state_name("mval"))
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, message)
          value = context.state(@def)
          case message.payload["step"]
          when 1
            value.set(message)
            @sink.push({ev: "stored", offset: message.offset})
          when 2
            got = value.get
            @sink.push({
              ev: "read", topic: got.topic, partition: got.partition,
              offset: got.offset, key: got.key, payload: got.payload,
              live_offset: message.offset,
              native_class: value.instance_variable_get(:@native).class.name
            })
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "mk", {step: 1})
      stored = sink.wait(1).first
      expect(stored[:ev]).to eq("stored")

      client.send_message(topic, "mk", {step: 2})
      read = sink.wait(1).first
      expect(read[:topic]).to eq(topic)
      expect(read[:key]).to eq("mk")
      expect(read[:payload]).to eq({"step" => 1})
      expect(read[:offset]).to eq(stored[:offset])
      expect(read[:offset]).not_to eq(read[:live_offset])
      expect(read[:native_class]).to eq("Prosody::NativeMessageValueState")
    end

    it "round-trips a stored message through a message deque collection" do
      definition = Prosody.message_deque(random_state_name("mdeq"))
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, message)
          deque = context.state(@def)
          case message.payload["step"]
          when 1
            deque.push(message)
            @sink.push({ev: "stored", offset: message.offset})
          when 2
            got = deque.get(0)
            scanned = []
            deque.each { |item| scanned << item.offset }
            native = deque.instance_variable_get(:@native)
            cursor = native.scan(:forward, {})
            cursor_class = cursor.class.name
            cursor.close
            @sink.push({
              ev: "read", topic: got.topic, offset: got.offset,
              key: got.key, payload: got.payload, live_offset: message.offset,
              scanned: scanned, native_class: native.class.name,
              cursor_class: cursor_class
            })
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "mk", {step: 1})
      stored = sink.wait(1).first
      expect(stored[:ev]).to eq("stored")

      client.send_message(topic, "mk", {step: 2})
      read = sink.wait(1).first
      expect(read[:topic]).to eq(topic)
      expect(read[:payload]).to eq({"step" => 1})
      expect(read[:offset]).to eq(stored[:offset])
      expect(read[:offset]).not_to eq(read[:live_offset])
      expect(read[:scanned]).to eq([stored[:offset]])
      expect(read[:native_class]).to eq("Prosody::NativeMessageDequeState")
      expect(read[:cursor_class]).to eq("Prosody::NativeMessageDequeScan")
    end

    it "round-trips messages through a message map collection with unicode keys and get_many" do
      definition = Prosody.message_map(random_state_name("mmap"))
      handler_class = Class.new(CompleteHandler) do
        def initialize(sink, definition)
          @sink = sink
          @def = definition
        end

        def on_message(context, message)
          map = context.state(@def)
          case message.payload["step"]
          when 1
            map.set("primary", message)
            map.set("café", message)
            @sink.push({ev: "stored", offset: message.offset})
          when 2
            primary = map.get("primary")
            unicode = map.get("café")
            batch = map.get_many(["primary", "café", "absent"])
            scanned = []
            map.each_pair { |key, value| scanned << [key, value.offset] }
            native = map.instance_variable_get(:@native)
            value_cursor = native.scan(:forward, {})
            value_cursor_class = value_cursor.class.name
            value_cursor.close
            key_cursor = native.keys(:forward, {})
            key_cursor_class = key_cursor.class.name
            key_cursor.close
            @sink.push({
              ev: "read",
              primary_offset: primary.offset,
              primary_payload: primary.payload,
              unicode_offset: unicode.offset,
              absent: map.get("absent"),
              batch_length: batch.length,
              batch_nils: batch.count(&:nil?),
              live_offset: message.offset,
              scanned: scanned, native_class: native.class.name,
              value_cursor_class: value_cursor_class,
              key_cursor_class: key_cursor_class
            })
          end
        end
      end

      client = build_client(definition)
      client.subscribe(handler_class.new(sink, definition))

      client.send_message(topic, "mk", {step: 1})
      stored = sink.wait(1).first
      expect(stored[:ev]).to eq("stored")

      client.send_message(topic, "mk", {step: 2})
      read = sink.wait(1).first
      expect(read[:primary_payload]).to eq({"step" => 1})
      expect(read[:primary_offset]).to eq(stored[:offset])
      expect(read[:unicode_offset]).to eq(stored[:offset])
      expect(read[:primary_offset]).not_to eq(read[:live_offset])
      expect(read[:absent]).to be_nil
      expect(read[:batch_length]).to eq(3)
      expect(read[:batch_nils]).to eq(1)
      expect(read[:scanned]).to eq([["café", stored[:offset]], ["primary", stored[:offset]]])
      expect(read[:native_class]).to eq("Prosody::NativeMessageMapState")
      expect(read[:value_cursor_class]).to eq("Prosody::NativeMessageMapScan")
      expect(read[:key_cursor_class]).to eq("Prosody::NativeMapKeyScan")
    end
  end
end
