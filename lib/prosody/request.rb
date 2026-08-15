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
    def request(topic:, key:, payload:, subsystems:, timeout:, headers: {})
      native_request(
        topic: topic,
        key: key,
        payload: payload,
        subsystems: subsystems,
        timeout: timeout,
        headers: headers
      )
    end
  end
end
