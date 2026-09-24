# frozen_string_literal: true

require "spec_helper"

# Query keyword translation across the native boundary. One handler run
# writes a map and a deque, then evaluates every query keyword, range form,
# and translation error against the real native handles. Published readers
# repeat a subset to prove their wiring. Core owns the query semantics; these
# examples check that each Ruby keyword reaches the matching core setting.
RSpec.describe "Prosody keyed state queries", integration: true do
  include_context "keyed state integration"

  KEYS = %w[a1 a2 b1 b2 c1].freeze
  ITEMS = [10, 11, 12, 13, 14].freeze

  # Each entry is [label, query method, keywords, expected keys or items].
  MAP_QUERIES = [
    [:all, :each_key, {}, KEYS],
    [:unset_keywords, :each_key, {from: nil, limit: nil}, KEYS],
    [:prefix, :each_key, {prefix: "a"}, %w[a1 a2]],
    [:from, :each_key, {from: "a2"}, %w[a2 b1 b2 c1]],
    [:after, :each_key, {after: "a2"}, %w[b1 b2 c1]],
    [:to, :each_key, {to: "b1"}, %w[a1 a2 b1]],
    [:before, :each_key, {before: "b1"}, %w[a1 a2]],
    [:reverse_from, :reverse_each_key, {from: "b2"}, %w[b2 b1 a2 a1]],
    [:reverse_after_before, :reverse_each_key, {after: "b2", before: "a1"}, %w[b1 a2]],
    [:inclusive_range, :each_key, {range: "a2".."b2"}, %w[a2 b1 b2]],
    [:exclusive_range, :each_key, {range: "a2"..."b2"}, %w[a2 b1]],
    [:beginless_range, :each_key, {range: .."a2"}, %w[a1 a2]],
    [:endless_range, :each_key, {range: "b2"..}, %w[b2 c1]],
    [:reverse_range, :reverse_each_key, {range: "a2".."b2"}, %w[b2 b1 a2]],
    [:limit, :each_key, {limit: 2}, %w[a1 a2]],
    [:reverse_limit, :reverse_each_key, {limit: 2}, %w[c1 b2]],
    [:prefix_after, :each_key, {prefix: "b", after: "b1"}, %w[b2]],
    [:page, :each_key, {after: "a2", limit: 2}, %w[b1 b2]],
    [:reverse_page, :reverse_each_key, {after: "b1", limit: 2}, %w[a2 a1]],
    [:pairs, :each_pair, {prefix: "b"}, [["b1", 2], ["b2", 3]]],
    [:reverse_pairs, :reverse_each_pair, {limit: 1}, [["c1", 4]]],
    [:values, :each_value, {prefix: "b"}, [2, 3]],
    [:reverse_values, :reverse_each_value, {prefix: "a"}, [1, 0]]
  ].freeze

  DEQUE_QUERIES = [
    [:all, :each, {}, ITEMS],
    [:from, :each, {from: 1}, [11, 12, 13, 14]],
    [:after, :each, {after: 1}, [12, 13, 14]],
    [:to, :each, {to: 2}, [10, 11, 12]],
    [:before, :each, {before: 2}, [10, 11]],
    [:reverse_from, :reverse_each, {from: 3}, [13, 12, 11, 10]],
    [:reverse_after_to, :reverse_each, {after: 3, to: 1}, [12, 11]],
    [:inclusive_range, :each, {range: 1..3}, [11, 12, 13]],
    [:exclusive_range, :each, {range: 1...3}, [11, 12]],
    [:beginless_range, :each, {range: ..1}, [10, 11]],
    [:endless_range, :each, {range: 3..}, [13, 14]],
    [:reverse_range, :reverse_each, {range: 1..3}, [13, 12, 11]],
    [:limit, :each, {limit: 2}, [10, 11]],
    [:reverse_limit, :reverse_each, {limit: 2}, [14, 13]],
    [:page, :each, {after: 1, limit: 2}, [12, 13]]
  ].freeze

  # Each entry is [label, handle, query method, keywords, error class, message].
  QUERY_ERRORS = [
    [:zero_limit, :map, :each_key, {limit: 0}, ArgumentError, /limit/],
    [:negative_limit, :map, :each_key, {limit: -1}, ArgumentError, /limit/],
    [:fractional_limit, :map, :each_key, {limit: 1.5}, ArgumentError, /limit/],
    [:string_limit, :map, :each_key, {limit: "2"}, ArgumentError, /limit/],
    [:both_starts, :map, :each_key, {from: "a", after: "b"}, ArgumentError, /from.*after/],
    [:both_ends, :map, :each_pair, {to: "a", before: "b"}, ArgumentError, /to.*before/],
    [:descending_keys, :map, :each_key, {range: "b".."a"}, ArgumentError, /ascending/],
    [:unknown_keyword, :map, :each_key, {bogus: 1}, ArgumentError, /unknown keyword: :bogus/],
    [:integer_key, :map, :each_key, {from: 1}, TypeError, /String/],
    [:range_type, :map, :each_key, {range: "a"}, TypeError, /Range/],
    [:negative_position, :deque, :each, {from: -1}, ArgumentError, /from.*non-negative/],
    [:fractional_position, :deque, :each, {before: 1.5}, ArgumentError, /before.*non-negative/],
    [:negative_range, :deque, :each, {range: -2..-1}, ArgumentError, /range.*non-negative/],
    [:descending_positions, :deque, :reverse_each, {range: 3..1}, ArgumentError, /ascending/],
    [:deque_prefix, :deque, :each, {prefix: "x"}, ArgumentError, /unknown keyword: :prefix/]
  ].freeze

  it "translates every query keyword for owned and published collections" do
    subsystem = "query-#{SecureRandom.hex(4)}"
    map_definition = Prosody.map(random_state_name("map"), published: true, read_cache: false)
    deque_definition = Prosody.deque(random_state_name("deque"), published: true, read_cache: false)
    handler_class = Class.new(CompleteHandler) do
      def initialize(sink, map_definition, deque_definition)
        @sink = sink
        @map_definition = map_definition
        @deque_definition = deque_definition
      end

      def on_message(context, _message)
        map = context.state(@map_definition)
        KEYS.each_with_index { |key, index| map.set(key, index) }
        deque = context.state(@deque_definition)
        ITEMS.each { |item| deque.push(item) }
        handles = {map: map, deque: deque}

        results = {}
        MAP_QUERIES.each { |label, method, query, _| results[[:map, label]] = map.public_send(method, **query).to_a }
        DEQUE_QUERIES.each { |label, method, query, _| results[[:deque, label]] = deque.public_send(method, **query).to_a }

        errors = QUERY_ERRORS.to_h do |label, handle, method, query, _, _|
          handles.fetch(handle).public_send(method, **query).to_a
          [label, nil]
        rescue ArgumentError, TypeError => e
          [label, e]
        end

        enumerator = map.each_key(prefix: "b")
        streamed = [enumerator.next, enumerator.next]
        exhausted = begin
          enumerator.next
          false
        rescue StopIteration
          true
        end

        map.commit
        deque.commit
        @sink.push({results: results, errors: errors, streamed: streamed, exhausted: exhausted})
      end
    end

    client = build_client(map_definition, deque_definition, subsystem: subsystem)
    client.subscribe(handler_class.new(sink, map_definition, deque_definition))
    client.send_message(topic, "k1", {go: true})
    observation = sink.wait(1).first
    expect(observation).not_to be_nil

    MAP_QUERIES.each do |label, _, _, expected|
      expect([label, observation[:results][[:map, label]]]).to eq([label, expected])
    end
    DEQUE_QUERIES.each do |label, _, _, expected|
      expect([label, observation[:results][[:deque, label]]]).to eq([label, expected])
    end
    QUERY_ERRORS.each do |label, _, _, _, error_class, message|
      error = observation[:errors][label]
      expect([label, error.class]).to eq([label, error_class])
      expect(error.message).to match(message)
    end
    expect(observation[:streamed]).to eq(%w[b1 b2])
    expect(observation[:exhausted]).to be(true)

    map_reader = client.state(subsystem, map_definition)
    expect(map_reader.each_key("k1", prefix: "b").to_a).to eq(%w[b1 b2])
    expect(map_reader.reverse_each_pair("k1", after: "b1", limit: 2).to_a).to eq([["a2", 1], ["a1", 0]])
    expect(map_reader.each_value("k1", range: "a2"..."b2").to_a).to eq([1, 2])
    expect { map_reader.each_key("k1", from: "a", after: "b").to_a }.to raise_error(ArgumentError, /from.*after/)

    deque_reader = client.state(subsystem, deque_definition)
    expect(deque_reader.each("k1", range: 1..2).to_a).to eq([11, 12])
    expect(deque_reader.reverse_each("k1", limit: 2).to_a).to eq([14, 13])
    expect { deque_reader.each("k1", from: -1).to_a }.to raise_error(ArgumentError, /non-negative/)
  end
end
