# frozen_string_literal: true

require "async"
require 'async/rspec'
require "prosody"

RSpec.configure do |config|
  # Enable flags like --only-failures and --next-failure
  config.example_status_persistence_file_path = ".rspec_status"

  # Disable RSpec exposing methods globally on `Module` and `main`
  config.disable_monkey_patching!

  config.expect_with :rspec do |c|
    c.syntax = :expect
  end
end

BOOTSTRAP_SERVERS = ENV.fetch("PROSODY_BOOTSTRAP_SERVERS", "localhost:9094") # Kafka connection string

# Set default Cassandra configuration for tests
ENV["PROSODY_CASSANDRA_NODES"] ||= "localhost:9042"
