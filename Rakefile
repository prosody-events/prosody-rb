# frozen_string_literal: true

require "bundler/gem_tasks"
require "rb_sys/cargo/metadata"
require "rb_sys/extensiontask"

task build: :compile

GEMSPEC = Gem::Specification.load("prosody.gemspec")

begin
  RbSys::ExtensionTask.new("prosody", GEMSPEC) do |ext|
    ext.lib_dir = "lib/prosody"
  end
rescue RbSys::CargoMetadataError
  # This is expected for the source gem, which can't be installed directly
  warn "Source gem cannot be installed directly, must be a supported platform"
end

require "rspec/core/rake_task"

RSpec::Core::RakeTask.new(:spec)

require "standard/rake"

task default: %i[compile spec standard]
