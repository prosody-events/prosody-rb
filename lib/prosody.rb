# frozen_string_literal: true

# The Prosody gem provides a Ruby interface for the Prosody event processing system.
# It implements a high-level client for working with Kafka message streams, with
# support for both producing and consuming messages in an idiomatic Ruby way.
#
# This library wraps a native Rust implementation for high performance while
# providing a comfortable Ruby API, with features including:
# - Configuration with Ruby-friendly syntax
# - Handler classes for processing messages
# - Async/non-blocking processing
# - OpenTelemetry integration for distributed tracing
# - Automatic error classification and retry logic

require "async"
require_relative "prosody/version"
require_relative "prosody/configuration"
require_relative "prosody/handler"
require_relative "prosody/processor"
require_relative "prosody/native_stubs" if defined?(Prosody::Client)

# Attempt to load the native extension specific to the current Ruby version first,
# falling back to the generic version if not available. This allows for optimized
# builds targeting specific Ruby versions.
begin
  ruby_version = /(\d+\.\d+)/.match(RUBY_VERSION)
  require_relative "prosody/#{ruby_version}/prosody"
rescue LoadError
  require_relative "prosody/prosody"
end
