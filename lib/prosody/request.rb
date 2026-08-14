# frozen_string_literal: true

module Prosody
  class ResponseError < Error; end

  class HandlerResponseError < ResponseError
    attr_reader :category, :handler_message

    def initialize(category:, handler_message:, message:)
      @category = category
      @handler_message = handler_message
      super(message)
    end
  end

  class ResponseTimeoutError < ResponseError; end
  class ResponseFormatMismatchError < ResponseError; end
  class MalformedResponseError < ResponseError; end

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
