# frozen_string_literal: true

# Native stubs for {Prosody::Context}: event metadata, cancellation, timer
# scheduling, and keyed-state vending.

module Prosody
  # Represents the context of a Kafka message, providing metadata and control
  # capabilities for message handling.
  #
  # Instances of this class are created by the native code and passed to your
  # EventHandler's #on_message method.
  #
  # @see ext/prosody/src/handler/context.rs for implementation
  class Context
    # @private
    def initialize
      raise NotImplementedError, "This class is implemented natively in Rust"
    end

    # Checks if cancellation has been requested.
    #
    # This method can be called within message handlers to detect when the
    # handler should exit. Cancellation includes message-level cancellation
    # (e.g., handler timeout) and partition shutdown. During shutdown,
    # cancellation is delayed until near the end of the shutdown timeout to
    # allow in-flight work to complete.
    #
    # @return [Boolean] true if cancellation has been requested, false otherwise
    #
    # @example Checking for cancellation in a loop
    #   def on_message(context, message)
    #     items = message.payload["items"]
    #     items.each do |item|
    #       return if context.should_cancel?
    #       process_item(item)
    #     end
    #   end
    def should_cancel?
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns the demand this handler call serves.
    #
    # A normal delivery has kind +:normal+ and retry ordinal 0. A retry after a
    # failure has kind +:failure+ and an ordinal that is 1 on the first retry.
    # The ordinal is an estimate; see {Prosody::Demand}.
    #
    # @return [Prosody::Demand]
    #
    # @example Reading the retry ordinal
    #   def on_message(context, message)
    #     logger.warn("retry #{context.demand.retry}") if context.demand.failure?
    #   end
    def demand
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Blocks until cancellation is signaled.
    #
    # Cancellation includes message-level cancellation (e.g., handler timeout)
    # and partition shutdown. During shutdown, cancellation is delayed until near
    # the end of the shutdown timeout to allow in-flight work to complete.
    # This method is useful for long-running handlers that need to wait for
    # external events while remaining responsive to cancellation.
    #
    # @return [void]
    #
    # @example Waiting for cancellation
    #   def on_message(context, message)
    #     # Do some work, then wait for cancellation
    #     context.on_cancel
    #   end
    def on_cancel
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Schedules a timer to fire at the specified time.
    #
    # Timers allow you to delay execution or implement timeout behavior within
    # your message handlers. When a timer fires, your handler's #on_timer method
    # will be called with the timer object.
    #
    # @param time [Time] When the timer should fire
    # @return [void]
    # @raise [ArgumentError] If the time is invalid or outside the supported range (1970-2106)
    # @raise [RuntimeError] If timer scheduling fails
    #
    # @example Scheduling a delayed action
    #   def on_message(context, message)
    #     # Schedule a timer to fire in 30 seconds
    #     context.schedule(Time.now + 30)
    #   end
    #
    #   def on_timer(context, timer)
    #     puts "Timer fired for key: #{timer.key}"
    #   end
    def schedule(time)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Clears all scheduled timers and schedules a new one at the specified time.
    #
    # This is equivalent to calling clear_scheduled followed by schedule, but
    # performed atomically.
    #
    # @param time [Time] When the new timer should fire
    # @return [void]
    # @raise [ArgumentError] If the time is invalid or outside the supported range
    # @raise [RuntimeError] If timer operations fail
    #
    # @example Replacing all timers with a new one
    #   def on_message(context, message)
    #     # Clear any existing timers and schedule a new one
    #     context.clear_and_schedule(Time.now + 60)
    #   end
    def clear_and_schedule(time)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Unschedules a timer that was scheduled for the specified time.
    #
    # If multiple timers were scheduled for the same time, this will remove one
    # of them. If no timer exists for the specified time, this method does nothing.
    #
    # @param time [Time] The time for which to unschedule the timer
    # @return [void]
    # @raise [ArgumentError] If the time is invalid
    # @raise [RuntimeError] If timer unscheduling fails
    #
    # @example Canceling a specific timer
    #   def on_message(context, message)
    #     timer_time = Time.now + 30
    #     context.schedule(timer_time)
    #
    #     # Later, cancel that specific timer
    #     context.unschedule(timer_time)
    #   end
    def unschedule(time)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Clears all scheduled timers.
    #
    # After calling this method, no timers will be scheduled to fire for this
    # message context.
    #
    # @return [void]
    # @raise [RuntimeError] If clearing timers fails
    #
    # @example Canceling all timers
    #   def on_message(context, message)
    #     context.clear_scheduled
    #   end
    def clear_scheduled
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Returns all currently scheduled timer times.
    #
    # The returned array contains Time objects representing when each scheduled
    # timer will fire. The array may be empty if no timers are scheduled.
    #
    # @return [Array<Time>] Array of scheduled timer times
    # @raise [RuntimeError] If retrieving scheduled times fails
    #
    # @example Checking scheduled timers
    #   def on_message(context, message)
    #     scheduled_times = context.scheduled
    #     puts "#{scheduled_times.length} timers scheduled"
    #     scheduled_times.each do |time|
    #       puts "Timer will fire at: #{time}"
    #     end
    #   end
    def scheduled
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native single-value JSON state handle for the named collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeJsonValueState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def value_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native ordered-map JSON state handle for the named collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeJsonMapState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def map_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native set state handle for the named collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeSetState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def set_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native deque JSON state handle for the named collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeJsonDequeState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def deque_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native single-value message state handle for the named
    # collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeMessageValueState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def message_value_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native ordered-map message state handle for the named
    # collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeMessageMapState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def message_map_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    # Vends the native deque message state handle for the named collection.
    #
    # Internal routing target for {Prosody::State::Vending#state}; prefer
    # +context.state(definition)+.
    #
    # @param name [String] the registered collection name
    # @return [NativeMessageDequeState] the native handle
    # @raise [PermanentStateError] if the name is unregistered or mismatched
    # @private
    def message_deque_state(name)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end
end
