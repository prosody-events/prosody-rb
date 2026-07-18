target :lib do
  check "lib"
  # Steep cannot model Configuration's define_method DSL or the
  # StateDefinition Data.define body. Validate those public APIs separately.
  ignore "lib/prosody/configuration.rb"
  ignore "lib/prosody/state.rb"
  ignore "lib/prosody/native_stubs.rb"
  signature "sig"
  signature "sig-private"

  library "logger"
end

# Consumer-side regression coverage for the generic public API. This verifies
# that a handler payload type flows through Message#payload and message-backed
# keyed state, rather than only checking that the library signatures parse.
target :consumer_types do
  check "typecheck"
  signature "sig"
  signature "typecheck"

  library "logger"
end

target :typed_examples do
  # Check every runnable example, including examples added in the future.
  check "examples"
  signature "sig"
  signature "examples"

  library "logger"
end
