# frozen_string_literal: true

module Prosody
  Success = Data.define(:value)
  Failure = Data.define(:error)
  HandlerError = Data.define(:message)

  Timeout = Data.define do
    def message = "no response arrived before the deadline"
  end

  FormatMismatch = Data.define do
    def message = "the responder answered in another format"
  end

  MalformedResponse = Data.define do
    def message = "the response did not decode"
  end

  class Client
    # Returns one outcome for each subsystem.
    # @param timeout [Numeric] The response deadline in seconds.
    # @raise [ArgumentError] if a subsystem name or timeout is invalid
    # @raise [RuntimeError] if the request cannot start or the Kafka send fails
    def request(topic:, key:, payload:, subsystems:, timeout:)
      native_request(
        topic: topic,
        key: key,
        payload: payload,
        subsystems: subsystems,
        timeout: timeout
      )
    end

    # Returns one excise outcome for each subsystem.
    def request_excise(topic:, key:, subsystems:, timeout:)
      native_request_excise(
        topic: topic,
        key: key,
        subsystems: subsystems,
        timeout: timeout
      )
    end
  end
end
