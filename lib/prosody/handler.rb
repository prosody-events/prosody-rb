# frozen_string_literal: true

module Prosody
  # Base error class for all Prosody-specific exceptions.
  #
  # Specific error types may extend this class to provide more detailed
  # error information and classification.
  class Error < StandardError; end

  # --------------------------------------------------------------------------
  # 1) Base error classes with a `#permanent?` contract
  # --------------------------------------------------------------------------

  # Abstract base for all errors raised by EventHandler methods.
  # Subclasses **must** implement `#permanent?` to indicate retry behavior.
  #
  # @abstract
  class EventHandlerError < Error
    # Indicates whether this error is permanent (no retry) or
    # transient (retryable).
    #
    # @return [Boolean] true if permanent (no retry), false if transient (retryable)
    # @raise [NotImplementedError] if not overridden by subclass
    def permanent?
      raise NotImplementedError, "#{self.class} must implement #permanent?"
    end
  end

  # Error indicating that the failure is temporary and can be retried.
  #
  # @see EventHandlerError#permanent?
  class TransientError < EventHandlerError
    # @return [false] indicates this error is retryable
    def permanent? = false
  end

  # Error indicating that the failure is permanent and should not be retried.
  #
  # @see EventHandlerError#permanent?
  class PermanentError < EventHandlerError
    # @return [true] indicates this error is not retryable
    def permanent? = true
  end

  # --------------------------------------------------------------------------
  # 2) Mixin that provides "decorators" for wrapping methods
  # --------------------------------------------------------------------------

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
  #       # Process the message...
  #     end
  #   end
  module ErrorClassification
    # Wraps the given instance method so that specified exception types
    # are caught and re-raised as Prosody::PermanentError.
    #
    # @param [Symbol] method_name the name of the method to wrap
    # @param [Array<Class>] exception_classes one or more Exception subclasses to catch
    # @return [void]
    # @raise [ArgumentError] if no exception classes given or invalid
    # @raise [NameError] if method_name is not defined
    def permanent(method_name, *exception_classes)
      wrap_errors(method_name, exception_classes, PermanentError)
    end

    # Wraps the given instance method so that specified exception types
    # are caught and re-raised as Prosody::TransientError.
    #
    # @param [Symbol] method_name the name of the method to wrap
    # @param [Array<Class>] exception_classes one or more Exception subclasses to catch
    # @return [void]
    # @raise [ArgumentError] if no exception classes given or invalid
    # @raise [NameError] if method_name is not defined
    def transient(method_name, *exception_classes)
      wrap_errors(method_name, exception_classes, TransientError)
    end

    private

    # Core implementation: redefines `method_name` to catch listed exceptions
    # and re-raise them as the specified error class.
    #
    # @param [Symbol] method_name the method to wrap
    # @param [Array<Class>] exception_classes exceptions to catch
    # @param [Class<EventHandlerError>] error_class the error class to wrap caught exceptions in
    # @return [void]
    def wrap_errors(method_name, exception_classes, error_class)
      original = instance_method(method_name)

      define_method(method_name) do |*args, &block|
        original.bind_call(self, *args, &block)
      rescue *exception_classes => e
        # Ruby auto-assigns e as the new exception's #cause
        raise error_class.new(e.message)
      end
    end
  end

  # --------------------------------------------------------------------------
  # 3) Base EventHandler that users will subclass
  # --------------------------------------------------------------------------

  # Abstract base class for handling incoming messages from Prosody.
  # Subclasses **must** implement `#on_message` to process received messages.
  # They may also use `permanent` or `transient` decorators to control retry logic
  # for specific exceptions.
  #
  # @example
  #   class MyHandler < Prosody::EventHandler
  #     permanent :on_message, TypeError
  #     transient :on_message, JSON::ParserError
  #
  #     def on_message(context, message)
  #       # Process message content...
  #     end
  #   end
  class EventHandler
    extend ErrorClassification

    # Process a single message received from Prosody.
    # This method must be implemented by subclasses to define
    # custom message handling logic.
    #
    # @param [Context] context the message context containing metadata and control capabilities
    # @param [Message] message the message content with payload and metadata
    # @raise [NotImplementedError] if not overridden by subclass
    # @return [void]
    def on_message(context, message)
      raise NotImplementedError, "Subclasses must implement #on_message"
    end
  end
end
