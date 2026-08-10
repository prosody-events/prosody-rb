# frozen_string_literal: true

module Prosody
  Ok = Data.define(:value)
  Err = Data.define(:error)
  HandlerResponseError = Data.define(:category, :message)
  ResponseTimeoutError = Data.define
  ResponseFormatMismatchError = Data.define
  MalformedResponseError = Data.define

  class Client
    def request(topic, key, payload, subsystems, timeout, headers: {})
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
