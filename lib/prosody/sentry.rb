# frozen_string_literal: true

module Prosody
  module SentryIntegration
    def self.enabled?
      if ENV["SENTRY_DSN"] && !defined?(::Sentry)
        unless @warned_missing_gem
          @warned_missing_gem = true
          Prosody.logger.error("SENTRY_DSN is set but sentry-ruby is not installed. Add `gem 'sentry-ruby'` to your Gemfile.")
        end
        return false
      end

      defined?(::Sentry) && ::Sentry.initialized?
    end

    def self.capture_exception(exception, context = {})
      return unless enabled?

      ::Sentry.with_scope do |scope|
        scope.set_context("prosody", context)
        event_type = context[:event_type]
        scope.set_tag("prosody.event_type", event_type.to_s) if event_type
        ::Sentry.capture_exception(exception)
      end
    end
  end
end
