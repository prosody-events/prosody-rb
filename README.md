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

The gem ships RBS signatures for the public API. `Prosody::EventHandler[Payload]`
carries an application payload type into `Prosody::Message[Payload]`, and keyed-
state definitions carry their item types through `context.state`. A bare handler,
message, definition, or state handle defaults to `Prosody::json_value`. See the
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
| `shutdown_timeout` / `PROSODY_SHUTDOWN_TIMEOUT` | Shutdown budget; handlers run freely until cancellation fires near the end of the timeout | 30s |
| `stall_threshold` / `PROSODY_STALL_THRESHOLD` | Report unhealthy if no progress for this long  | 5m                     |
| `probe_port` / `PROSODY_PROBE_PORT`     | HTTP port for health checks (nil to disable)         | 8000                   |
| `failure_topic` / `PROSODY_FAILURE_TOPIC` | Send unprocessable messages here (dead letter queue) | -                     |
| `idempotence_cache_size` / `PROSODY_IDEMPOTENCE_CACHE_SIZE` | Global shared cache capacity across all partitions for message deduplication. Consumer deduplication is mandatory and cannot be disabled, so this must be at least 1; setting it to 0 in the client configuration is rejected | 8192 |
| `idempotence_version` / `PROSODY_IDEMPOTENCE_VERSION` | Version string for cache-busting dedup hashes | 1              |
| `idempotence_ttl` / `PROSODY_IDEMPOTENCE_TTL`         | TTL for dedup records in Cassandra            | 7d (604800 seconds) |
| `slab_size` / `PROSODY_SLAB_SIZE`       | Timer storage granularity (rarely needs changing)    | 1h                     |
| `message_spans` / `PROSODY_MESSAGE_SPANS` | Span linking for message execution: `child` (child-of) or `follows_from` | `child` |
| `timer_spans` / `PROSODY_TIMER_SPANS`   | Span linking for timer execution: `child` (child-of) or `follows_from`   | `follows_from` |

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
| `defer_store_cache_size` / `PROSODY_DEFER_STORE_CACHE_SIZE` | Maximum deferred store cache entries per Cassandra defer store | 8192 |
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

### Keyed State

