# frozen_string_literal: true

# This file is checked by Steep as a consumer of Prosody's public signatures.
# It is not loaded at runtime.

ORDER_BACKLOG = Prosody.message_deque("order-backlog")
ORDER_TOTALS = Prosody.map("order-totals")

class TypedOrderHandler < Prosody::EventHandler
  def on_excise(_context, message)
    message.key
    {"accepted" => true}
  end

  def on_message(context, message)
    payload = message.payload
    order_id = payload["order_id"]
    demand = context.demand
    consume_order(order_id, demand.retry) if demand.failure?
    total = payload["total"]

    # Message-backed state preserves the handler's payload type.
    backlog = context.state(ORDER_BACKLOG)
    backlog.push(message)
    oldest = backlog.first
    backlog.reverse_each(range: 0..9, limit: 2) { |pending| pending.payload["order_id"].upcase }

    # This call is a regression constraint: state(ORDER_TOTALS) must infer as
    # MapState[Integer], not untyped or a generic JSON-valued state handle.
    consume_totals(context.state(ORDER_TOTALS))

    consume_order(order_id, total)
    oldest_payload = oldest&.payload
    consume_order(oldest_payload["order_id"], oldest_payload["total"]) if oldest_payload
    {"accepted" => true}
  end

  def on_timer(_context, _timer)
  end

  private

  def consume_order(order_id, total)
    "#{order_id.upcase}:#{total + 1}"
  end

  def consume_totals(totals)
    totals.set("latest", 1)
    # Query keywords keep the item types.
    totals.each_key(prefix: "order:", after: "order:1", limit: 10) { |key| key.upcase }
    totals.reverse_each_value(range: "a"..."m").each { |total| total + 1 }
    nil
  end
end

class DefaultPayloadConsumer
  def payload(message)
    message.payload
  end
end
