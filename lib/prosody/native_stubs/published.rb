# frozen_string_literal: true

# Native stubs for the published-state readers. The Ruby readers in
# lib/prosody/state wrap these classes.

module Prosody
  # @private
  class NativePublishedValue
    def get(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  # @private
  class NativePublishedMap
    def get(key, map_key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def get_many(key, map_keys)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def contains_key(key, map_key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def contains_many(key, map_keys)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def is_empty(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def scan(key, direction, query = {})
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def keys(key, direction, query = {})
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  # @private
  class NativePublishedSet
    def contains(key, member)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def contains_many(key, members)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def is_empty(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def keys(key, direction, query = {})
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end

  # @private
  class NativePublishedDeque
    def get(key, index)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def length(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def is_empty(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def peek_front(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def peek_back(key)
      raise NotImplementedError, "This method is implemented natively in Rust"
    end

    def scan(key, direction, query = {})
      raise NotImplementedError, "This method is implemented natively in Rust"
    end
  end
end
