# frozen_string_literal: true

module Prosody
  # --------------------------------------------------------------------------
  # 1) Base error classes with a `#permanent?` contract
  # --------------------------------------------------------------------------

  # Prosody::EventHandlerError
  #
  # Abstract base for all errors raised by EventHandler methods.
  # Subclasses **must** implement `#permanent?` to indicate retry behavior.
  #
  # @abstract
  class EventHandlerError < StandardError
    # Indicates whether this error is permanent (no retry) or
    # transient (retryable).
    #
    # @return [Boolean] `true` if permanent, `false` if transient
    # @raise  [NotImplementedError] if not overridden by subclass
    def permanent?
      raise NotImplementedError, "#{self.class} must implement #permanent?"
    end
  end

  # Prosody::TransientError
  #
  # Represents a retryable (transient) error.
  #
  # @see EventHandlerError#permanent?
  class TransientError < EventHandlerError
    # @return [false]
    def permanent? = false
  end

  # Prosody::PermanentError
  #
  # Represents a non-retryable (permanent) error.
  #
  # @see EventHandlerError#permanent?
  class PermanentError < EventHandlerError
    # @return [true]
    def permanent? = true
  end

  # --------------------------------------------------------------------------
  # 2) Mixin that provides “decorators” for wrapping methods
  # --------------------------------------------------------------------------

  # Prosody::ErrorClassification
  #
  # Mixin providing class-level methods to wrap instance methods so that
  # specified exceptions are re-wrapped as PermanentError or TransientError.
  #
  # @example
  #   class MyHandler < Prosody::EventHandler
  #     extend Prosody::ErrorClassification
  #
  #     # Treat TypeError as permanent (no retry)
  #     permanent :on_message, TypeError
  #
  #     # Treat JSON::ParserError as transient (retryable)
  #     transient :on_message, JSON::ParserError
  #
  #     def on_message(context, message)
  #       # …
  #     end
  #   end
  module ErrorClassification
    # Wraps the given instance method so that specified exception types
    # are caught and re-raised as Prosody::PermanentError.
    #
    # @param [Symbol] method_name       the name of the method to wrap
    # @param [Array<Class>] exception_classes one or more Exception subclasses
    # @return [void]
    # @raise  [ArgumentError] if no exception classes given or invalid
    # @raise  [NameError]     if method_name is not defined
    def permanent(method_name, *exception_classes)
      wrap_errors(method_name, exception_classes, PermanentError)
    end

    # Wraps the given instance method so that specified exception types
    # are caught and re-raised as Prosody::TransientError.
    #
    # @param [Symbol] method_name       the name of the method to wrap
    # @param [Array<Class>] exception_classes one or more Exception subclasses
    # @return [void]
    # @raise  [ArgumentError] if no exception classes given or invalid
    # @raise  [NameError]     if method_name is not defined
    def transient(method_name, *exception_classes)
      wrap_errors(method_name, exception_classes, TransientError)
    end

    private

    # Core implementation: redefine `method_name` to catch listed exceptions
    # and re-raise them as `error_class`.
    #
    # @param [Symbol] method_name
    # @param [Array<Class>] exception_classes
    # @param [Class<EventHandlerError>] error_class
    # @return [void]
    def wrap_errors(method_name, exception_classes, error_class)
      original = instance_method(method_name)

      define_method(method_name) do |*args, &block|
        begin
          original.bind(self).call(*args, &block)
        rescue *exception_classes => e
          # Ruby auto-assigns e as the new exception’s #cause
          raise error_class.new(e.message)
        end
      end
    end
  end

  # --------------------------------------------------------------------------
  # 3) Base EventHandler that users will subclass
  # --------------------------------------------------------------------------

  # Prosody::EventHandler
  #
  # Abstract base class for handling incoming messages.
  # Subclasses **must** implement `#on_message`.  They may call
  # `permanent :on_message, SomeError` or
  # `transient :on_message, OtherError` to control retry logic.
  #
  # @example
  #   class MyHandler < Prosody::EventHandler
  #     permanent  :on_message, TypeError
  #     transient  :on_message, JSON::ParserError
  #
  #     def on_message(context, message)
  #       # …process…
  #     end
  #   end
  class EventHandler
    extend ErrorClassification

    # Process a single message.
    #
    # @param [Context] context the current message context
    # @param [Message] message the incoming message payload
    # @raise  [NotImplementedError] if not overridden
    # @return [void]
    def on_message(context, message)
      raise NotImplementedError, "Subclasses must implement #on_message"
    end
  end
end
