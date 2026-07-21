# frozen_string_literal: true

require "securerandom"
require "timeout"

# Shared harness for the keyed-state specs.
#
# Reuses the same Queue + Timeout observation idiom as
# `client_spec.rb`'s `MessageStream`, giving the state specs a single home for
# the sink and the per-example topic/collection naming helpers. Auto-loaded by
# `spec_helper.rb`'s `support/*.rb` glob and mixed into every example group via
# `config.include KeyedStateSupport`.
module KeyedStateSupport
  # A thread-safe observation sink. Handlers push observation hashes and the
  # example drains them under `Timeout.timeout`, exactly like `MessageStream`.
  class StateSink
    def initialize
      @queue = Queue.new
    end

    # Push one observation.
    def push(observation)
      @queue.push(observation)
    end

    # Drain up to `count` observations, waiting up to `timeout` for each.
    #
    # @return [Array] the collected observations (may be fewer than `count`)
    def wait(count = 1, timeout = TestConfig::MESSAGE_TIMEOUT)
      collected = []
      count.times do
        collected << Timeout.timeout(timeout) { @queue.pop }
      rescue Timeout::Error
        break
      end
      collected
    end
  end

  # Builds a fresh observation sink.
  def new_sink
    StateSink.new
  end

  # A per-example nonce collection name. Every state test must use a fresh name
  # so a prior run's registered identity in the shared Cassandra keyspace/group
  # cannot collide with this one.
  def random_state_name(prefix = "col")
    "#{prefix}-#{SecureRandom.hex(6)}"
  end

  # A per-example nonce topic name.
  def random_state_topic
    "state-test-#{Time.now.to_i}-#{SecureRandom.hex(4)}"
  end

  # Builds a client configuration with the given state collection definitions
  # registered (and any extra options merged in), reusing `TestConfig`'s
  # bootstrap/cassandra/group settings.
  def state_config(topic, *definitions, **options)
    TestConfig.create_configuration(topic, {state_collections: definitions}.merge(options))
  end
end

# Shared setup for the keyed-state integration specs: a fresh per-example topic
# (4 partitions), an admin client, a sink, per-example client tracking, and
# `after` teardown. Mirrors client_spec.rb's topic lifecycle; kept in one place
# so the scenario/lifecycle/scan specs do not each re-declare it.
RSpec.shared_context "keyed state integration" do
  let(:topic) { random_state_topic }
  let(:sink) { new_sink }
  let(:admin_client_class) { Prosody.const_get(:AdminClient) }
  let(:admin) { admin_client_class.new([TestConfig::BOOTSTRAP_SERVERS]) }

  before do
    admin.create_topic(topic, 4, 1)
    sleep 1
  end

  after do
    @clients&.each do |client|
      client.unsubscribe if client.consumer_state == :running
    rescue => e
      puts "Could not unsubscribe client: #{e.message}"
    end
    ([topic] + (@extra_topics || [])).each do |name|
      admin.delete_topic(name)
    rescue => e
      puts "Could not delete topic #{name}: #{e.message}"
    end
  end

  # Builds and tracks a client (subscribed to `topic`) so `after` unsubscribes it.
  def build_client(*definitions, **options)
    client = Prosody::Client.new(state_config(topic, *definitions, **options))
    (@clients ||= []) << client
    client
  end

  # Tracks an externally-built client for `after` cleanup.
  def track_client(client)
    (@clients ||= []) << client
    client
  end

  # Creates and tracks an additional topic (4 partitions) for `after` cleanup.
  def create_extra_topic
    name = random_state_topic
    admin.create_topic(name, 4, 1)
    sleep 1
    (@extra_topics ||= []) << name
    name
  end
end
