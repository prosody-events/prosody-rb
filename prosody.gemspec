# frozen_string_literal: true

require_relative "lib/prosody/version"
require "rbconfig"

Gem::Specification.new do |spec|
  spec.name          = "prosody"
  spec.version       = Prosody::VERSION
  spec.authors       = ["Joshua Griffith"]
  spec.email         = ["Joshua.Griffith@fnf.com"]

  spec.summary       = "Ruby bindings for Kafka with configurable retry mechanisms and OpenTelemetry support"
  spec.description   = <<~DESC
    Prosody offers Ruby bindings to the Prosody Kafka client,
    providing features for message production and consumption,
    including configurable retry mechanisms, failure handling strategies,
    and integrated OpenTelemetry support for distributed tracing.
  DESC

  spec.homepage               = "https://github.com/cincpro/prosody-rb"
  spec.required_ruby_version  = ">= 3.1.0"
  spec.required_rubygems_version = ">= 3.3.11"

  spec.metadata["allowed_push_host"] = "https://rubygems.org"
  spec.metadata["homepage_uri"]      = spec.homepage
  spec.metadata["source_code_uri"]   = "#{spec.homepage}.git"
  spec.metadata["changelog_uri"]     = "#{spec.homepage}/blob/main/CHANGELOG.md"

  # Ensure your native extension is built at gem-build time
  spec.extensions = ["ext/prosody/extconf.rb"]

  # Collect all Git-tracked files plus the just-built shared library
  spec.files = Dir.chdir(__dir__) do
    files = `git ls-files -z`.split("\x0").reject do |f|
      f == File.basename(__FILE__) ||
        %w[
          bin/ test/ spec/ features/
          .git .github
          Gemfile Gemfile.lock
        ].any? { |pat| f.start_with?(pat) }
    end

    # Pick up the compiled extension (.so on Linux, .bundle on macOS)
    ext = RbConfig::CONFIG["DLEXT"]
    files + Dir.glob("lib/prosody/*.#{ext}")
  end

  spec.require_paths = ["lib"]
  spec.bindir       = "exe"
  spec.executables  = Dir.glob("exe/*").map { |f| File.basename(f) }

  spec.add_dependency "rb_sys", "~> 0.9.111"
  spec.add_dependency "async",  "~> 2.23"
  spec.add_development_dependency "async-rspec", "~> 1.17.0"
end
