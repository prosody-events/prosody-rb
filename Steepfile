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
