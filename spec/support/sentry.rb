# frozen_string_literal: true

if ENV["SENTRY_DSN"]
  require "sentry-ruby"

  Sentry.init do |config|
    config.dsn = ENV["SENTRY_DSN"]
  end
end
