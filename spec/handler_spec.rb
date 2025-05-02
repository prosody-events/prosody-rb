# frozen_string_literal: true

require "spec_helper"

RSpec.describe Prosody::Error do
  it "inherits from StandardError" do
    expect(described_class.superclass).to eq(StandardError)
  end
end

RSpec.describe Prosody::EventHandlerError do
  it "inherits from Prosody::Error" do
    expect(described_class.superclass).to eq(Prosody::Error)
  end

  it "requires #permanent? to be implemented" do
    error = described_class.new("Test error")
    expect { error.permanent? }.to raise_error(NotImplementedError, /must implement #permanent\?/)
  end
end

RSpec.describe Prosody::TransientError do
  it "inherits from EventHandlerError" do
    expect(described_class.superclass).to eq(Prosody::EventHandlerError)
  end

  it "implements #permanent? as false" do
    error = described_class.new("Test error")
    expect(error.permanent?).to be(false)
  end
end

RSpec.describe Prosody::PermanentError do
  it "inherits from EventHandlerError" do
    expect(described_class.superclass).to eq(Prosody::EventHandlerError)
  end

  it "implements #permanent? as true" do
    error = described_class.new("Test error")
    expect(error.permanent?).to be(true)
  end
end

RSpec.describe Prosody::ErrorClassification do
  before(:all) do
    # Define test exception classes
    Object.const_set(:TestError1, Class.new(StandardError))
    Object.const_set(:TestError2, Class.new(StandardError))
  end

  after(:all) do
    # Clean up test exception classes
    Object.send(:remove_const, "TestError1")
    Object.send(:remove_const, "TestError2")
  end

  let(:test_class) do
    Class.new do
      extend Prosody::ErrorClassification

      # Define methods that will be wrapped
      def method_with_permanent_error
        raise TestError1, "A permanent error occurred"
      end

      def method_with_transient_error
        raise TestError2, "A transient error occurred"
      end

      def method_with_multiple_errors
        raise TestError1, "Could be either type"
      end

      # Apply decorators
      permanent :method_with_permanent_error, TestError1
      transient :method_with_transient_error, TestError2
    end
  end

  let(:instance) { test_class.new }

  it "wraps exceptions as PermanentError with the permanent decorator" do
    expect { instance.method_with_permanent_error }.to raise_error(Prosody::PermanentError)
  end

  it "wraps exceptions as TransientError with the transient decorator" do
    expect { instance.method_with_transient_error }.to raise_error(Prosody::TransientError)
  end

  it "preserves the original error message" do
    begin
      instance.method_with_permanent_error
    rescue Prosody::PermanentError => e
      expect(e.message).to eq("A permanent error occurred")
    end

    begin
      instance.method_with_transient_error
    rescue Prosody::TransientError => e
      expect(e.message).to eq("A transient error occurred")
    end
  end

  it "sets the original exception as the cause" do
    begin
      instance.method_with_permanent_error
    rescue Prosody::PermanentError => e
      expect(e.cause).to be_a(TestError1)
    end

    begin
      instance.method_with_transient_error
    rescue Prosody::TransientError => e
      expect(e.cause).to be_a(TestError2)
    end
  end

  context "with multiple exception types" do
    let(:multi_error_class) do
      Class.new do
        extend Prosody::ErrorClassification

        def method_with_errors
          yield
        end

        # Decorate with multiple exception types
        permanent :method_with_errors, TestError1, TestError2
      end
    end

    let(:multi_instance) { multi_error_class.new }

    it "handles all specified exception types" do
      expect {
        multi_instance.method_with_errors { raise TestError1, "Error 1" }
      }.to raise_error(Prosody::PermanentError)

      expect {
        multi_instance.method_with_errors { raise TestError2, "Error 2" }
      }.to raise_error(Prosody::PermanentError)
    end

    it "doesn't catch unspecified exceptions" do
      expect {
        multi_instance.method_with_errors { raise "Not wrapped" }
      }.to raise_error(RuntimeError)
    end
  end

  context "when decorating non-existent methods" do
    it "raises NameError" do
      expect {
        Class.new do
          extend Prosody::ErrorClassification
          permanent :nonexistent_method, StandardError
        end
      }.to raise_error(NameError)
    end
  end

  context "when decorating with no exception classes" do
    it "raises ArgumentError" do
      expect {
        Class.new do
          extend Prosody::ErrorClassification

          def some_method
          end

          permanent :some_method
        end
      }.to raise_error(ArgumentError)
    end
  end
end

RSpec.describe Prosody::EventHandler do
  it "extends ErrorClassification" do
    expect(described_class.singleton_class.included_modules).to include(Prosody::ErrorClassification)
  end

  it "requires #on_message to be implemented" do
    handler = described_class.new
    expect {
      handler.on_message(nil, nil)
    }.to raise_error(NotImplementedError, /must implement #on_message/)
  end

  context "with a concrete implementation" do
    let(:test_handler_class) do
      Class.new(Prosody::EventHandler) do
        attr_reader :message_received

        permanent :on_message, ArgumentError
        transient :on_message, RuntimeError

        def on_message(context, message)
          @message_received = message

          # Error handling cases
          if message == "cause_argument_error"
            raise ArgumentError, "Bad argument"
          elsif message == "cause_runtime_error"
            raise "Runtime issue"
          end
        end
      end
    end

    let(:handler) { test_handler_class.new }
    let(:context) { double("Context") }

    it "processes messages correctly" do
      handler.on_message(context, "test message")
      expect(handler.message_received).to eq("test message")
    end

    it "wraps ArgumentError as PermanentError" do
      expect {
        handler.on_message(context, "cause_argument_error")
      }.to raise_error(Prosody::PermanentError)
    end

    it "wraps RuntimeError as TransientError" do
      expect {
        handler.on_message(context, "cause_runtime_error")
      }.to raise_error(Prosody::TransientError)
    end
  end
end
