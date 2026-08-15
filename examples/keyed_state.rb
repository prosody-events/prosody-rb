# frozen_string_literal: true

require "prosody"
require "logger"

# Keyed-state example: per-key value/map/deque collections that survive across
# events. Definitions are declared once and reused for both registration (on the
# client) and binding (inside the handler). Every state op yields the fiber,
# never the thread.

# Definitions: declared once, reused for registration and binding. See
# keyed_state.rbs for payload and state types checked by Steep.
CART = Prosody.value("cart", ttl: 30 * 24 * 3600)      # ValueState
TOTALS = Prosody.map("totals")                          # keys are always String
BACKLOG = Prosody.message_deque("backlog", capacity: 100) # bounded window of messages

class KeyedStateHandler < Prosody::EventHandler
  def initialize(logger:)
    @logger = logger
  end

  def on_message(context, message)
    cart = context.state(CART)             # bound for this attempt only
    current = cart.get || {"items" => []}  # Hash, or nil when absent
    cart.set(current.merge("items" => current["items"] + [message.payload["order_id"]]))

    totals = context.state(TOTALS)
    totals.set(message.key, message.payload["total"])
    # Steep infers key as String and total as Integer from TOTALS's RBS type.
    totals.each_pair { |key, total| @logger.info(format_total(key, total)) }

    backlog = context.state(BACKLOG)
    backlog.push(message)                  # stores the full Prosody::Message
    oldest = backlog.get(0)                # Message[order_event]?, preserving payload shape
    @logger.info("oldest order: #{format_order_id(oldest.payload["order_id"])}") if oldest
  end

  private

  def format_total(key, total)
    "#{key}=#{total}"
  end

  def format_order_id(order_id)
    order_id
  end
end

if __FILE__ == $PROGRAM_NAME
  client = Prosody::Client.new(
    mock: true,
    group_id: "keyed-state-example",
    subscribed_topics: "orders",
    state_collections: [CART, TOTALS, BACKLOG]
  )
  client.subscribe(KeyedStateHandler.new(logger: Logger.new($stdout)))
  client.shutdown
end
