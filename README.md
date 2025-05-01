# Prosody Ruby: Ruby Bindings for Kafka

Prosody Ruby offers Ruby bindings to the [Prosody Kafka client](https://github.com/cincpro/prosody), providing
features for message production and consumption, including configurable retry mechanisms, failure handling
strategies, and integrated OpenTelemetry support for distributed tracing.

## Features

- Rust-powered Kafka client for high performance
- Message production and consumption support
- Configurable modes: pipeline, low-latency, and best-effort
- OpenTelemetry integration for distributed tracing
- Efficient parallel processing with key-based ordering
- Intelligent partition pausing for backpressure management
- Mock Kafka broker support for testing
- Event type filtering for selectively processing messages
- Source system tracking to prevent message processing loops

## Installation

Installation is straightforward using Bundler:

```ruby
# In your Gemfile
source "https://gem.fury.io/realgeeks/" do
  gem "prosody"
end
```

Or install directly:

```bash
gem install prosody --source=https://gem.fury.io/realgeeks/
```

## Quick Start

```ruby
require "prosody"

# Initialize the client with Kafka bootstrap server, consumer group, and topics
client = Prosody::Client.new(
  # Bootstrap servers should normally be set using the PROSODY_BOOTSTRAP_SERVERS environment variable
  bootstrap_servers: "localhost:9092",

  # To allow loopbacks, the source_system must be different from the group_id.
  # Normally, the source_system would be left unspecified, which would default to the group_id.
  source_system: "my-application-source",

  # The group_id should be set to the name of your application
  group_id: "my-application",

  # Topics the client should subscribe to
  subscribed_topics: "my-topic"
)

# Define a custom message handler
class MyHandler < Prosody::EventHandler
  def on_message(context, message)
    # Process the received message
    puts "Received message: #{message.payload.inspect}"
    puts "From topic: #{message.topic}, partition: #{message.partition}, offset: #{message.offset}"
  end
end

# Subscribe to messages using the custom handler
client.subscribe(MyHandler.new)

# Send a message to a topic
client.send_message("my-topic", "message-key", { content: "Hello, Kafka!" })

# Ensure proper shutdown when done
client.unsubscribe
```

## Architecture

Prosody enables efficient, parallel processing of Kafka messages while maintaining order for messages with the same key:

- **Partition-Level Parallelism**: Separate management of each Kafka partition
- **Key-Based Queuing**: Ordered processing for each key within a partition
- **Concurrent Processing**: Simultaneous processing of different keys
- **Backpressure Management**: Pause consumption from backed-up partitions

## Configuration

The `Prosody::Client` constructor accepts these key parameters via a `Prosody::Configuration` object or a hash:

- `bootstrap_servers` (String | Array[String]): Kafka bootstrap servers (required)
- `group_id` (String): Consumer group ID (required for consumption)
- `subscribed_topics` (String | Array[String]): Topics to subscribe to (required for consumption)
- `source_system` (String): Identifier for the producing system to prevent loops (defaults to group_id)
- `allowed_events` (String | Array[String]): Prefixes of event types to process (processes all if unspecified)
- `mode` (String | Symbol): 'pipeline' (default), 'low_latency', or 'best_effort'

Additional optional parameters control behavior like message committal, polling intervals, and retry logic. Most
parameters can be set via environment variables (e.g., `PROSODY_BOOTSTRAP_SERVERS`).

Example with `Prosody::Configuration`:

```ruby
config = Prosody::Configuration.new do |c|
  c.bootstrap_servers = ["kafka1:9092", "kafka2:9092"]
  c.group_id = "my-consumer-group"
  c.subscribed_topics = ["topic1", "topic2"]
  c.max_concurrency = 10
  c.mode = :low_latency
end

client = Prosody::Client.new(config)
```

## Liveness and Readiness Probes

Prosody includes a built-in probe server for consumer-based applications that provides health check endpoints. The probe
server is tied to the consumer's lifecycle and offers two main endpoints:

1. `/readyz`: A readiness probe that checks if any partitions are assigned to the consumer. Returns a success status
   only when the consumer has at least one partition assigned, indicating it's ready to process messages.

2. `/livez`: A liveness probe that checks if any partitions have stalled (haven't processed a message within a
   configured time threshold).

Configure the probe server using either the client constructor:

```ruby
client = Prosody::Client.new(
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic",
  probe_port: 8000,  # Set to false to disable
  stall_threshold: 15.0  # Seconds before considering a partition stalled
)
```

Or via environment variables:

```bash
PROSODY_PROBE_PORT=8000  # Set to 'none' to disable
PROSODY_STALL_THRESHOLD=15s  # Default stall detection threshold
```

### Important Notes

1. The probe server starts automatically when the consumer is subscribed and stops when unsubscribed.
2. A partition is considered "stalled" if it hasn't processed a message within the `stall_threshold` duration.
3. The stall threshold should be set based on your application's message processing latency and expected message
   frequency.
4. Setting the threshold too low might cause false positives, while setting it too high could delay detection of actual
   issues.
5. The probe server is only active when consuming messages (not for producer-only usage).

## Advanced Usage

### Pipeline Mode

Pipeline mode is the default mode. Ensures ordered processing, retrying failed operations indefinitely:

```ruby
# Initialize client in pipeline mode
client = Prosody::Client.new(
  mode: :pipeline,  # Explicitly set pipeline mode (this is the default)
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic"
)
```

### Low-Latency Mode

Prioritizes quick processing, sending persistently failing messages to a failure topic:

```ruby
# Initialize client in low-latency mode
client = Prosody::Client.new(
  mode: :low_latency,  # Set low-latency mode
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic",
  failure_topic: "failed-messages"  # Specify a topic for failed messages
)
```

### Best-Effort Mode

Optimized for development environments or services where message processing failures are acceptable:

```ruby
# Initialize client in best-effort mode
client = Prosody::Client.new(
  mode: :best_effort,  # Set best-effort mode
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic"
)
```

## Event Type Filtering

Prosody supports filtering messages based on event type prefixes, allowing your consumer to process only specific types of events:

```ruby
# Process only events with types starting with "user." or "account."
client = Prosody::Client.new(
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic",
  allowed_events: ["user.", "account."]
)
```

Or via environment variables:

```bash
PROSODY_ALLOWED_EVENTS=user.,account.
```

### Matching Behavior

Prefixes must match exactly from the start of the event type:

✓ Matches:
- `{"type": "user.created"}` matches prefix `user.`
- `{"type": "account.deleted"}` matches prefix `account.`

✗ No Match:
- `{"type": "admin.user.created"}` doesn't match `user.`
- `{"type": "my.account.deleted"}` doesn't match `account.`
- `{"type": "notification"}` doesn't match any prefix

If no prefixes are configured, all messages are processed. Messages without a `type` field are always processed.

## Source System Deduplication

Prosody prevents processing loops in distributed systems by tracking the source of each message:

```ruby
# Consumer and producer in one application
client = Prosody::Client.new(
  group_id: "my-service",
  source_system: "my-service-producer",  # Must differ from group_id to allow loopbacks; defaults to group_id
  subscribed_topics: "my-topic"
)
```

Or via environment variable:

```bash
PROSODY_SOURCE_SYSTEM=my-service-producer
```

### How It Works

1. **Producers** add a `source-system` header to all outgoing messages.
2. **Consumers** check this header on incoming messages.
3. If a message's source system matches the consumer's group ID, the message is skipped.

This prevents endless loops where a service consumes its own produced messages.

## Message Deduplication

Prosody automatically deduplicates messages using the `id` field in their JSON payload. Consecutive messages with the
same ID and key are processed only once.

```ruby
# Messages with IDs are deduplicated per key
client.send_message("my-topic", "key1", {
  id: "msg-123",  # Message will be processed
  content: "Hello!"
})

client.send_message("my-topic", "key1", {
  id: "msg-123",  # Message will be skipped (duplicate)
  content: "Hello again!"
})

client.send_message("my-topic", "key2", {
  id: "msg-123",  # Message will be processed (different key)
  content: "Hello!"
})
```

Deduplication can be disabled by setting:

```ruby
client = Prosody::Client.new(
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic",
  idempotence_cache_size: 0  # Disable deduplication
)
```

Or via environment variable:

```bash
PROSODY_IDEMPOTENCE_CACHE_SIZE=0
```

Note that this deduplication is best-effort and not guaranteed. Because identifiers are cached ephemerally in memory,
duplicates can still occur when instances rebalance or restart.

## OpenTelemetry Tracing

Prosody supports OpenTelemetry tracing, allowing you to monitor and analyze the performance of your Kafka-based
applications. The library will automatically integrate with your application's OpenTelemetry setup.

### Required Gems

To use OpenTelemetry tracing with Prosody, you need to install the opentelemetry-api gem:

```ruby
# In your Gemfile
gem "opentelemetry-api", "~> 1.5"
```

### Initializing Tracing

To initialize tracing in your application:

```ruby
class MyHandler < Prosody::EventHandler
  def initialize
    # Create the tracer once and reuse it
    @tracer = OpenTelemetry.tracer_provider.tracer('my-service')
  end

  def on_message(context, message)
    @tracer.in_span("process-message") do |span|
      span.add_attributes({
        'message.key' => message.key,
        'message.topic' => message.topic
      })

      # Process the message
      puts "Processing message: #{message.payload}"
    end
  end
end
```

### Setting OpenTelemetry Environment Variables

Set the following standard OpenTelemetry environment variables:

```
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
OTEL_SERVICE_NAME=my-service-name
```

### Using Tracing in Your Application

After initializing tracing, traces will be automatically propagated through Kafka. You can also create your own spans in your message handlers:

```ruby
class MyHandler < Prosody::EventHandler
  def on_message(context, message)
    tracer = OpenTelemetry.tracer_provider.tracer('my-service')

    tracer.in_span("process-message") do |span|
      span.add_attributes({
        'message.key' => message.key,
        'message.topic' => message.topic
      })

      # Process the message
      puts "Processing message: #{message.payload}"
    end
  end
end
```

## Best Practices

### Error Handling

Prosody classifies errors as transient (temporary, can be retried) or permanent (won't be resolved by retrying). By default, all errors are considered transient.

The `Prosody::EventHandler` class provides two methods for error classification:

1. `permanent` - For errors that should not be retried
2. `transient` - For errors that can be retried

```ruby
class MyHandler < Prosody::EventHandler
  # Mark TypeErrors and NoMethodErrors as permanent (not retryable)
  permanent :on_message, TypeError, NoMethodError

  # Mark JSON::ParserError as transient (retryable)
  transient :on_message, JSON::ParserError

  def on_message(context, message)
    # Your message handling logic here
    # TypeError and NoMethodError will be treated as permanent
    # JSON::ParserError will be treated as transient
    # All other exceptions will be treated as transient (default behavior)
  end
end
```

You can also create custom error types that explicitly declare if they're permanent:

```ruby
# Define custom error types
class MyPermanentError < Prosody::PermanentError
  # permanent? method returns true
end

class MyTransientError < Prosody::TransientError
  # permanent? method returns false
end

class MyHandler < Prosody::EventHandler
  def on_message(context, message)
    # Later in your code, raise the appropriate error type
    if invalid_data?(message.payload)
      raise MyPermanentError, "Invalid data format"
    elsif temporary_issue?
      raise MyTransientError, "Temporary connection issue"
    end
  end
end
```

Best practices:

- Use permanent errors for issues like malformed data or business logic violations.
- Use transient errors for temporary issues like network problems.
- Be cautious with permanent errors as they prevent retries and can result in data loss.
- Consider system reliability and data consistency when classifying errors.

### Ensuring Idempotent Message Handlers

Idempotent message handlers are crucial for maintaining data consistency, fault tolerance, and scalability when working
with distributed, event-based systems. They ensure that processing a message multiple times has the same effect as
processing it once, which is essential for recovering from failures.

Strategies for achieving idempotence:

1. **Natural Idempotence**: Use inherently idempotent operations (e.g., setting a value in a key-value store).

2. **Deduplication with Unique Identifiers**:

- Kafka messages can be uniquely identified by their partition and offset.
- Before processing, check if the message has been handled before.
- Store processed message identifiers with an appropriate TTL.

3. **Database Upserts**: Use upsert operations for database writes (e.g., `INSERT ... ON CONFLICT DO UPDATE` in
   PostgreSQL).

4. **Partition Offset Tracking**:

- Store the latest processed offset for each partition.
- Only process messages with higher offsets than the last processed one.
- Critically, store these offsets transactionally with other state updates to ensure consistency.

5. **Idempotency Keys for External APIs**: Utilize idempotency keys when supported by external APIs.

6. **Check-then-Act Pattern**:

- For non-idempotent external systems, verify if an operation was previously completed before execution.
- Maintain a record of completed operations, keyed by a unique message identifier.

7. **Saga Pattern**:

- Implement a state machine in your database for multi-step operations.
- Each message advances the state machine, allowing for idempotent processing and easy failure recovery.
- Particularly useful for complex, distributed transactions across multiple services.

### Proper Shutdown

Always unsubscribe from topics before exiting your application:

```ruby
# Ensure proper shutdown
client.unsubscribe
```

This ensures:

1. Completion and commitment of all in-flight work
2. Quick rebalancing, allowing other consumers to take over partitions
3. Proper release of resources

Implement shutdown handling in your application:

```ruby
require "prosody"
require "signal_trap"

client = Prosody::Client.new(
  bootstrap_servers: "localhost:9092",
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic"
)

# Set up a shutdown queue
shutdown = Queue.new

# Configure signal handlers to trigger shutdown
Signal.trap("INT") { shutdown.push(nil) }
Signal.trap("TERM") { shutdown.push(nil) }

# Subscribe to messages
client.subscribe(MyHandler.new)

# Block until a signal is received
shutdown.pop # This blocks until something is pushed to the queue by a signal handler

# Clean shutdown
puts "Shutting down gracefully..."
client.unsubscribe
```

## API Reference

### Prosody::Client

The main client for interacting with Kafka:

- `initialize(config)`: Create a new client with the given configuration.
- `send_message(topic, key, payload)`: Send a message to a specified topic.
- `subscribe(handler)`: Subscribe to messages using the provided handler.
- `unsubscribe()`: Unsubscribe from messages and shut down the consumer.

### Prosody::Configuration

A flexible configuration class with typed parameters:

```ruby
config = Prosody::Configuration.new do |c|
  c.bootstrap_servers = "localhost:9092"
  c.group_id = "my-consumer-group"
  # more settings...
end
```

### Prosody::EventHandler

An abstract base class for user-defined handlers:

```ruby
class MyHandler < Prosody::EventHandler
  # Optional error classification
  permanent :on_message, TypeError
  transient :on_message, JSON::ParserError

  def on_message(context, message)
    # Implement your message handling logic here
  end
end
```

### Prosody::Message

Represents a Kafka message with the following methods:

- `topic`: The name of the topic (String)
- `partition`: The partition number (Integer)
- `offset`: The message offset within the partition (Integer)
- `timestamp`: The timestamp when the message was created (Time)
- `key`: The message key (String)
- `payload`: The deserialized message payload (Hash or Array)

### Prosody::Context

Represents the context of a Kafka message, providing metadata and control capabilities for message handling.

## Release Process

Prosody uses an automated release process managed by GitHub Actions. The workflow is defined in the `.github/workflows/release.yaml` file. The process includes:

1. **Release Please**: Creates or updates release PRs based on Conventional Commits in the main branch
2. **Cross-compilation**: Builds native gems for multiple platforms:
   - Linux (x86_64, aarch64)
   - Linux musl (x86_64, aarch64)
   - macOS (x86_64, arm64)
   - Windows (x64)
3. **Testing**: Validates the built gems on Linux platforms
4. **Publishing**: Publishes the gems to Gemfury

### Contributing to Releases

To contribute to a release:

1. Make your changes in a feature branch
2. Use [Conventional Commits](https://www.conventionalcommits.org/) syntax for your commit messages
3. Create a pull request to merge your changes into the `main` branch
4. Once your PR is approved and merged, Release Please will include your changes in the next release PR
