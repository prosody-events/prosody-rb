# frozen_string_literal: true

module Prosody
  # = Native Interface Stubs
  #
  # This file and the files in native_stubs/ contain stub definitions for
  # native methods implemented in the Prosody Rust extension. These stubs
  # provide documentation and method signatures for Ruby tooling like editors
  # and documentation generators, but the actual implementations are in the
  # Rust extension.
  #
  # == Implementation Notes
  #
  # The actual implementations of these methods are in the Rust extension at:
  # ext/prosody/src/

  # Wrapper for dynamically-typed results returned from async operations.
  # This is an internal class used by the native code to transfer results
  # between Rust and Ruby.
  #
  # @private
  class DynamicResult
    # @private
    def initialize
      raise NotImplementedError, "This class is implemented natively in Rust"
    end
  end

  # Internal processor for executing tasks asynchronously.
  # This class is used internally by the native code.
  #
  # @private
  class AsyncTaskProcessor
    # @private
    def initialize(logger = Prosody.logger)
      # Actual implementation is in lib/prosody/processor.rb
    end

    # @private
    def start
      # Actual implementation is in lib/prosody/processor.rb
    end

    # @private
    def stop
      # Actual implementation is in lib/prosody/processor.rb
    end

    # @private
    def submit(task_id, carrier, event_context, callback, &block)
      # Actual implementation is in lib/prosody/processor.rb
    end
  end
end

require_relative "native_stubs/context"
require_relative "native_stubs/message"
require_relative "native_stubs/client"
require_relative "native_stubs/state"
require_relative "native_stubs/published"
