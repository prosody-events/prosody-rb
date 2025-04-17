module Prosody
  # Abstract base class for event handlers
  class EventHandler
    # Handle a Kafka message
    # @param context [Context] The context of the message
    # @param message [Message] The Kafka message to be processed
    # @raise [NotImplementedError] This method must be implemented by subclasses
    def on_message(context, message)
      raise NotImplementedError, "Subclasses must implement #on_message"
    end
  end
end
