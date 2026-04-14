# frozen_string_literal: true

require_relative "lib/prosody/version"

Gem::Specification.new do |spec|
  spec.name = "prosody"
  spec.version = Prosody::VERSION
  spec.authors = ["Joshua Griffith"]

  spec.summary = "Ruby bindings for the Prosody Kafka client library"
  spec.description = <<~DESC
    Prosody offers Ruby bindings to the Prosody Kafka client, providing features
    for message production and consumption, including configurable retry
    mechanisms, failure handling strategies, and integrated OpenTelemetry
    support for distributed tracing.
  DESC
  spec.homepage = "https://github.com/prosody-events/prosody-rb"

  spec.license = "MIT"

  spec.required_ruby_version = ">= 3.2.0"
  spec.required_rubygems_version = ">= 3.3.11"

  # Metadata for RubyGems
  spec.metadata["allowed_push_host"] = "https://rubygems.org"
  spec.metadata["homepage_uri"] = spec.homepage
  spec.metadata["source_code_uri"] = "https://github.com/prosody-events/prosody-rb"
  spec.metadata["changelog_uri"] = "https://github.com/prosody-events/prosody-rb/blob/main/CHANGELOG.md"

  # Specify which files should be included in the gem
  gemspec = File.basename(__FILE__)
  spec.files = IO.popen(%w[git ls-files -z], chdir: __dir__, err: IO::NULL) do |ls|
    ls.readlines("\x0", chomp: true).reject do |f|
      (f == gemspec) ||
        f.start_with?(*%w[bin/ test/ spec/ features/ .git .github appveyor Gemfile])
    end
  end

  spec.bindir = "exe"
  spec.executables = spec.files.grep(%r{\Aexe/}) { |f| File.basename(f) }
  spec.require_paths = ["lib"]
  spec.extensions = ["ext/prosody/extconf.rb"]

  # Runtime dependencies
  spec.add_dependency "async", "~> 2.39"
  spec.add_dependency "opentelemetry-api", "~> 1.9"
  spec.add_dependency "rb_sys", "~> 0.9.126"

  # Development dependencies
  spec.add_development_dependency "async-rspec", "~> 1.17"
  spec.add_development_dependency "opentelemetry-sdk", "~> 1.11"
  spec.add_development_dependency "opentelemetry-exporter-otlp", "~> 0.33"
  spec.add_development_dependency "sentry-ruby", "~> 6.5"
end
