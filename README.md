# Prosody: Ruby Bindings for Kafka

Prosody offers Ruby bindings to the [Prosody Kafka client](https://github.com/cincpro/prosody), providing
features for message production and consumption, including configurable retry mechanisms, failure handling
strategies, and integrated OpenTelemetry support for distributed tracing.

## Features

- **Kafka Consumer**: Per-key ordering with cross-key concurrency, offset management, consumer groups
- **Kafka Producer**: Idempotent delivery with configurable retries
- **Timer System**: Persistent scheduled execution backed by Cassandra or in-memory store
- **Quality of Service**: Fair scheduling limits concurrency and prevents failures from starving fresh traffic. Pipeline mode adds deferred retry and monopolization detection
- **Distributed Tracing**: OpenTelemetry integration for tracing message flow across services
- **Backpressure**: Pauses partitions when handlers fall behind
- **Mocking**: In-memory Kafka broker for tests (`mock: true`)
- **Failure Handling**: Pipeline (retry forever), Low-Latency (dead letter), Best-Effort (log and skip)

## Installation

Add this line to your application's Gemfile:

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

    # Schedule a timer for delayed processing (requires Cassandra unless mock: true)
    if message.payload["schedule_followup"]
      future_time = Time.now + 30 # 30 seconds from now
      context.schedule(future_time)
    end
  end

  def on_timer(context, timer)
    # Handle timer firing
    puts "Timer fired for key: #{timer.key} at #{timer.time}"
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

## Quality of Service

All modes use **fair scheduling** to limit concurrency and distribute execution time. Pipeline mode adds **deferred
retry** and **monopolization detection**.

### Fair Scheduling (All Modes)

The scheduler controls which message runs next and how many run concurrently.

**Virtual Time (VT):** Each key accumulates VT equal to its handler execution time. The scheduler picks the key with the
lowest VT. A key that runs for 500ms accumulates 500ms of VT; a key that hasn't run recently has zero VT and gets
priority.

**Two-Class Split:** Normal messages and failure retries have separate VT pools. The scheduler allocates execution time
between them (default: 70% normal, 30% failure). During a failure spike, retries get at most 30% of execution time—fresh
messages continue processing.

**Starvation Prevention:** Tasks receive a quadratic priority boost based on wait time. A task waiting 2 minutes
(configurable) gets maximum boost, overriding VT disadvantage.

### Deferred Retry (Pipeline Mode)

Moves failing keys to timer-based retry so the partition can continue processing other keys.

On transient failure: store the message offset in Cassandra, schedule a timer, return success. The partition advances.
When the timer fires, reload the message from Kafka and retry.

```ruby
# Configure defer behavior
client = Prosody::Client.new(
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic",
  defer_enabled: true,           # Enable deferral (default: true)
  defer_base: 1.0,               # Wait 1s before first retry
  defer_max_delay: 86400.0,      # Cap at 24 hours
  defer_failure_threshold: 0.9   # Disable when >90% failing
)
```

**Failure Rate Gating:** When >90% of recent messages fail, deferral disables. The retry middleware blocks the
partition, applying backpressure upstream.

### Monopolization Detection (Pipeline Mode)

Rejects keys that consume too much execution time.

The middleware tracks per-key execution time in 5-minute rolling windows. Keys exceeding 90% of window time are rejected
with a transient error, routing them through defer.

```ruby
# Configure monopolization detection
client = Prosody::Client.new(
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic",
  monopolization_enabled: true,     # Enable detection (default: true)
  monopolization_threshold: 0.9,    # Reject keys using >90% of window
  monopolization_window: 300.0      # 5-minute window
)
```

### Handler Timeout

Handlers are automatically cancelled if they exceed a deadline:

```ruby
client = Prosody::Client.new(
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic",
  timeout: 30.0,             # Cancel after 30 seconds
  stall_threshold: 60.0      # Report unhealthy after 60 seconds
)
```

When a handler times out, `context.should_cancel?` returns `true`. The handler should exit promptly. If not specified,
timeout defaults to 80% of `stall_threshold`.

## Configuration

Configure via constructor options or environment variables. Options fall back to environment variables when unset.

### Core

| Option / Environment Variable           | Description                                       | Default      |
|-----------------------------------------|---------------------------------------------------|--------------|
| `bootstrap_servers` / `PROSODY_BOOTSTRAP_SERVERS` | Kafka servers to connect to             | -            |
| `group_id` / `PROSODY_GROUP_ID`         | Consumer group name                               | -            |
| `subscribed_topics` / `PROSODY_SUBSCRIBED_TOPICS` | Topics to read from                     | -            |
| `allowed_events` / `PROSODY_ALLOWED_EVENTS` | Only process events matching these prefixes   | (all)        |
| `source_system` / `PROSODY_SOURCE_SYSTEM` | Tag for outgoing messages (prevents reprocessing)| `<group_id>` |
| `mock` / `PROSODY_MOCK`                 | Use in-memory Kafka for testing                   | false        |

### Consumer

| Option / Environment Variable           | Description                                          | Default                |
|-----------------------------------------|------------------------------------------------------|------------------------|
| `max_concurrency` / `PROSODY_MAX_CONCURRENCY` | Max messages being processed simultaneously     | 32                     |
| `max_uncommitted` / `PROSODY_MAX_UNCOMMITTED` | Max queued messages before pausing consumption  | 64                     |
| `timeout` / `PROSODY_TIMEOUT`           | Cancel handler if it runs longer than this           | 80% of stall threshold |
| `commit_interval` / `PROSODY_COMMIT_INTERVAL` | How often to save progress to Kafka            | 1s                     |
| `poll_interval` / `PROSODY_POLL_INTERVAL` | How often to fetch new messages from Kafka         | 100ms                  |
| `shutdown_timeout` / `PROSODY_SHUTDOWN_TIMEOUT` | Shutdown budget; handlers complete freely before cancellation fires near the deadline | 30s |
| `stall_threshold` / `PROSODY_STALL_THRESHOLD` | Report unhealthy if no progress for this long  | 5m                     |
| `probe_port` / `PROSODY_PROBE_PORT`     | HTTP port for health checks (nil to disable)         | 8000                   |
| `failure_topic` / `PROSODY_FAILURE_TOPIC` | Send unprocessable messages here (dead letter queue) | -                     |
| `idempotence_cache_size` / `PROSODY_IDEMPOTENCE_CACHE_SIZE` | Global shared cache capacity across all partitions for message deduplication (0 disables the entire deduplication middleware, both in-memory and persistent) | 8192 |
| `idempotence_version` / `PROSODY_IDEMPOTENCE_VERSION` | Version string for cache-busting dedup hashes | 1              |
| `idempotence_ttl` / `PROSODY_IDEMPOTENCE_TTL`         | TTL for dedup records in Cassandra            | 7d (604800 seconds) |
| `slab_size` / `PROSODY_SLAB_SIZE`       | Timer storage granularity (rarely needs changing)    | 1h                     |

### Producer

| Option / Environment Variable           | Description                     | Default |
|-----------------------------------------|---------------------------------|---------|
| `send_timeout` / `PROSODY_SEND_TIMEOUT` | Give up sending after this long | 1s      |

### Retry

When a handler fails, retry with exponential backoff:

| Option / Environment Variable           | Description                       | Default |
|-----------------------------------------|-----------------------------------|---------|
| `max_retries` / `PROSODY_MAX_RETRIES`   | Give up after this many attempts  | 3       |
| `retry_base` / `PROSODY_RETRY_BASE`     | Wait this long before first retry | 20ms    |
| `max_retry_delay` / `PROSODY_RETRY_MAX_DELAY` | Never wait longer than this  | 5m      |

### Deferral (Pipeline Mode)

| Option / Environment Variable           | Description                                       | Default |
|-----------------------------------------|---------------------------------------------------|---------|
| `defer_enabled` / `PROSODY_DEFER_ENABLED` | Enable deferral for new messages                | true    |
| `defer_base` / `PROSODY_DEFER_BASE`     | Wait this long before first deferred retry        | 1s      |
| `defer_max_delay` / `PROSODY_DEFER_MAX_DELAY` | Never wait longer than this                 | 24h     |
| `defer_failure_threshold` / `PROSODY_DEFER_FAILURE_THRESHOLD` | Disable deferral when failure rate exceeds this | 0.9 |
| `defer_failure_window` / `PROSODY_DEFER_FAILURE_WINDOW` | Measure failure rate over this time window | 5m     |
| `defer_cache_size` / `PROSODY_DEFER_CACHE_SIZE` | Track this many deferred keys in memory     | 1024    |
| `defer_seek_timeout` / `PROSODY_DEFER_SEEK_TIMEOUT` | Timeout when loading deferred messages    | 30s     |
| `defer_discard_threshold` / `PROSODY_DEFER_DISCARD_THRESHOLD` | Read optimization (rarely needs changing) | 100  |

### Monopolization Detection (Pipeline Mode)

| Option / Environment Variable           | Description                             | Default |
|-----------------------------------------|-----------------------------------------|---------|
| `monopolization_enabled` / `PROSODY_MONOPOLIZATION_ENABLED` | Enable hot key protection   | true    |
| `monopolization_threshold` / `PROSODY_MONOPOLIZATION_THRESHOLD` | Max handler time as fraction of window | 0.9 |
| `monopolization_window` / `PROSODY_MONOPOLIZATION_WINDOW` | Measurement window            | 5m      |
| `monopolization_cache_size` / `PROSODY_MONOPOLIZATION_CACHE_SIZE` | Max distinct keys to track  | 8192    |

### Fair Scheduling (All Modes)

| Option / Environment Variable           | Description                                                      | Default |
|-----------------------------------------|------------------------------------------------------------------|---------|
| `scheduler_failure_weight` / `PROSODY_SCHEDULER_FAILURE_WEIGHT` | Fraction of processing time reserved for retries | 0.3    |
| `scheduler_max_wait` / `PROSODY_SCHEDULER_MAX_WAIT` | Messages waiting this long get maximum priority          | 2m      |
| `scheduler_wait_weight` / `PROSODY_SCHEDULER_WAIT_WEIGHT` | Priority boost for waiting messages (higher = more aggressive) | 200.0 |
| `scheduler_cache_size` / `PROSODY_SCHEDULER_CACHE_SIZE` | Max distinct keys to track                             | 8192    |

### Cassandra

Persistent storage for timers and deferred retries (not needed if `mock: true`):

| Option / Environment Variable           | Description                        | Default |
|-----------------------------------------|------------------------------------|---------|
| `cassandra_nodes` / `PROSODY_CASSANDRA_NODES` | Servers to connect to (host:port) | -      |
| `cassandra_keyspace` / `PROSODY_CASSANDRA_KEYSPACE` | Keyspace name              | prosody |
| `cassandra_user` / `PROSODY_CASSANDRA_USER` | Username                          | -       |
| `cassandra_password` / `PROSODY_CASSANDRA_PASSWORD` | Password                   | -       |
| `cassandra_datacenter` / `PROSODY_CASSANDRA_DATACENTER` | Prefer this datacenter for queries | - |
| `cassandra_rack` / `PROSODY_CASSANDRA_RACK` | Prefer this rack for queries      | -       |
| `cassandra_retention` / `PROSODY_CASSANDRA_RETENTION` | Delete data older than this | 1y     |

### Telemetry Emitter

Prosody can emit internal processing events (message lifecycle, timer events) to a Kafka topic for observability:

| Option / Environment Variable           | Description                                    | Default                    |
|-----------------------------------------|------------------------------------------------|----------------------------|
| `telemetry_topic` / `PROSODY_TELEMETRY_TOPIC` | Kafka topic to produce telemetry events to | `prosody.telemetry-events` |
| `telemetry_enabled` / `PROSODY_TELEMETRY_ENABLED` | Enable or disable the telemetry emitter  | true                       |

## Logging

Prosody exposes a module-level logger used by both the native Rust extension and the Ruby async processor. By default it
writes to `$stdout` at the `INFO` level.

```ruby
# Read the current logger
Prosody.logger
# => #<Logger:... @level=1 ...>

# Assign a custom logger
Prosody.logger = Logger.new("log/prosody.log", level: Logger::DEBUG)

# Or silence logging entirely
Prosody.logger = Logger.new(File::NULL)
```

Set `Prosody.logger` **before** creating a `Prosody::Client`. The Rust runtime reads the logger on first client
initialization and will use whatever logger is configured at that point.

Setting the logger back to `nil` restores the default:

```ruby
Prosody.logger = nil
Prosody.logger.level  # => Logger::INFO
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
  probe_port: 8000,        # Set to false to disable
  stall_threshold: 15.0    # Seconds before considering a partition stalled
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

You can monitor the stall state programmatically using the client's methods:

```ruby
# Get the number of partitions currently assigned to this consumer
partition_count = client.assigned_partitions

# Check if the consumer has stalled partitions
if client.is_stalled?
  warn 'Consumer has stalled partitions'
end
```

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

Matches:
- `{"type": "user.created"}` matches prefix `user.`
- `{"type": "account.deleted"}` matches prefix `account.`

No Match:
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

The deduplication system uses:
- A **global in-memory cache** shared across all partitions, surviving partition reassignments within a process
- A **Cassandra-backed persistent store** for cross-restart deduplication

```ruby
# Messages with IDs are deduplicated per key
client.send_message("my-topic", "key1", {
  id: "msg-123",      # Message will be processed
  content: "Hello!"
})

client.send_message("my-topic", "key1", {
  id: "msg-123",      # Message will be skipped (duplicate)
  content: "Hello again!"
})

client.send_message("my-topic", "key2", {
  id: "msg-123",      # Message will be processed (different key)
  content: "Hello!"
})
```

Setting `idempotence_cache_size` to `0` disables the **entire** deduplication middleware (both the in-memory cache and the Cassandra-backed persistent store):

```ruby
client = Prosody::Client.new(
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic",
  idempotence_cache_size: 0  # Disable all deduplication (both in-memory and persistent)
)
```

Or via environment variable:

```bash
PROSODY_IDEMPOTENCE_CACHE_SIZE=0
```

To invalidate all previously recorded dedup entries (e.g. after a data migration), change the version string:

```ruby
client = Prosody::Client.new(
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic",
  idempotence_version: "2"  # Changing this invalidates all existing dedup records
)
```

The `idempotence_ttl` option controls how long dedup records are retained in Cassandra (default: 7 days):

```ruby
client = Prosody::Client.new(
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic",
  idempotence_ttl: 86400.0  # Keep dedup records for 1 day
)
```

Note that the in-memory cache is best-effort. Duplicates can still occur across different process instances.

## Timer Functionality

Prosody supports timer-based delayed execution within message handlers. When a timer fires, your handler's `on_timer` method will be called:

```ruby
class MyHandler < Prosody::EventHandler
  def on_message(context, message)
    # Schedule a timer to fire in 30 seconds
    future_time = Time.now + 30
    context.schedule(future_time)

    # Schedule multiple timers
    one_minute = Time.now + 60
    two_minutes = Time.now + 120
    context.schedule(one_minute)
    context.schedule(two_minutes)

    # Check what's scheduled
    scheduled_times = context.scheduled
    puts "Scheduled timers: #{scheduled_times.length}"
  end

  def on_timer(context, timer)
    puts "Timer fired!"
    puts "Key: #{timer.key}"
    puts "Scheduled time: #{timer.time}"
  end
end
```

### Timer Methods

The context provides timer scheduling methods that allow you to delay execution or implement timeout behavior:

- `schedule(time)`: Schedules a timer to fire at the specified time
- `clear_and_schedule(time)`: Clears all timers and schedules a new one
- `unschedule(time)`: Removes a timer scheduled for the specified time
- `clear_scheduled`: Removes all scheduled timers
- `scheduled`: Returns an array of all scheduled timer times

### Timer Object

When a timer fires, the `on_timer` method receives a timer object with these properties:

- `key` (String): The entity key identifying what this timer belongs to
- `time` (Time): The time when this timer was scheduled to fire

**Note**: Timer precision is limited to seconds due to the underlying storage format. Sub-second precision in scheduled times will be rounded to the nearest second.

### Timer Configuration

Timer functionality requires Cassandra for persistence unless running in mock mode. Configure Cassandra connection via environment variable:

```bash
PROSODY_CASSANDRA_NODES=localhost:9042  # Required for timer persistence
```

Or programmatically when creating the client:

```ruby
client = Prosody::Client.new(
  bootstrap_servers: "localhost:9092",
  group_id: "my-application",
  subscribed_topics: "my-topic",
  cassandra_nodes: "localhost:9042"  # Required unless mock: true
)
```

For testing, you can use mock mode to avoid Cassandra dependency:

```ruby
# Mock mode for testing (timers work but aren't persisted)
client = Prosody::Client.new(
  bootstrap_servers: "localhost:9092",
  group_id: "my-application",
  subscribed_topics: "my-topic",
  mock: true  # No Cassandra required in mock mode
)
```

## OpenTelemetry Tracing

Prosody supports OpenTelemetry tracing, allowing you to monitor and analyze the performance of your Kafka-based
applications. The library will emit traces using the OTLP protocol if the `OTEL_EXPORTER_OTLP_ENDPOINT` environment
variable is defined.

Note: Prosody emits its own traces separately because it uses its own tracing runtime, as it would be expensive to send
all traces to Ruby.

### Required Gems

To use OpenTelemetry tracing with Prosody, you need to install the following gems:

```ruby
gem 'opentelemetry-sdk', '~> 1.10'
gem 'opentelemetry-api', '~> 1.7'
gem 'opentelemetry-exporter-otlp', '~> 0.31'
```

### Initializing Tracing

To initialize tracing in your application:

```ruby
require 'opentelemetry/sdk'
require 'opentelemetry/exporter/otlp'

OpenTelemetry::SDK.configure do |c|
  c.service_name = 'my-service-name'
  c.add_span_processor(
    OpenTelemetry::SDK::Trace::Export::BatchSpanProcessor.new(
      OpenTelemetry::Exporter::OTLP::Exporter.new
    )
  )
end

tracer = OpenTelemetry.tracer_provider.tracer('my-service-name')
```

### Setting OpenTelemetry Environment Variables

Set the following standard OpenTelemetry environment variables:

```bash
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
OTEL_SERVICE_NAME=my-service-name
```

For more information on these and other OpenTelemetry environment variables, refer to
the [OpenTelemetry specification](https://opentelemetry.io/docs/specs/otel/configuration/sdk-environment-variables/#general-sdk-configuration).

### Using Tracing in Your Application

After initializing tracing, you can define spans in your application, and they will be properly propagated through
Kafka:

```ruby
class MyHandler < Prosody::EventHandler
  def initialize
    @tracer = OpenTelemetry.tracer_provider.tracer('my-service-name')
  end

  def on_message(context, message)
    @tracer.in_span('process-message') do |span|
      # Process the received message
      span.add_event('message.received', attributes: {
        'message.payload' => message.payload.to_json
      })
    end
  end
end
```

## Best Practices

### Ensuring Thread-Safe Handlers

Your event handler methods will be called concurrently. Avoid using mutable shared state across event handler calls.
If you must use shared state, use appropriate synchronization primitives.

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

Implement shutdown handling in your application using signal handlers:

```ruby
require "prosody"

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

### Error Handling

Prosody classifies errors as transient (temporary, can be retried) or permanent (won't be resolved by retrying). By
default, all errors are considered transient.

Use the `Prosody::EventHandler` error classification methods:

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

Best practices:

- Use permanent errors for issues like malformed data or business logic violations.
- Use transient errors for temporary issues like network problems.
- Be cautious with permanent errors as they prevent retries and can result in data loss.
- Consider system reliability and data consistency when classifying errors.

### Handling Task Cancellation

Prosody cancels tasks during partition rebalancing or timeout. During shutdown, handlers run freely for most of the shutdown timeout before the cancellation signal fires — giving in-flight work time to complete. When cancelled, your handler receives `Async::Stop` at the next yield point (I/O operation, sleep, etc.).

Best practices:

1. Use `ensure` blocks for resource cleanup—they run even when `Async::Stop` is raised.
2. For CPU-bound loops that don't yield, check `context.should_cancel?` periodically.
3. Exit promptly when cancelled to avoid rebalancing delays.

```ruby
class MyHandler < Prosody::EventHandler
  def on_message(context, message)
    resource = acquire_resource
    begin
      items = message.payload["items"]
      items.each do |item|
        # For CPU-bound work, check cancellation periodically
        return if context.should_cancel?

        process_item(item)
      end
    ensure
      # Always runs, even on Async::Stop
      release_resource(resource)
    end
  end
end
```

If you catch `Async::Stop` and don't re-raise it, Prosody considers the task successful:

```ruby
def on_message(context, message)
  do_work
rescue Async::Stop
  # Custom cleanup on cancellation
  cleanup
  raise  # Re-raise to signal cancellation to Prosody
end
```

Failing to handle cancellation properly can lead to resource leaks or delayed rebalancing.

## Release Process

Prosody uses an automated release process managed by GitHub Actions. Here's an overview of how releases are handled:

1. **Trigger**: The release process is triggered automatically on pushes to the `main` branch.

2. **Release Please**: The process starts with the "Release Please" action, which:
    - Analyzes commit messages since the last release.
    - Creates or updates a release pull request with changelog updates and version bumps.
    - When the PR is merged, it creates a GitHub release and a git tag.

3. **Build Process**: If a new release is created, the following build jobs are triggered:
    - Linux builds for x86_64 and aarch64 architectures.
    - Linux musl builds for the same architectures.
    - macOS builds for x86_64 and arm64 architectures.
    - Windows builds for x64 architecture.

4. **Artifact Upload**: Each build job uploads its artifacts (Ruby native extensions) to GitHub Actions.

5. **Publication**: If all builds are successful, the final step publishes the built gems.

### Contributing to Releases

To contribute to a release:

1. Make your changes in a feature branch.
2. Use [Conventional Commits](https://www.conventionalcommits.org/) syntax for your commit messages. This helps Release
   Please determine the next version number and generate the changelog.
3. Create a pull request to merge your changes into the `main` branch.
4. Once your PR is approved and merged, Release Please will include your changes in the next release PR.

### Manual Releases

While the process is automated, manual intervention may sometimes be necessary:

- You can manually trigger the release workflow from the GitHub Actions tab if needed.
- If you need to make changes to the release PR created by Release Please, you can do so before merging it.

Ensure you have thoroughly tested your changes before merging to `main`.

## API Reference

### Prosody::Client

- `new(**config)`: Initialize a new Prosody client with the given configuration.
- `send_message(topic, key, payload)`: Send a message to a specified topic.
- `consumer_state`: Get the current state of the consumer (`:unconfigured`, `:configured`, or `:running`).
- `source_system`: Get the source system identifier configured for the client.
- `subscribe(handler)`: Subscribe to messages using the provided handler.
- `unsubscribe`: Unsubscribe from messages and shut down the consumer.
- `assigned_partitions`: Get the number of partitions currently assigned to this consumer.
- `is_stalled?`: Check if the consumer has stalled partitions.

### Prosody::EventHandler

A base class for user-defined handlers:

```ruby
class MyHandler < Prosody::EventHandler
  # Optional error classification
  permanent :on_message, TypeError
  transient :on_message, JSON::ParserError

  def on_message(context, message)
    # Implement your message handling logic here
  end

  def on_timer(context, timer)
    # Implement your timer handling logic here
  end
end
```

### Prosody::Message

Represents a Kafka message with the following attributes:

- `topic` (String): The name of the topic.
- `partition` (Integer): The partition number.
- `offset` (Integer): The message offset within the partition.
- `timestamp` (Time): The timestamp when the message was created or sent.
- `key` (String): The message key.
- `payload` (Hash/Array/String): The message payload as a JSON-deserializable value.

### Prosody::Context

Represents the context of message processing:

- `should_cancel?`: Check if cancellation has been requested (includes timeout and shutdown).
- `on_cancel`: Blocks until cancellation is signaled.

Timer scheduling methods:

- `schedule(time)`: Schedules a timer to fire at the specified time
- `clear_and_schedule(time)`: Clears all timers and schedules a new one
- `unschedule(time)`: Removes a timer scheduled for the specified time
- `clear_scheduled`: Removes all scheduled timers
- `scheduled`: Returns an array of all scheduled timer times

### Prosody::Timer

Represents a timer that has fired, provided to the `on_timer` method:

- `key` (String): The entity key identifying what this timer belongs to
- `time` (Time): The time when this timer was scheduled to fire
