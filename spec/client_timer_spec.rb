# frozen_string_literal: true

require "spec_helper"

# Integration tests for timer scheduling and firing through Prosody::Context.
RSpec.describe Prosody::Client, integration: true do
  include_context "client integration"

  # Helper methods for timer operations
  def create_timer_test_handler(event_stream)
    Class.new do
      def initialize(event_stream)
        @event_stream = event_stream
      end

      def on_message(context, message)
        case message.payload["action"]
        when "test_schedule_multiple"
          test_schedule_multiple_timers(context)
        when "test_unschedule"
          test_unschedule_specific_timer(context)
        when "test_clear_and_schedule"
          test_clear_and_schedule_operation(context)
        when "test_clear_scheduled"
          test_clear_all_scheduled_timers(context)
        when "test_scheduled_empty"
          test_scheduled_when_empty(context)
        end
      end

      def on_timer(context, timer)
        # Timer fired - not used in these tests
      end

      def on_excise(_context, _message)
      end

      private

      def test_schedule_multiple_timers(context)
        times = create_future_times([30, 60, 90])
        schedule_times(context, times)

        scheduled_times = context.scheduled
        @event_stream.push({
          operation: :schedule_multiple,
          scheduled_count: scheduled_times.length,
          times_are_time_objects: scheduled_times.all? { |t| t.is_a?(Time) },
          original_times: times
        })
      end

      def test_unschedule_specific_timer(context)
        times = create_future_times([40, 80])
        schedule_times(context, times)

        before_count = context.scheduled.length
        context.unschedule(times.first)
        after_count = context.scheduled.length

        @event_stream.push({
          operation: :unschedule,
          before_count: before_count,
          after_count: after_count,
          unscheduled_time: times.first,
          remaining_time: times.last
        })
      end

      def test_clear_and_schedule_operation(context)
        initial_times = create_future_times([50, 100])
        new_time = create_future_times([150]).first

        schedule_times(context, initial_times)
        before_count = context.scheduled.length

        context.clear_and_schedule(new_time)
        after_scheduled = context.scheduled

        @event_stream.push({
          operation: :clear_and_schedule,
          before_count: before_count,
          after_count: after_scheduled.length,
          new_time: new_time,
          time_matches: after_scheduled.any? { |t| (t.to_i - new_time.to_i).abs <= 1 }
        })
      end

      def test_clear_all_scheduled_timers(context)
        times = create_future_times([70, 140])
        schedule_times(context, times)

        before_count = context.scheduled.length
        context.clear_scheduled
        after_count = context.scheduled.length

        @event_stream.push({
          operation: :clear_scheduled,
          before_count: before_count,
          after_count: after_count,
          completely_cleared: after_count == 0
        })
      end

      def test_scheduled_when_empty(context)
        scheduled_times = context.scheduled

        @event_stream.push({
          operation: :scheduled_empty,
          scheduled_count: scheduled_times.length,
          is_array: scheduled_times.is_a?(Array),
          is_empty: scheduled_times.empty?
        })
      end

      def create_future_times(offsets)
        now = Time.now
        offsets.map { |offset| now + offset }
      end

      def schedule_times(context, times)
        times.each { |time| context.schedule(time) }
      end
    end
  end

  def send_timer_test_messages(client, topic)
    client.send_message(topic, "timer-ops-1", {action: "test_schedule_multiple"})
    client.send_message(topic, "timer-ops-2", {action: "test_unschedule"})
    client.send_message(topic, "timer-ops-3", {action: "test_clear_and_schedule"})
    client.send_message(topic, "timer-ops-4", {action: "test_clear_scheduled"})
    client.send_message(topic, "timer-ops-5", {action: "test_scheduled_empty"})
  end

  def verify_timer_operations(timer_operations_data)
    expect(timer_operations_data.length).to eq(5)

    verify_schedule_multiple_operation(timer_operations_data)
    verify_unschedule_operation(timer_operations_data)
    verify_clear_and_schedule_operation(timer_operations_data)
    verify_clear_scheduled_operation(timer_operations_data)
    verify_scheduled_empty_operation(timer_operations_data)
  end

  def verify_schedule_multiple_operation(data)
    schedule_data = data.find { |d| d[:operation] == :schedule_multiple }
    expect(schedule_data).not_to be_nil
    expect(schedule_data[:scheduled_count]).to eq(3)
    expect(schedule_data[:times_are_time_objects]).to eq(true)
  end

  def verify_unschedule_operation(data)
    unschedule_data = data.find { |d| d[:operation] == :unschedule }
    expect(unschedule_data).not_to be_nil
    expect(unschedule_data[:before_count]).to eq(2)
    expect(unschedule_data[:after_count]).to eq(1)
  end

  def verify_clear_and_schedule_operation(data)
    clear_and_schedule_data = data.find { |d| d[:operation] == :clear_and_schedule }
    expect(clear_and_schedule_data).not_to be_nil
    expect(clear_and_schedule_data[:before_count]).to eq(2)
    expect(clear_and_schedule_data[:after_count]).to eq(1)
    expect(clear_and_schedule_data[:time_matches]).to eq(true)
  end

  def verify_clear_scheduled_operation(data)
    clear_data = data.find { |d| d[:operation] == :clear_scheduled }
    expect(clear_data).not_to be_nil
    expect(clear_data[:before_count]).to eq(2)
    expect(clear_data[:after_count]).to eq(0)
    expect(clear_data[:completely_cleared]).to eq(true)
  end

  def verify_scheduled_empty_operation(data)
    empty_data = data.find { |d| d[:operation] == :scheduled_empty }
    expect(empty_data).not_to be_nil
    expect(empty_data[:scheduled_count]).to eq(0)
    expect(empty_data[:is_array]).to eq(true)
    expect(empty_data[:is_empty]).to eq(true)
  end

  # Test comprehensive timer functionality
  it "provides comprehensive timer scheduling operations" do
    timer_stream = new_sink
    handler = create_timer_test_handler(timer_stream)

    client.subscribe(handler.new(timer_stream))
    send_timer_test_messages(client, topic)

    timer_operations_data = timer_stream.wait(5)
    verify_timer_operations(timer_operations_data)
  end

  def create_timer_firing_test_handler(event_stream)
    Class.new do
      def initialize(event_stream)
        @event_stream = event_stream
      end

      def on_message(context, message)
        if message.payload["action"] == "test_timer_firing"
          schedule_timers_for_firing_test(context)
        end
      end

      def on_timer(context, timer)
        capture_timer_firing_event(timer)
      end

      def on_excise(_context, _message)
      end

      private

      def schedule_timers_for_firing_test(context)
        now = Time.now
        timer1_time = now + 1
        timer2_time = now + 2

        context.schedule(timer1_time)
        context.schedule(timer2_time)

        @event_stream.push({
          type: :scheduled,
          scheduled_at: now,
          timer1_target: timer1_time,
          timer2_target: timer2_time,
          scheduled_count: context.scheduled.length
        })
      end

      def capture_timer_firing_event(timer)
        fired_at = Time.now
        @event_stream.push({
          type: :timer_fired,
          key: timer.key,
          timer_time: timer.time,
          actual_fire_time: fired_at,
          timer_time_class: timer.time.class.name
        })
      end
    end
  end

  def verify_timer_scheduling_event(scheduling_events)
    expect(scheduling_events.length).to eq(1)

    scheduled_event = scheduling_events.first
    expect(scheduled_event[:type]).to eq(:scheduled)
    expect(scheduled_event[:scheduled_count]).to eq(2)

    scheduled_event
  end

  def verify_timer_firing_events(fired_events, scheduled_event)
    expect(fired_events.length).to eq(2)

    fired_events.each do |event|
      verify_timer_properties(event)
      verify_timer_accuracy(event, scheduled_event)
    end

    verify_timer_firing_order(fired_events)
  end

  def verify_timer_properties(event)
    expect(event[:type]).to eq(:timer_fired)
    expect(event[:key]).to eq("timer-fire-test")
    expect(event[:timer_time_class]).to eq("Time")
    expect(event[:timer_time]).to be_a(Time)
  end

  def verify_timer_accuracy(event, scheduled_event)
    expected_target = if event[:timer_time].to_i == scheduled_event[:timer1_target].to_i
      scheduled_event[:timer1_target]
    else
      scheduled_event[:timer2_target]
    end

    expect(event[:actual_fire_time]).to be_within(1).of(expected_target)
    expect(event[:timer_time].to_i).to be_within(1).of(expected_target.to_i)
  end

  def verify_timer_firing_order(fired_events)
    first_timer = fired_events.min_by { |e| e[:actual_fire_time] }
    second_timer = fired_events.max_by { |e| e[:actual_fire_time] }

    expect(first_timer[:actual_fire_time]).to be < second_timer[:actual_fire_time]
  end

  # Test that timers actually fire at the correct time
  it "fires timers at the correct time with proper Ruby Time objects" do
    timer_stream = new_sink
    handler = create_timer_firing_test_handler(timer_stream)

    client.subscribe(handler.new(timer_stream))
    client.send_message(topic, "timer-fire-test", {action: "test_timer_firing"})

    scheduling_events = timer_stream.wait(1)
    scheduled_event = verify_timer_scheduling_event(scheduling_events)

    fired_events = timer_stream.wait(2, 5)
    verify_timer_firing_events(fired_events, scheduled_event)
  end
end
