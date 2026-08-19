# frozen_string_literal: true

require "prosody"

# The complete typed counterpart of the README burst-windowing example.
ACTIVITY_WINDOW = Prosody.value("activity-window")
PENDING_ACTIVITIES = Prosody.message_deque("pending-activities", capacity: 100)

class ActivityWindowHandler < Prosody::EventHandler
  def on_excise(context, message)
    puts "Excise #{message.key}"
    context.state(PENDING_ACTIVITIES).clear
    context.state(ACTIVITY_WINDOW).clear
    context.clear_scheduled
    nil
  end

  def on_message(context, message)
    window = context.state(ACTIVITY_WINDOW)
    pending = context.state(PENDING_ACTIVITIES)

    if window.get
      pending.push(message)
    else
      notify(message.key, [message])
      window.set(true)
      context.clear_and_schedule(Time.now + 5 * 60)
    end
  end

  def on_timer(context, timer)
    pending = context.state(PENDING_ACTIVITIES)
    batch = pending.each.to_a
    notify(timer.key, batch) unless batch.empty?
    pending.clear
    context.state(ACTIVITY_WINDOW).clear
  end

  private

  def notify(user_id, activities)
    puts "notify #{user_id}: #{activities.length} activities"
  end
end

if __FILE__ == $PROGRAM_NAME
  client = Prosody::Client.new(
    mock: true,
    group_id: "activity-window-example",
    subscribed_topics: "activities",
    state_collections: [ACTIVITY_WINDOW, PENDING_ACTIVITIES]
  )
  client.subscribe(ActivityWindowHandler.new)
  client.shutdown
end
