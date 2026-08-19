# Prosody: Ruby Bindings for Kafka

Prosody offers Ruby bindings to the [Prosody Kafka client](https://github.com/prosody-events/prosody), providing
features for message production and consumption, including configurable retry mechanisms, failure handling
strategies, and integrated OpenTelemetry support for distributed tracing.

## Features

- **Kafka Consumer**: Per-key ordering with cross-key concurrency, offset management, consumer groups
- **Kafka Producer**: Idempotent delivery with configurable retries
- **Timer System**: Persistent scheduled execution backed by Cassandra or in-memory store
- **Keyed State**: Per-key value/map/deque collections that survive across events, transactional by default
- **Quality of Service**: Fair scheduling limits concurrency and prevents failures from starving fresh traffic. Pipeline mode adds deferred retry and monopolization detection
- **Distributed Tracing**: OpenTelemetry integration for tracing message flow across services
- **Backpressure**: Pauses partitions when handlers fall behind
- **Mocking**: In-memory Kafka broker for tests (`mock: true`)
- **Failure Handling**: Pipeline (retry forever), Low-Latency (dead letter), Best-Effort (log and skip)

## Installation

Add this line to your application's Gemfile:

```ruby
gem "prosody"
```

Or install directly:

```bash
gem install prosody
```

The gem ships RBS signatures for the public API. `Prosody::EventHandler[Payload, Response]`
carries the payload type into `Prosody::Message[Payload]`. It also checks each handler response.
State definitions carry their item types through `context.state`. A bare handler,
message, definition, or state handle uses `Prosody::json_value`. See the
[typed examples](examples/) for Ruby and companion RBS files checked by Steep.

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
  def on_excise(_context, message)
    puts "Excise key: #{message.key}"
  end

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
client.send_message("my-topic", "message-key", {"content" => "Hello, Kafka!"})
client.excise("my-topic", "obsolete-key")

# Ensure proper shutdown when done
client.shutdown
```

## Excise records

A compacted Kafka topic keeps the latest value for each key. To remove a key, Kafka needs a record with that key and no payload.

Call `excise(topic, key)` to send this record. Prosody sends received excise records to `on_excise`, not to `on_message`.

Each handler must implement `on_message`, `on_excise`, and `on_timer`. Subscription fails before consumption if a method is missing.

If an excise record is a request, return a response from `on_excise`. Prosody uses this response as the subsystem result.

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

For the complete configuration reference, see [CONFIGURATION.md](CONFIGURATION.md).

Constructor options take precedence. Unset options use environment variables, then library defaults.

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
> [!NOTE]
> **Rails users:** Because prosody-rb is built on the async gem, handlers run on fibers rather than threads. Rails defaults ActiveSupport's isolation level to `:thread`, which causes ActiveRecord connections to be shared across fibers and produces errors like:
>
> ```
> ActiveRecord::StatementInvalid: Mysql2::Error: This connection is in use by: #<Fiber
> ```
>
> Set the isolation level to `:fiber` in `config/application.rb`:
>
> ```ruby
> config.active_support.isolation_level = :fiber
> ```

### Client Stall State
You can monitor the stall state programmatically using the client's methods:

```ruby
# Get the number of partitions currently assigned to this consumer
partition_count = client.assigned_partitions

# Check if the consumer has stalled partitions
if client.is_stalled?
  warn 'Consumer has stalled partitions'
end
```

## Subsystems

Kafka uses a consumer group ID for client processes that share records. Prosody uses this ID to separate the keyed state of each consumer group.

The consumer group ID is part of the stream design. Applications must not use it in public interfaces for requests or published state.

Applications need a stable name when they send requests or read published state.

A subsystem provides a stable name. One or more consumer groups can use the same subsystem name.

Prosody does not combine results from these consumer groups. A request uses the first response for the subsystem. A published-state read uses one consumer group that publishes the state.

Callers use the subsystem name, not a consumer group ID. You can change the consumer groups. You do not need to change the callers.

## Requests

A normal Kafka send does not return consumer results. A request lets a producer wait for results from selected subsystems.

You can send a request from a handler or from other application code. The Prosody client does not need an active subscription.

Requests return one outcome for each selected subsystem. The result hash uses canonical subsystem names as keys.

Use `request_excise` to send an excise record and collect the same outcome type.

Do not rely on hash iteration order.

Prosody raises an error if the request cannot produce the complete result hash.

Do not wait for a request if the current consumer group must process it for the same key. That group cannot process it until the handler returns.

Message and excise handler return values become successful request outcomes. Each return value must have a JSON representation.

Return a JSON response from each message and excise handler:

Set `subsystem` to `inventory` on the client that subscribes this handler.

```ruby
class InventoryHandler < Prosody::EventHandler
  def on_message(_context, message)
    {"accepted" => message.key}
  end

  def on_excise(_context, message)
    {"excised" => message.key}
  end

  def on_timer(_context, _timer)
  end
end
```

Send the request:

Set `timeout` in seconds.

```ruby
subsystems = ["inventory", "billing"]
results = client.request(
  topic: "orders",
  key: "order-1",
  payload: {"type" => "order.created"},
  subsystems: subsystems,
  timeout: 2.0
)

results.each do |subsystem, outcome|
  if outcome.is_a?(Prosody::Failure)
    warn "#{subsystem}: #{outcome.error.message}"
  else
    puts "#{subsystem}: #{outcome.value}"
  end
end
```

The example can print these results:

```text
inventory: {"accepted"=>"order-1"}
billing: no response arrived before the deadline
```

Each value is a `Success` or `Failure`. Each failure contains one typed response error.

Each response error has one message.

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
  "id" => "msg-123",      # Message will be processed
  "content" => "Hello!"
})

client.send_message("my-topic", "key1", {
  "id" => "msg-123",      # Message will be skipped (duplicate)
  "content" => "Hello again!"
})

client.send_message("my-topic", "key2", {
  "id" => "msg-123",      # Message will be processed (different key)
  "content" => "Hello!"
})
```

Consumer deduplication is **mandatory** — it is the commit oracle that makes
keyed state correct — so it cannot be disabled. `idempotence_cache_size` must be
at least 1; setting it to `0` in the client configuration raises an
`ArgumentError`:

```ruby
client = Prosody::Client.new(
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic",
  idempotence_cache_size: 0  # Rejected: consumer deduplication cannot be disabled
)
```

This applies to every client — `Prosody::Client.new` always builds a consumer, so
`0` is rejected regardless of whether any topics are subscribed, and whether it is
supplied in the client configuration or via `PROSODY_IDEMPOTENCE_CACHE_SIZE`.

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

## Keyed State

A handler can process events for different keys concurrently. Prosody processes only one event at a time for each key. Many decisions need data from earlier events for the same key.

A Kafka key identifies the entity for an event, such as a customer or order. Keyed state stores separate data for each key. Prosody selects the current message or timer key automatically.

With Cassandra, keyed state survives a process restart. It also survives when Kafka assigns a partition to a different process. By default, Prosody commits keyed-state changes after the handler completes without an error. Prosody discards pending keyed-state changes from a failed attempt.

Use keyed state for counters, duplicate detection, rolling totals, pending work, and per-key workflows. Use a database for business records, joins, and unplanned queries. Repeated database reads can make stream processing slow and expensive.

Give most collections a time to live (TTL). Set the TTL beyond the longest timer or workflow that uses the collection. Omit it only when inactive keys must remain forever.

### A counter for each key

Declare each collection once. Register it on the client. In a handler, ask the event context for the current key's state:

```ruby
COUNTER = Prosody.value("counter", ttl: 30 * 24 * 60 * 60)

class CountHandler < Prosody::EventHandler
  def on_message(context, _message)
    count = context.state(COUNTER)
    count.set((count.get || 0) + 1)
  end

  def on_excise(_context, _message); end
  def on_timer(_context, _timer); end
end

client = Prosody::Client.new(
  group_id: "counters",
  subscribed_topics: "events",
  state_collections: [COUNTER]
)
```

Each Kafka key now has an independent counter. A counter expires when that key has no update for 30 days.

### Window activity into one notification

This example groups a burst of activity for one user. It sends the first event immediately. It collects later events for five minutes.

If later events arrive, the timer sends one summary when the window ends. The user ID is the Kafka key, so each user has an independent window.

```ruby
WINDOW = Prosody.value("window", ttl: 24 * 60 * 60)
PENDING = Prosody.message_deque("pending", capacity: 100, ttl: 24 * 60 * 60)

class ActivityHandler < Prosody::EventHandler
  def on_message(context, message)
    window = context.state(WINDOW)
    pending = context.state(PENDING)

    if window.get
      pending.push(message)
      return
    end

    notify(message.key, [message])
    window.set(true)
    context.clear_and_schedule(Time.now + 5 * 60)
  end

  def on_timer(context, timer)
    pending = context.state(PENDING)
    batch = []
    pending.each { |message| batch << message }

    notify(timer.key, batch) unless batch.empty?
    pending.clear
    context.state(WINDOW).clear
  end

  def on_excise(_context, _message); end
end
```

See the complete, Steep-checked example for signatures, client setup, and `notify`: [`examples/keyed_state_windowing.rb`](examples/keyed_state_windowing.rb) and [`examples/keyed_state_windowing.rbs`](examples/keyed_state_windowing.rbs).

Why this works:

- Register both definitions in `state_collections` before you subscribe. Keyed state uses Cassandra unless `mock: true`.
- Use `clear_and_schedule`, not `schedule`, so a retried event does not add another timer for the same key.
- `capacity: 100` and the one-day TTL bound the saved backlog. Overflow drops the oldest message because this example only appends.
- A `message_deque` requires the original Kafka messages during the window. Use `deque` when topic retention or compaction cannot provide them.
- Prosody runs one handler at a time for each key, so a user's message and timer handlers cannot overlap.
- A notification is outside the state transaction. A retry can send it again. Use a stable operation ID to reject duplicate notifications.

### Collections and handles

A definition sets a collection's durable name, kind, and options. Register each definition once on the client.

Pass the same definition to `context.state` inside a handler. Prosody uses the current event key for that handle.

Do not reuse a durable name for a different collection kind or payload type. Create handles inside the handler. Do not retain handles or iterators.

State operations look synchronous. They yield the current fiber while Prosody performs the work.

| Collection | JSON payload | Kafka message | Main operations |
| --- | --- | --- | --- |
| Value | `Prosody.value` | `Prosody.message_value` | `get`, `set`, `clear` |
| Ordered string map | `Prosody.map` | `Prosody.message_map` | `get`, `get_many`, `key?`, `set`, `delete`, `each_pair`, `each_key`, `clear` |
| Deque | `Prosody.deque` | `Prosody.message_deque` | `push`, `unshift`, `pop`, `shift`, `get`, `length`, `each`, `clear` |

Map and deque scans return enumerators when called without a block. Map keys are strings.

`nil` means absence. Do not store this value. Use `clear` or `delete`.

### When keyed-state changes become visible

Reads inside a handler see earlier keyed-state writes from that handler. By default, Prosody buffers keyed-state changes until the event succeeds.

Prosody then commits the pending keyed-state changes. If the handler raises, Prosody discards its pending keyed-state changes. This transaction does not include other handler side effects.

Each collection also offers explicit controls for workflows that need different behavior:

- `read_uncommitted: true` writes changes before Prosody records the event as complete. A crash can make these changes visible before a retry. Use this option only when each retry writes the same state.
- `commit` commits the collection's pending changes before the handler ends. A later handler failure does not remove them.
- `rollback` discards pending changes since the last `commit`. It cannot undo committed changes.

### Published state

Handlers normally read state only for their current event key. Sometimes another service needs that state but must not consume the owner's Kafka topics.

Published state provides this read-only access.

Configure the subsystem name on each publisher. Enable publication on the collection definition. Register the definition on the Prosody client:

```ruby
CURRENT_ORDER = Prosody.value("current-order", published: true)

owner = Prosody::Client.new(
  group_id: "order-writer",
  subsystem: "checkout",
  state_collections: [CURRENT_ORDER]
)

# The handler uses the key from its current event.
current_order = context.state(CURRENT_ORDER)
current_order.set({"sku" => "book"})
```

You can read published state from a handler or from other application code. The Prosody client does not need an active subscription.

Use the subsystem and the same definition to open a reader:

```ruby
order_reader = client.state("checkout", CURRENT_ORDER)
current_order = order_reader.get("customer-123")
```

The reader returns only committed state. It cannot change the collection. Each read takes an explicit key because no handler supplies one.

Map and deque readers fetch data in chunks. They do not load the complete collection before iteration starts. Readers return an `Enumerator` without a block.

Use `reverse_each_pair`, `reverse_each_key`, `reverse_each_value`, or `reverse_each` for reverse traversal.

The default cache window is five seconds. Set `read_cache:` to select a different window. Set `read_cache: false` to bypass the cache.

To stop publication, deploy the definition with `published: false`. Keep the definition registered during that deployment. Keep the subsystem configured during that deployment.

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

  def on_excise(_context, _message); end
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

  def on_excise(_context, _message); end
  def on_timer(_context, _timer); end
end
```

### Span Linking

By default, message execution spans use **`child`** (child-of relationship — the execution span is part of
the same trace as the producer). Timer execution spans use **`follows_from`** (the execution span starts a
new trace with a span link back to the scheduling span, since timer execution is causally related but not part of
the same operation).

Both strategies are configurable via the `message_spans` / `PROSODY_MESSAGE_SPANS` and `timer_spans` /
`PROSODY_TIMER_SPANS` options. Accepted values: `'child'`, `'follows_from'`.

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

### Application shutdown

`unsubscribe` stops only the active subscription. Other client services continue to run.

Call `shutdown` when the application terminates. Shutdown stops the active subscription and all other client services. The client rejects new operations after shutdown.

Call `unsubscribe` only when the application will use the client again. You do not need to call `unsubscribe` before `shutdown`.

```ruby
client.shutdown
```

Handle application shutdown with signal handlers:

```ruby
require "prosody"

client = Prosody::Client.new(
  bootstrap_servers: "localhost:9092",
  group_id: "my-consumer-group",
  subscribed_topics: "my-topic"
)

# Create the shutdown queue.
shutdown = Queue.new

# Register the signal handlers.
Signal.trap("INT") { shutdown.push(nil) }
Signal.trap("TERM") { shutdown.push(nil) }

# Subscribe with the application handler.
client.subscribe(MyHandler.new)

# Wait for a shutdown signal.
shutdown.pop

# Shut down the client.
puts "Client shutdown starts."
client.shutdown
```

### Error Handling

Prosody classifies errors as transient (temporary, can be retried) or permanent (won't be resolved by retrying). By
default, all errors are considered transient.

The error classes and classification methods apply to `on_message`, `on_excise`, and `on_timer`.

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

  def on_excise(_context, _message); end
  def on_timer(_context, _timer); end
end
```

Best practices:

- Use permanent errors for issues like malformed data or business logic violations.
- Use transient errors for temporary issues like network problems.
- Be cautious with permanent errors as they prevent retries and can result in data loss.
- Consider system reliability and data consistency when classifying errors.

### Handling Task Cancellation

Prosody cancels tasks during partition rebalancing, timeout, or shutdown. During shutdown, handlers run freely for most of the `shutdown_timeout` before the cancellation signal fires—giving in-flight work time to complete. When cancelled, your handler receives `Async::Stop` at the next yield point (I/O operation, sleep, etc.).

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

  def on_excise(_context, _message); end
  def on_timer(_context, _timer); end
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
- `send_message(String topic, String key, Prosody::json_value payload)`: Send a JSON-serializable message.
- `request(topic:, key:, payload:, subsystems:, timeout:)`: Return one outcome for each subsystem.
- `request_excise(topic:, key:, subsystems:, timeout:)`: Send an excise request.
- `consumer_state`: Get the client state (`:shut_down`, `:unconfigured`, `:configured`, or `:running`).
- `source_system`: Get the source system identifier configured for the client.
- `state(subsystem, definition)`: Open a typed, read-only published value, map, or deque.
- `subscribe: [Payload, Response] (Prosody::EventHandler[Payload, Response]) -> void`: Preserve both handler types.
- `unsubscribe`: Stop the consumer. You can subscribe again later.
- `shutdown`: Stop all client services. Concurrent and repeated calls wait for the same operation.
- `assigned_partitions`: Get the number of partitions currently assigned to this consumer.
- `is_stalled?`: Check if the consumer has stalled partitions.

### Prosody::AdminClient

- `new(bootstrap_servers)`: Create an admin client for the specified Kafka servers.
- `create_topic(name, partitions, replication_factor)`: Create a Kafka topic.
- `delete_topic(name)`: Delete a Kafka topic.

### Prosody::EventHandler

A base class for user-defined handlers. Its RBS payload parameter flows into
`Message#payload`; a bare handler defaults to `Prosody::json_value`.

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

  def on_excise(_context, _message)
    # Implement your excise handling logic here
  end
end
```

### Prosody::Message

`Prosody::Message[Payload]` represents a Kafka message. `Payload` defaults to
`Prosody::json_value` (`nil`, booleans, numbers, strings, arrays, and
string-keyed hashes, recursively). The parameter is static documentation and
does not perform runtime validation.

For a type-safe handler, describe the JSON record and specialize the handler in
your application's RBS:

```rbs
type order_event = { "order_id" => String, "total" => Integer }
type response = { "accepted" => bool }

class OrderHandler < Prosody::EventHandler[order_event, response]
  def on_message: (Prosody::Context, Prosody::Message[order_event]) -> response
  def on_excise: (Prosody::Context, Prosody::ExciseMessage) -> response
end
```

Ruby can then use `message.payload["order_id"]` as a `String` and
`message.payload["total"]` as an `Integer`. See
[`examples/keyed_state.rb`](examples/keyed_state.rb) and its companion
[`examples/keyed_state.rbs`](examples/keyed_state.rbs) for payload typing that
also flows through message-backed state.

Messages have the following attributes:

- `topic` (String): The name of the topic.
- `partition` (Integer): The partition number.
- `offset` (Integer): The message offset within the partition.
- `timestamp` (Time): The timestamp when the message was created or sent.
- `key` (String): The message key.
- `payload` (`Payload`): The JSON-deserialized message payload.

### Prosody::Context

Represents the context of message processing:

- `should_cancel?`: Check if cancellation has been requested (includes timeout and shutdown).
- `on_cancel`: Blocks until cancellation is signaled.
- `state(definition)`: Binds a registered collection for the current event attempt, returning a typed handle (`ValueState`, `MapState`, or `DequeState`). Raises `PermanentStateError` when the name was never registered, or when the definition's `kind`/`payload` disagrees with the collection's durably-registered schema. See the [Keyed State](#keyed-state-2) API reference below.

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

### Keyed State

Definition constructors (each returns a frozen definition object used both in `Configuration#state_collections` and with `context.state`):

- `Prosody.value(name, ttl: nil, read_uncommitted: nil, published: nil, read_cache: nil)`
- `Prosody.map(name, ttl: nil, keyset_limit: nil, read_uncommitted: nil, published: nil, read_cache: nil)`
- `Prosody.deque(name, ttl: nil, capacity: nil, read_uncommitted: nil, published: nil, read_cache: nil)`
- `Prosody.message_value(name, ttl: nil, read_uncommitted: nil)`
- `Prosody.message_map(name, ttl: nil, keyset_limit: nil, read_uncommitted: nil)`
- `Prosody.message_deque(name, ttl: nil, capacity: nil, read_uncommitted: nil)`

Published readers take the user key as their first argument. `Prosody::PublishedValue` provides `get`. `Prosody::PublishedMap` provides `get`, `get_many`, `key?`, `each_pair`, `each_key`, `each_value`, and their reverse variants. `Prosody::PublishedDeque` provides `get`, `length`/`size`, `empty?`, `first`, `last`, `each`, and `reverse_each`. Traversal methods return an `Enumerator` when no block is given.

`Prosody::ValueState`:

- `get`, `set(value)`, `clear`, `commit`, `rollback`

`Prosody::MapState` (keys are `String`):

- `get(key)`, `get_many(keys)`, `set(key, value)`, `delete(key)` (returns `nil`), `clear`
- `key?`, `each_pair`, `each_key`, and `each_value` (each with reverse traversal), `commit`, `rollback`

`Prosody::DequeState`:

- `push(value)`, `unshift(value)`, `pop`, `shift`, `length` (aliased `size`), `empty?`, `get(index)`, `clear`
- `each` / `reverse_each` (block or `Enumerator`), `commit`, `rollback`

Errors:

- `Prosody::TransientStateError < Prosody::TransientError`: the default — a temporary store read/write failure, or any caller mistake (a `nil`/unrepresentable write, item-shape mismatch, out-of-range index, invalid scan direction), rejected transient so it retries rather than discarding the message.
- `Prosody::PermanentStateError < Prosody::PermanentError`: reserved for failures a retry cannot resolve in-process (unregistered/identity-mismatched collection, duplicate registration, bad TTL), or one a handler raises explicitly.
- `Prosody::NullValueError < Prosody::TransientStateError`: raised when a `nil` is written; use `clear`/`delete` instead.

State errors are never Terminal (core folds Terminal into Transient).