Register keyed-state collections before you subscribe. Persistence is backed by Cassandra and is not needed when `mock: true`. See the [Keyed State](#keyed-state-1) feature section for handler usage; the client-level knobs and per-collection fields are below. Where an option and an environment variable are paired, an explicitly set option wins; otherwise the environment variable applies, then the default.

| Option / Environment Variable | Description | Default |
|-------------------------------|-------------|---------|
| `state_collections` / - | Keyed-state collections to register before subscribe (array of definitions or config hashes; duplicate names rejected) | (none) |
| `state_cache_dir` / `PROSODY_FJALL_CACHE_DIR` | Root directory for the local committed-value cache; each live client needs its own directory (it is locked exclusively) | per-client temp dir |
| `state_recovery_delay` / `PROSODY_KEYED_STATE_RECOVERY_DELAY` | Whole-second delay between staging a provisional cell and the recovery sweep; every collection TTL must strictly exceed it | 30s |

Prefer the definition constructors (`Prosody.value` / `.map` / `.deque` and their `message_*` variants, documented below): they serialize into `state_collections` so you declare each collection once and reuse the same object with `context.state`. Each entry has these fields:

| Field | Description | Default |
|-------|-------------|---------|
| `name` | Collection name; non-empty and unique within the client | (required) |
| `kind` | `"value"`, `"map"`, or `"deque"` | (required) |
| `payload` | `"json"` (JSON values) or `"message"` (the full Kafka message the handler received) | (required) |
| `ttl_seconds` | Per-write TTL in whole seconds (at least 1; must exceed the recovery delay) | (none) |
| `read_uncommitted` | Opt out of transactional staging | false |
| `keyset_limit` | Map-only; ordered-scan bound in `0..=4096` (`0` disables ordered-scan tracking) | 128 |
| `capacity` | Deque-only window bound (at least 1); keeps at most N slots, enforced lazily on push. Runtime-only and mutable across deploys — not persisted | unbounded |

Constructors set these via keyword arguments (`ttl:`, `keyset_limit:`, `capacity:`, `read_uncommitted:`).

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

Prosody supports keyed state: per-key data that a handler reads and writes and that survives across events. State is partitioned by the message key, so each key has a single writer at a time, and by default writes settle atomically with the event — a handler that raises leaves no partial state. Values are either JSON payloads or the full Kafka `Prosody::Message` the handler received. Register collections on the client before subscribing, then bind them inside the handler with `context.state(definition)`. Every operation is fiber-yield async: it looks like an ordinary blocking call but yields the fiber (never the thread) while the Rust core drives it, exactly like the rest of Prosody (see [ARCHITECTURE.md](ARCHITECTURE.md)).

**Quickstart — one durable counter.** A `Value` gives every Kafka key durable local memory: update it in the handler, and Prosody publishes the new state only when that event succeeds — even across restarts and rebalances.

```ruby
COUNTER = Prosody.value("counter") # one ValueState per Kafka key

class CountHandler < Prosody::EventHandler
  def on_message(context, message)
    count = context.state(COUNTER)  # bound for this event
    count.set((count.get || 0) + 1) # read-modify-write; settles atomically with the event
  end
end

client = Prosody::Client.new(
  bootstrap_servers: "localhost:9092",
  group_id: "counts",
  subscribed_topics: "events",
  state_collections: [COUNTER]
)
client.subscribe(CountHandler.new)
```

**Batch a burst of activity per user.** Your consumer reads a stream of activity events — likes, comments, follows — each tagged with the user it is about (the Kafka key). Notifying on every event spams an active user; what you want is to tell them the instant something happens, then, if more arrives right after, hold it and send a single summary a few minutes later.

By hand this is fiddly: you need a durable place to stash pending events *per user*, a timer *per user* to send the summary, and all of it has to survive a restart or the work moving to another machine. Prosody gives you exactly those two things — durable per-key state and a per-key timer:

1. **First event for a user** → send it now, mark that a batch is open, and set a timer for 5 minutes out.
2. **More events arrive before the timer fires** → don't notify again; just save each one.
3. **Timer fires** → send one summary of everything saved, then close the batch so the next event starts fresh.

```ruby
# Declare the collections once; register both via state_collections: [WINDOW, PENDING].
WINDOW  = Prosody.value("window")                          # is a batch open for this user?
PENDING = Prosody.message_deque("pending", capacity: 100)  # keep the latest 100 messages

class ActivityHandler < Prosody::EventHandler
  # message.key = user id; message.payload = { "actor" => ..., "action" => ... }
  def on_message(context, message)
    window  = context.state(WINDOW)   # bind THIS user's handles for THIS event
    pending = context.state(PENDING)
    if window.get
      pending.push(message)           # a batch is open → just save the message
    else
      notify(message.key, [message])  # first event → send it right away
      window.set(true)
      # clear_and_schedule (not schedule): timers are NOT rolled back with state,
      # so a retried event must not stack a second timer — this keeps exactly one.
      context.clear_and_schedule(Time.now + 5 * 60)
    end
  end

  def on_timer(context, timer)        # fires ~5 minutes later, for timer.key
    pending = context.state(PENDING)
    batch = []
    pending.each { |msg| batch << msg } # the scan resolves the saved messages concurrently
    notify(timer.key, batch) unless batch.empty? # one summary of what actually happened
    pending.clear                       # empty the buffer
    context.state(WINDOW).clear         # close the batch; the next event opens a fresh one
  end

  # Your own delivery (push, email, …) — the only thing here you write.
  def notify(user_id, activities)
    # ...
  end
end
```

The complete handler and its payload/state signatures are checked by Steep in
[`examples/keyed_state_windowing.rb`](examples/keyed_state_windowing.rb) and
[`examples/keyed_state_windowing.rbs`](examples/keyed_state_windowing.rbs).

`window.get` returns `true` or `nil` (the flag is only ever set to `true` or cleared), so it reads as "is a batch open?". A `message_deque` stores whole Kafka messages and resolves each back on read, so draining it with the `each` scan resolves the saved messages **concurrently** — a `shift`-per-item loop would be one Kafka fetch *serially per element* (the anti-pattern the codebase forbids), so drain via the scan then `clear`, never a `shift` loop. `capacity: 100` bounds the buffer so one unusually active user can't grow it without limit; on overflow the **oldest saved** message drops — never the one already delivered. The `WINDOW` flag is only ever `true` or **absent** — close it with `clear`, never `set(false)`; the timer, not the flag, owns *when* the batch ends. Prosody runs at most one handler at a time per key, so a message and the timer for the same user never overlap. One honesty caveat: sending a notification is an outside effect that isn't undone if the event is retried, so a retry may resend it; a production notifier should use an idempotency key or an outbox.

### Definitions

A definition constructor declares one collection and returns a frozen definition object carrying its `name`, `kind`, and `payload`. Reference that definition both in `Configuration#state_collections` (registration) and in `context.state` (binding) — declare each collection once and reuse it. (Reuse is a convenience, not a requirement: binding matches a definition to a registered collection by its `name`/`kind`/`payload` fields, not by object identity, so a structurally-equal definition also works.) Three kinds, each with a JSON variant (values are your JSON payload) and a message variant (values are the full Kafka `Prosody::Message`):

- `Prosody.value(name, ttl:, read_uncommitted:)`: single value. Vends a `ValueState`.
- `Prosody.map(name, ttl:, keyset_limit:, read_uncommitted:)`: ordered map with **String** keys. Vends a `MapState`.
- `Prosody.deque(name, ttl:, capacity:, read_uncommitted:)`: double-ended queue. Vends a `DequeState`.
- `Prosody.message_value(name, ...)`: single value holding a `Prosody::Message`. Vends a `ValueState`.
- `Prosody.message_map(name, ...)`: ordered map of `Prosody::Message` (String keys). Vends a `MapState`.
- `Prosody.message_deque(name, capacity:, ...)`: deque of `Prosody::Message`. Vends a `DequeState`.

Every constructor accepts `ttl:` (whole seconds) and `read_uncommitted:`; maps also accept `keyset_limit:`, and deques accept `capacity:`. `capacity:` bounds the window to at most N slots, enforced lazily **on push** (an overflowing push evicts the opposite end first, decode-free). It is runtime-only and mutable across deploys — never persisted and not part of the collection's identity — so a shrunk deque reports its old length until the next push trims it toward the new bound. Payloads cross the boundary as plain JSON with no runtime validation, so a definition documents the intended shape but does not enforce one. Map keys are always `String`.

### State Handles

`context.state(definition)` vends a typed handle bound to the collection for the current event attempt. The handle — and any iterator it opens — is valid only within the handler invocation that created it; there is no post-handler read window. Binding an unregistered name raises a `PermanentStateError`; so does a definition whose `kind` or `payload` disagrees with what was durably registered under that name (a schema conflict across deploys, validated by core at first use — not a Ruby object-identity check).

`ValueState`:

- `get`: reads the current value, or `nil` when absent.
- `set(value)`: buffers a write. Writing `nil` is rejected — call `clear`.
- `clear`: deletes the stored value.
- `value` / `value=`: idiomatic aliases of `get` / `set`, so a value cell reads and writes like an attribute (`cart.value`, `cart.value = basket`).
- `commit` / `rollback`: see [Commit and Rollback](#commit-and-rollback).

`MapState` (keys are always `String`):

- `get(key)`: reads the value for `key`, or `nil` when absent.
- `get_many(keys)`: reads several keys in one isolated batch, returning one entry per key in the same order (`result[i]` is the value for `keys[i]`); a missing key is `nil`.
- `set(key, value)`: inserts or overwrites. Writing `nil` is rejected — call `delete(key)`.
- `delete(key)`: removes `key`. Deliberately returns `nil`, not the removed value or a "was present" flag (a documented divergence from `Hash#delete`; surfacing it would force a hidden read on every delete).
- `clear`: removes every entry.
- `each_pair` / `reverse_each_pair`: see [Scan Iteration](#scan-iteration).
- `each_key` / `reverse_each_key`: yield each live key in key (or reverse-key) order; see [Scan Iteration](#scan-iteration). The key scan skips value decode and the resolver, so a message-backed map enumerates keys with **zero Kafka fetches** (not zero-I/O). There is deliberately no eager `keys` array, which would materialize the whole remote keyset.
- `commit` / `rollback`.

  Idiomatic `Hash`-style conveniences, each composed from the canonical ops above and doing only **bounded** reads (there is deliberately no `keys`/`values`/`to_h`/`count`, which would materialize the whole remote map):

- `[]` / `[]=` / `store`: aliases of `get` / `set` (`store` mirrors `Hash#store`, returning the stored value).
- `fetch(key, default)` / `fetch(key) { |key| ... }`: like `Hash#fetch` — the value when present, otherwise the block result, else the default, else a `KeyError`. One read.
- `key?` (aliases `has_key?` / `include?` / `member?`): a presence check — no value decode and no resolver run (not no-I/O). A message-backed map answers presence with **zero Kafka fetches** (`true` even for a present-but-unfetchable cell).
- `values_at(*keys)`: like `Hash#values_at`, in one batched read (`get_many`).
- `fetch_values(*keys)` / `slice(*keys)`: like their `Hash` namesakes, each in one batched read.
- `dig(key, *rest)`: like `Hash#dig` — one read for `key`, then digs into the returned local value.
- `each`: alias of `each_pair`.

`DequeState`:

- `push(value)`: appends at the back. Writing `nil` is rejected.
- `unshift(value)`: prepends at the front. Writing `nil` is rejected.
- `pop`: removes and returns the back element, or `nil` when empty.
- `shift`: removes and returns the front element, or `nil` when empty.
- `length` (aliased `size`): number of live elements.
- `empty?`: whether the deque holds no live elements.
- `get(index)`: reads the element at `index`, or `nil` when out of range. A non-negative `index` reads from the front; a negative one resolves Array-style — `-1` is the back element, `-n` the nth from the end. A fractional or non-Integer value is a caller mistake, rejected with a `TransientStateError`.
- `each` / `reverse_each`: see [Scan Iteration](#scan-iteration).
- `commit` / `rollback`.

  Idiomatic `Array`-style conveniences, composed from the canonical ops (bounded reads only — there is deliberately no `to_a`/`map`/`sort`, which would materialize the whole remote deque):

- `<<` / `append` / `prepend`: append at the back (`<<`, `append`) or front (`prepend`), each returning `self` for chaining.
- `first` / `last`: the front / back element, or `nil` when empty. Each is a single endpoint-slot read (one round trip, no length read). Under a TTL an expired endpoint slot reads `nil` even when live interior elements remain — a peek never searches inward.
- `fetch(index, default)` / `fetch(index) { |index| ... }`: like `Array#fetch` — negatives resolve from the end (`-1` is the back element), a fractional or non-Integer index raises `TransientStateError`.
- No `[]` or `at`: `get` and `fetch` take a single `Integer` index (negatives resolve from the back, Array-style), but the deque deliberately does not wear `Array`'s `[]`/`at`, which would invite a range read (`deque[0..2]`) that a remote deque cannot honor. Use `get`, or `first` / `last` for the ends.

Writing a JSON `nil` to any handle raises `NullValueError` (a `TransientStateError`): `nil` is not storable because it is indistinguishable from absence, so the store is left untouched — use `clear`/`delete` to express deletion.

### Scan Iteration

Maps expose `each_pair` / `reverse_each_pair` (yielding `key, value`) and `each_key` / `reverse_each_key` (yielding keys only, skipping value decode and the resolver); deques expose `each` / `reverse_each` (yielding elements). The `reverse_*` variant is the backward direction — there is no direction argument. Called without a block, each returns an `Enumerator`; each step yields the fiber while the next chunk is fetched.

Iterators are valid only within the attempt that opened them. Exiting the loop early — a `break`, `return`, or a raised exception — closes the underlying native cursor via `ensure`, so an early exit releases the scan promptly:

```ruby
context.state(totals).reverse_each_pair do |key, total|
  break if total > 1000 # early exit closes the cursor
  process(key, total)
end
```

Ruby deliberately does **not** mix in `Enumerable`: its aggregate methods (`map`, `to_a`, `select`, ...) would silently materialize an unbounded remote collection. Traversal is explicit — iterate with the block form, or drive the returned `Enumerator` one step at a time.

### Commit and Rollback

Every handle exposes `commit` and `rollback`. By default a handler's writes are buffered and settle atomically when the event completes; commit and rollback are the explicit mid-handler escape hatch.

- `commit` durably flushes this collection's buffered operations mid-handler. It is at-least-once: the flush becomes visible even if the event later fails and is redelivered, and it establishes a floor that a later `rollback` cannot cross.
- `rollback` discards this collection's buffered uncommitted operations back to the last commit floor. It is infallible.

Both return `nil`. The erased core seam deliberately drops the store outcome, so there is **no** `:applied` / `:noop` return — do not expect one.

### Semantics

- **Per-key single writer.** State is keyed by the message key; only one handler invocation writes a given key at a time.
- **Transactional by default.** A handler's writes settle atomically with the event. A handler that raises leaves no partial state (unless you opted a collection into `read_uncommitted:`, or flushed explicitly with `commit`).
- **At-least-once.** Redelivery re-runs the handler; reads reflect committed prior attempts. Keep handlers idempotent.
- **Attempt-scoped.** The context, the handles it vends, and any iterators those handles open are valid only within the handler invocation that created them. Do not retain them past the handler.

### Error Handling

Keyed-state failures surface as structured errors that flow through the same handler-error bridge as everything else (the transient/permanent category is carried as data, never parsed from the message):

- `TransientStateError` (subclasses `TransientError`): the default. A temporary store read/write failure, **and every caller mistake** — a rejected `nil`/unrepresentable write (use `clear`/`delete` instead), an item-shape mismatch, an out-of-range or non-Integer deque index, or an invalid scan direction. Caller mistakes are transient on purpose: a permanent error discards the in-flight message and can silently lose data, so a code error retries and stays visible (logs/metrics/lag) until you fix it. `NullValueError` is a `TransientStateError`.
- `PermanentStateError` (subclasses `PermanentError`): reserved for failures a retry cannot resolve in-process — an unregistered or identity-mismatched collection, a duplicate registration, or a bad TTL. (A handler may also raise one explicitly to declare its own failure permanent.)

State errors are never Terminal — the core folds Terminal into Transient. Because they subclass the existing `PermanentError` / `TransientError` hierarchy, rethrowing one from a handler classifies the event exactly like a plain permanent/transient error, with no bridge change.

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
- `consumer_state`: Get the current state of the consumer (`:unconfigured`, `:configured`, or `:running`).
- `source_system`: Get the source system identifier configured for the client.
- `subscribe: [Payload] (Prosody::EventHandler[Payload]) -> void`: Subscribe while preserving the handler's payload specialization.
- `unsubscribe`: Unsubscribe from messages and shut down the consumer.
- `assigned_partitions`: Get the number of partitions currently assigned to this consumer.
- `is_stalled?`: Check if the consumer has stalled partitions.

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

class OrderHandler < Prosody::EventHandler[order_event]
  def on_message: (Prosody::Context, Prosody::Message[order_event]) -> void
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

- `Prosody.value(name, ttl: nil, read_uncommitted: nil)`
- `Prosody.map(name, ttl: nil, keyset_limit: nil, read_uncommitted: nil)`
- `Prosody.deque(name, ttl: nil, read_uncommitted: nil)`
- `Prosody.message_value(name, ttl: nil, read_uncommitted: nil)`
- `Prosody.message_map(name, ttl: nil, keyset_limit: nil, read_uncommitted: nil)`
- `Prosody.message_deque(name, ttl: nil, read_uncommitted: nil)`

`Prosody::ValueState`:

- `get`, `set(value)`, `clear`, `commit`, `rollback`

`Prosody::MapState` (keys are `String`):

- `get(key)`, `get_many(keys)`, `set(key, value)`, `delete(key)` (returns `nil`), `clear`
- `each_pair` / `reverse_each_pair` (block or `Enumerator`), `commit`, `rollback`

`Prosody::DequeState`:

- `push(value)`, `unshift(value)`, `pop`, `shift`, `length` (aliased `size`), `empty?`, `get(index)`, `clear`
- `each` / `reverse_each` (block or `Enumerator`), `commit`, `rollback`

Errors:

- `Prosody::TransientStateError < Prosody::TransientError`: the default — a temporary store read/write failure, or any caller mistake (a `nil`/unrepresentable write, item-shape mismatch, out-of-range index, invalid scan direction), rejected transient so it retries rather than discarding the message.
- `Prosody::PermanentStateError < Prosody::PermanentError`: reserved for failures a retry cannot resolve in-process (unregistered/identity-mismatched collection, duplicate registration, bad TTL), or one a handler raises explicitly.
- `Prosody::NullValueError < Prosody::TransientStateError`: raised when a `nil` is written; use `clear`/`delete` instead.

State errors are never Terminal (core folds Terminal into Transient).
