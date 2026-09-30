# frozen_string_literal: true

# = Native Interface Stubs
#
# The files in native_stubs/ document the public classes that the Prosody Rust
# extension implements. Editors and documentation tools read them. The runtime
# does not load them, because the native extension defines these classes.
#
# The implementations are in the Rust extension at ext/prosody/src/.

require_relative "native_stubs/context"
require_relative "native_stubs/message"
require_relative "native_stubs/client"
