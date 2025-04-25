# frozen_string_literal: true

require "async"
require_relative "prosody/version"
require_relative "prosody/configuration"
require_relative "prosody/handler"
require_relative "prosody/processor"
require "logger"

# Create a logger that writes to both STDOUT and a file
def setup_logger
  logger = Logger.new(STDOUT)
  logger.level = Logger::DEBUG

  # Also log to a file for persistence
  begin
    file_logger = Logger.new(File.join(Dir.pwd, "prosody_load.log"))
    file_logger.level = Logger::DEBUG

    # Return a combined logger
    logger.define_singleton_method(:info) do |msg|
      super(msg)
      file_logger.info(msg)
    end

    logger.define_singleton_method(:debug) do |msg|
      super(msg)
      file_logger.debug(msg)
    end

    logger.define_singleton_method(:error) do |msg|
      super(msg)
      file_logger.error(msg)
    end
  rescue => e
    logger.error("Failed to create file logger: #{e.message}")
  end

  logger
end

# Load the appropriate native extension with extensive logging
def load_prosody_extension
  logger = setup_logger

  # Log Ruby environment details
  logger.info("=" * 80)
  logger.info("ENVIRONMENT DETAILS:")
  logger.info("Ruby Version: #{RUBY_VERSION}")
  logger.info("Ruby Platform: #{RUBY_PLATFORM}")
  logger.info("Ruby Engine: #{RUBY_ENGINE}")
  logger.info("RbConfig::CONFIG['host_os']: #{RbConfig::CONFIG['host_os']}")
  logger.info("RbConfig::CONFIG['libdir']: #{RbConfig::CONFIG['libdir']}")
  logger.info("RbConfig::CONFIG['rubylibdir']: #{RbConfig::CONFIG['rubylibdir']}")
  logger.info("Current Directory: #{Dir.pwd}")
  logger.info("__FILE__: #{__FILE__}")
  logger.info("__dir__: #{__dir__}")
  logger.info("$LOAD_PATH: #{$LOAD_PATH.join(', ')}")

  # Extract Ruby version - log the exact match and result
  full_ruby_version = RUBY_VERSION
  version_match = RUBY_VERSION.match(/(\d+\.\d+)/)
  logger.info("RUBY_VERSION match result: #{version_match.inspect}")

  ruby_version = version_match ? version_match[1] : nil
  logger.info("Extracted Ruby version: #{ruby_version.inspect}")

  # Log the directory structure
  logger.info("=" * 80)
  logger.info("DIRECTORY STRUCTURE:")

  prosody_dir = File.join(__dir__, "prosody")
  logger.info("Base prosody directory: #{prosody_dir}")
  logger.info("Base prosody directory exists? #{File.directory?(prosody_dir)}")

  if File.directory?(prosody_dir)
    logger.info("Contents of prosody directory:")
    Dir.entries(prosody_dir).each do |entry|
      next if entry == "." || entry == ".."
      path = File.join(prosody_dir, entry)
      if File.directory?(path)
        logger.info("  [DIR] #{entry}")
        begin
          Dir.entries(path).each do |subentry|
            next if subentry == "." || subentry == ".."
            subpath = File.join(path, subentry)
            file_type = File.directory?(subpath) ? "[DIR]" : "[FILE]"
            file_info = ""
            if !File.directory?(subpath)
              file_size = File.size(subpath) rescue "unknown"
              file_readable = File.readable?(subpath) rescue "unknown"
              file_info = " (Size: #{file_size}, Readable: #{file_readable})"
            end
            logger.info("    #{file_type} #{subentry}#{file_info}")
          end
        rescue => e
          logger.error("    Error reading directory: #{e.message}")
        end
      else
        file_size = File.size(path) rescue "unknown"
        file_readable = File.readable?(path) rescue "unknown"
        logger.info("  [FILE] #{entry} (Size: #{file_size}, Readable: #{file_readable})")
      end
    end
  end

  # Try all possible extension files with specific extension types
  logger.info("=" * 80)
  logger.info("SEARCHING FOR EXTENSION FILES:")

  # Search for all prosody extension files with any extension
  all_extensions = Dir.glob(File.join(prosody_dir, "**", "prosody.*"))
  logger.info("All prosody.* files found:")
  all_extensions.each do |ext_file|
    rel_path = ext_file.sub(__dir__, "$DIR")
    file_size = File.size(ext_file) rescue "unknown"
    file_readable = File.readable?(ext_file) rescue "unknown"
    logger.info("  #{rel_path} (Size: #{file_size}, Readable: #{file_readable})")
  end

  # Try to load the extension
  logger.info("=" * 80)
  logger.info("ATTEMPTING TO LOAD EXTENSION:")

  extension_paths = []

  # First priority: version-specific extension
  if ruby_version
    version_specific_path = File.join(__dir__, "prosody", ruby_version, "prosody")
    extension_paths << version_specific_path
    logger.info("Adding version-specific path: #{version_specific_path}")
  end

  # Second priority: generic extension
  generic_path = File.join(__dir__, "prosody", "prosody")
  extension_paths << generic_path
  logger.info("Adding generic path: #{generic_path}")

  # Try each path
  extension_paths.each do |path|
    logger.info("Attempting to load: #{path}")
    begin
      require path
      logger.info("Successfully loaded extension from: #{path}")
      return true
    rescue LoadError => e
      logger.info("Failed to load extension from #{path}: #{e.message}")
      next
    rescue => e
      logger.error("Unexpected error loading extension from #{path}: #{e.class.name} - #{e.message}")
      logger.error(e.backtrace.join("\n")) if e.respond_to?(:backtrace) && e.backtrace
      next
    end
  end

  # If we get here, no extension could be loaded
  logger.info("=" * 80)
  logger.info("EXTENSION LOADING FAILED:")

  available_dirs = Dir.glob(File.join(__dir__, "prosody", "*"))
    .select { |path| File.directory?(path) }
    .map { |dir| File.basename(dir) }

  extension_files = Dir.glob(File.join(__dir__, "prosody", "**", "prosody.*"))
    .map { |path| File.dirname(path).split(File::SEPARATOR).last + "/" + File.basename(path) }

  error_message = "Could not load Prosody extension for Ruby #{RUBY_VERSION}. " \
    "Current platform: #{RUBY_PLATFORM}. " \
    "Available directories: #{available_dirs.inspect}. " \
    "Available extension files: #{extension_files.inspect}"

  logger.error(error_message)
  raise LoadError, error_message
end

# Load the extension first, so it can define the Prosody module
begin
  load_prosody_extension
rescue => e
  # Log but still raise the error
  puts "Failed to load Prosody extension: #{e.message}"
  raise
end

module Prosody
  class Error < StandardError; end
end
