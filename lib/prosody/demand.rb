# frozen_string_literal: true

module Prosody
  # The demand a handler call serves, from +context.demand+.
  #
  # +kind+ is +:normal+ for a normal delivery and +:failure+ for a retry after
  # a failure. +retry+ is the retry ordinal: 0 for a normal delivery and 1 on
  # the first retry. After Prosody defers an event, the ordinal starts again
  # at 1. The ordinal is an estimate. Keep an exact attempt count in keyed
  # state if the handler needs one.
  #
  # @example Alert only on a retry
  #   def on_message(context, message)
  #     alert(message.key) if context.demand.failure?
  #   end
  Demand = Data.define(:kind, :retry) do
    # Whether this is a normal delivery.
    #
    # @return [Boolean]
    def normal? = kind == :normal

    # Whether this is a retry after a failure.
    #
    # @return [Boolean]
    def failure? = kind == :failure
  end
end
