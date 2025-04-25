# frozen_string_literal: true

require "async"
require_relative "prosody/version"
require_relative "prosody/configuration"
require_relative "prosody/handler"
require_relative "prosody/processor"

# Tries to require the extension for the given Ruby version first
begin
  ruby_version = /(\d+\.\d+)/.match(RUBY_VERSION)
  require_relative "#{ruby_version}/prosody"
rescue LoadError
  require "prosody"
end

module Prosody
  class Error < StandardError; end
end
