# frozen_string_literal: true

require "async"
require 'async/rspec'
require "prosody"

# Load shared test configuration
Dir[File.join(__dir__, "support", "*.rb")].each { |file| require file }

RSpec.configure do |config|
  # Enable flags like --only-failures and --next-failure
  config.example_status_persistence_file_path = ".rspec_status"

  # Disable RSpec exposing methods globally on `Module` and `main`
  config.disable_monkey_patching!

  # Exclude tracing tests unless explicitly requested
  config.filter_run_excluding :tracing

  config.expect_with :rspec do |c|
    c.syntax = :expect
  end
end
