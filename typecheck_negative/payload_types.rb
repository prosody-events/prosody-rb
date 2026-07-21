# frozen_string_literal: true

# Deliberately invalid calls. Diagnostics are committed in
# steep_expectations.yml, so Steep fails if these errors disappear.
NEGATIVE_TOTALS = Prosody.map("negative-totals")
NEGATIVE_MESSAGES = Prosody.message_deque("negative-messages")

class NegativeTypeChecks
  def check(client, context, message)
    client.send_message("events", "key", Object.new)
    # @type var totals: Prosody::MapState[Integer]
    totals = context.state(NEGATIVE_TOTALS)
    totals.set("key", "not-an-integer")
    context.state(NEGATIVE_MESSAGES).push(message.payload)
  end
end
