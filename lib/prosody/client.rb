# frozen_string_literal: true

module Prosody
  # Ruby-side lifecycle helpers for the native {Prosody::Client}.
  class Client
    # Creates a client, yields it, and shuts it down when the block exits.
    # The client shuts down also when the block raises. Repeated shutdown
    # calls wait for the same operation, so the block can also call
    # {#shutdown}.
    #
    # @param config [Hash, Configuration] Client configuration
    # @yieldparam client [Client] the new client
    # @return [Object] the value of the block
    # @raise [ArgumentError] if no block is given or the configuration is invalid
    # @raise [RuntimeError] if client initialization or shutdown fails
    #
    # @example
    #   Prosody::Client.open(bootstrap_servers: "localhost:9092") do |client|
    #     client.send_message("my-topic", "key", {"hello" => "world"})
    #   end
    def self.open(config)
      raise ArgumentError, "Prosody::Client.open requires a block" unless block_given?

      client = new(config)
      begin
        yield client
      ensure
        client.shutdown
      end
    end
  end
end
