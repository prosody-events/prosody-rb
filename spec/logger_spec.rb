# frozen_string_literal: true

require "prosody"

RSpec.describe "Prosody.logger" do
  around do |example|
    original = Prosody.instance_variable_get(:@logger)
    Prosody.instance_variable_set(:@logger, nil)
    example.run
  ensure
    Prosody.instance_variable_set(:@logger, original)
  end

  describe ".logger" do
    it "returns a Logger instance" do
      expect(Prosody.logger).to be_a(Logger)
    end

    it "writes to $stdout" do
      logger = Prosody.logger
      dev = logger.instance_variable_get(:@logdev)
      expect(dev.dev).to eq($stdout)
    end

    it "defaults to INFO level" do
      expect(Prosody.logger.level).to eq(Logger::INFO)
    end

    it "memoizes the logger" do
      expect(Prosody.logger).to be(Prosody.logger)
    end
  end

  describe ".logger=" do
    it "assigns a custom logger" do
      custom = Logger.new($stderr)
      Prosody.logger = custom
      expect(Prosody.logger).to be(custom)
    end

    it "re-creates the default when set to nil" do
      Prosody.logger = Logger.new($stderr)
      Prosody.logger = nil
      expect(Prosody.logger).to be_a(Logger)
      expect(Prosody.logger.level).to eq(Logger::INFO)
    end
  end
end
