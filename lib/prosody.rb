# frozen_string_literal: true

require "async"
require_relative "prosody/version"
require_relative "prosody/configuration"
require_relative "prosody/handler"
require_relative "prosody/processor"

# Tries to require the extension for the given Ruby version first
begin
  RUBY_VERSION =~ /(\d+\.\d+)/
  require "prosody/#{Regexp.last_match(1)}/prosody"
rescue LoadError
  require "prosody/prosody"
end

module Prosody
  class Error < StandardError; end
end
