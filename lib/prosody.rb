# frozen_string_literal: true

require "async"
require_relative "prosody/version"
require_relative "prosody/configuration"
require_relative "prosody/handler"
require_relative "prosody/processor"

# Load the appropriate native extension
def load_prosody_extension
  ruby_version = RUBY_VERSION.match(/(\d+\.\d+)/)[1]
  extension_paths = [
    File.join(__dir__, "prosody", ruby_version, "prosody"),
    File.join(__dir__, "prosody", "prosody")
  ]

  extension_paths.each do |path|
    require path
    return true
  rescue LoadError
    next
  end

  # If we get here, no extension could be loaded
  available_dirs = Dir.glob(File.join(__dir__, "prosody", "*"))
    .select { |path| File.directory?(path) }
    .map { |dir| File.basename(dir) }

  extension_files = Dir.glob(File.join(__dir__, "prosody", "**", "prosody.*"))
    .map { |path| File.dirname(path).split(File::SEPARATOR).last + "/" + File.basename(path) }

  raise LoadError, "Could not load Prosody extension for Ruby #{RUBY_VERSION}. " \
    "Current platform: #{RUBY_PLATFORM}. " \
    "Available directories: #{available_dirs.inspect}. " \
    "Available extension files: #{extension_files.inspect}"
end

# Load the extension first, so it can define the Prosody module
load_prosody_extension

module Prosody
  class Error < StandardError; end
end
