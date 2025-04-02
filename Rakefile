# frozen_string_literal: true

require "bundler/gem_tasks"
require "rspec/core/rake_task"

RSpec::Core::RakeTask.new(:spec)

require "standard/rake"

require "rb_sys/extensiontask"

task build: :compile

GEMSPEC = Gem::Specification.load("prosody.gemspec")

RbSys::ExtensionTask.new("prosody", GEMSPEC) do |ext|
  ext.lib_dir = "lib/prosody"
end

task default: %i[compile spec standard]
