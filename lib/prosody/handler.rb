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
    # @param [Class<Exception>] exception_classes one or more Exception subclasses to catch
    # @return [Prosody::json_value] The wrapped method result
    # @raise [ArgumentError] if no exception classes given
    # @raise [NameError] if method_name is not defined on this class or its ancestors
    def permanent(method_name, *exception_classes)
      wrap_errors(method_name, exception_classes, PermanentError)
    end

    # Wraps the given instance method so that specified exception types
    # are caught and re-raised as Prosody::TransientError.
    #
    # @param [Symbol] method_name the name of the method to wrap
    # @param [Class<Exception>] exception_classes one or more Exception subclasses to catch
    # @return [Prosody::json_value] The wrapped method result
    # @raise [ArgumentError] if no exception classes given
    # @raise [NameError] if method_name is not defined on this class or its ancestors
    def transient(method_name, *exception_classes)
      wrap_errors(method_name, exception_classes, TransientError)
    end

    private

    # Core implementation: prepends a module that defines the same method name,
    # rescuing the specified exceptions and re-raising them as the given error_class.
    #
    # @param [Symbol] method_name the method to wrap
    # @param [Array<Class<Exception>>] exception_classes exceptions to catch
    # @param [Class<EventHandlerError>] error_class the error class to wrap caught exceptions in
    # @return [Prosody::json_value] the message response
    def wrap_errors(method_name, exception_classes, error_class)
      # Must specify at least one exception class
      if exception_classes.empty?
        raise ArgumentError, "At least one exception class must be provided"
      end

      # Ensure the method exists (in this class or its ancestors)
      unless method_defined?(method_name)
        raise NameError, "Method `#{method_name}` is not defined"
      end

      # Build a prepended wrapper module
      wrapper = Module.new do
        define_method(method_name) do |*args, &block|
          super(*args, &block)
        rescue *exception_classes => e
          # The new exception's #cause will be set automatically
          Kernel.raise error_class.new(e.message)
        end
      end
      wrapper.instance_variable_set(:@prosody_error_wrapper, true)

      prepend wrapper
    end
  end

  # --------------------------------------------------------------------------
  # 3) Base EventHandler that users will subclass
  # --------------------------------------------------------------------------

  # Abstract base class for handling incoming messages and timers from Prosody.
  # The RBS type parameter describes the JSON payload delivered in each
  # {Message}; it defaults to +Prosody::json_value+. Declare a narrower payload
  # shape in your application's RBS (for example,
  # +EventHandler[order_event]+) to type-check payload access.
  # Subclasses must implement `#on_message`, `#on_excise`, and `#on_timer`.
  # They may also use `permanent` or `transient` decorators to control retry logic.
  #
  # @example
  #   class MyHandler < Prosody::EventHandler
  #     extend Prosody::ErrorClassification
  #
  #     permanent :on_message, ArgumentError
  #     transient :on_message, RuntimeError
  #     permanent :on_timer, ArgumentError
  #     transient :on_timer, RuntimeError
  #
  #     def on_message(context, message)
  #       # Process message...
  #     end
  #
  #     def on_excise(context, message)
  #       # Process excise record...
  #     end
  #
  #     def on_timer(context, trigger)
  #       # Process timer event...
  #     end
  #   end
  class EventHandler
    extend ErrorClassification

    HANDLER_METHODS = %i[on_message on_excise on_timer].freeze

    def self.validate_handler!(handler)
      owners = handler.class.ancestors.take_while { |owner| owner != self }

      HANDLER_METHODS.each do |name|
        validate_method!(handler, owners, name)
      end
    end

    def self.validate_method!(handler, owners, name)
      implemented = owners.any? do |owner|
        !owner.instance_variable_get(:@prosody_error_wrapper) &&
          (owner.instance_methods(false) + owner.private_instance_methods(false)).include?(name)
      end
      raise ArgumentError, "handler must implement ##{name}" unless implemented
      method = handler.method(name)
      while method.owner.instance_variable_get(:@prosody_error_wrapper)
        method = method.super_method
        raise ArgumentError, "handler must implement ##{name}" unless method
      end
      return if accepts_two_parameters?(method)

      raise ArgumentError, "handler ##{name} must accept two parameters"
    end
    private_class_method :validate_method!

    def self.accepts_two_parameters?(method)
      parameters = method.parameters
      required = parameters.count { |kind, _| kind == :req }
      positional = parameters.count do |parameter|
        parameter.first == :req || parameter.first == :opt
      end
      has_rest = parameters.any? { |kind, _| kind == :rest }
      has_required_keyword = parameters.any? { |kind, _| kind == :keyreq }

      !has_required_keyword && required <= 2 && (has_rest || positional >= 2)
    end
    private_class_method :accepts_two_parameters?

    # Process a single message received from Prosody.
    # This method must be implemented by subclasses to define
    # custom message handling logic.
    #
    # @param [Context] context the message context
    # @param [Message<Payload>] message the message and its typed JSON payload
    # @raise [NotImplementedError] if not overridden by subclass
    # @return [Prosody::json_value] the message response
    def on_message(context, message)
      raise NotImplementedError, "Subclasses must implement #on_message"
    end

    # Process an excise record for a key.
    #
    # @param [Context] context the event context
    # @param [ExciseMessage] message the excise record metadata
    # @raise [NotImplementedError] if not overridden by a subclass
    # @return [Prosody::json_value] the excise response
    def on_excise(context, message)
      raise NotImplementedError, "Subclasses must implement #on_excise"
    end

    # Process a timer event when it fires.
    # This method must be implemented by subclasses to handle
    # scheduled timer events if they can fire.
    #
    # @param [Context] context the event context
    # @param [Timer] timer the timer containing key, time, and span
    # @return [void]
    def on_timer(context, timer)
      raise NotImplementedError, "Subclasses must implement #on_timer"
    end
  end
end
