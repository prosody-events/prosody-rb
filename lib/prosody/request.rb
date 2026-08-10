# frozen_string_literal: true

module Prosody
  Ok = Struct.new(:value)
  Err = Struct.new(:error)
  HandlerResponseError = Struct.new(:category, :message)
  ResponseTimeoutError = Class.new
  ResponseFormatMismatchError = Class.new
  MalformedResponseError = Class.new

  class Client
    def request(topic, key, payload, subsystems, timeout, headers: {})
      native_request(
        topic: topic,
        key: key,
        payload: payload,
        subsystems: subsystems,
        timeout: timeout.to_f,
        headers: headers
      )
    end
  end
end
