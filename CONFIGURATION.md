# Configuration

Configure via constructor options or environment variables. Options fall back to environment variables when unset.

The Ruby client reports values it cannot convert to Prosody types. Prosody validates configuration semantics when the client is built.

## Core

| Option / Environment Variable           | Description                                       | Default      |
|-----------------------------------------|---------------------------------------------------|--------------|
| `bootstrap_servers` / `PROSODY_BOOTSTRAP_SERVERS` | Kafka servers to connect to             | -            |
| `group_id` / `PROSODY_GROUP_ID`         | Consumer group name                               | -            |
| `subscribed_topics` / `PROSODY_SUBSCRIBED_TOPICS` | Topics to read from                     | -            |
| `allowed_events` / `PROSODY_ALLOWED_EVENTS` | Only process events matching these prefixes   | (all)        |
| `source_system` / `PROSODY_SOURCE_SYSTEM` | Tag for outgoing messages (prevents reprocessing)| `<group_id>` |
| `mock` / `PROSODY_MOCK`                 | Use in-memory Kafka for testing                   | false        |
| `mode` / -                              | Processing mode: `pipeline`, `low_latency`, or `best_effort` | `pipeline` |
| - / `PROSODY_LOG`                       | Rust log filter, such as `info` or `prosody=debug` | `info` |

## Peer requests

| Option / Environment Variable | Description | Default |
|--------------------------------|-------------|---------|
| `peer_bind_address` / `PROSODY_PEER_BIND_ADDRESS` | Address for the peer listener | `0.0.0.0:0` |
| `peer_advertised_connect` / `PROSODY_PEER_ADVERTISED_CONNECT` | gRPC connect URI that other clients use | - |
| `peer_network_name` / `PROSODY_PEER_NETWORK_NAME` | Network name used to identify direct routes | - |
| `peer_cache_capacity` / `PROSODY_PEER_CACHE_CAPACITY` | Peer channels and registrations held in each cache | 256 |
| `peer_registration_ttl` / `PROSODY_PEER_REGISTRATION_TTL` | Duration of each peer registration lease | 30s |

## Consumer

| Option / Environment Variable           | Description                                          | Default                |
|-----------------------------------------|------------------------------------------------------|------------------------|
| `max_concurrency` / `PROSODY_MAX_CONCURRENCY` | Max messages being processed simultaneously     | 32                     |
| `max_uncommitted` / `PROSODY_MAX_UNCOMMITTED` | Max queued messages before pausing consumption  | 64                     |
| `timeout` / `PROSODY_TIMEOUT`           | Cancel handler if it runs longer than this           | 80% of stall threshold |
| `commit_interval` / `PROSODY_COMMIT_INTERVAL` | How often to save progress to Kafka            | 1s                     |
| `poll_interval` / `PROSODY_POLL_INTERVAL` | How often to fetch new messages from Kafka         | 100ms                  |
| `shutdown_timeout` / `PROSODY_SHUTDOWN_TIMEOUT` | Shutdown budget; handlers run freely until cancellation fires near the end of the timeout | 30s |
| `stall_threshold` / `PROSODY_STALL_THRESHOLD` | Report unhealthy if no progress for this long  | 5m                     |
| `probe_port` / `PROSODY_PROBE_PORT`     | HTTP port for health checks; use `false`, `:disabled`, or the environment value `none` to disable | 8000 |
| - / `PROSODY_STATISTICS_INTERVAL`       | How often librdkafka reports client statistics; must be between 1ms and 24h | 5s |
| `failure_topic` / `PROSODY_FAILURE_TOPIC` | Send unprocessable messages here (dead letter queue) | -                     |
| `idempotence_cache_size` / `PROSODY_IDEMPOTENCE_CACHE_SIZE` | Global shared cache capacity across all partitions for message deduplication. Consumer deduplication is mandatory and cannot be disabled, so this must be at least 1; setting it to 0 in the client configuration is rejected | 8192 |
| `idempotence_version` / `PROSODY_IDEMPOTENCE_VERSION` | Version string for cache-busting dedup hashes | 1              |
| `idempotence_ttl` / `PROSODY_IDEMPOTENCE_TTL`         | TTL for dedup records in Cassandra            | 7d (604800 seconds) |
| `slab_size` / `PROSODY_SLAB_SIZE`       | Timer storage granularity (rarely needs changing)    | 1h                     |
| `message_spans` / `PROSODY_MESSAGE_SPANS` | Span linking for message execution: `child` (child-of) or `follows_from` | `child` |
| `timer_spans` / `PROSODY_TIMER_SPANS`   | Span linking for timer execution: `child` (child-of) or `follows_from`   | `follows_from` |

## Producer

| Option / Environment Variable           | Description                     | Default |
|-----------------------------------------|---------------------------------|---------|
| `send_timeout` / `PROSODY_SEND_TIMEOUT` | Give up sending after this long | 1s      |

## Retry

Retry backoff applies in pipeline and low-latency modes. `max_retries` controls how many retries low-latency mode performs before routing the failure to `failure_topic`. Pipeline mode uses deferral and does not use this limit.

| Option / Environment Variable           | Description                       | Default |
|-----------------------------------------|-----------------------------------|---------|
| `max_retries` / `PROSODY_MAX_RETRIES`   | Low-latency retries before routing to the failure topic | 3       |
| `retry_base` / `PROSODY_RETRY_BASE`     | Wait this long before first retry | 20ms    |
| `max_retry_delay` / `PROSODY_RETRY_MAX_DELAY` | Never wait longer than this  | 5m      |

## Deferral (Pipeline Mode)

| Option / Environment Variable           | Description                                       | Default |
|-----------------------------------------|---------------------------------------------------|---------|
| `defer_enabled` / `PROSODY_DEFER_ENABLED` | Enable deferral for new messages                | true    |
| `defer_base` / `PROSODY_DEFER_BASE`     | Wait this long before first deferred retry        | 1s      |
| `defer_max_delay` / `PROSODY_DEFER_MAX_DELAY` | Never wait longer than this                 | 24h     |
| `defer_failure_threshold` / `PROSODY_DEFER_FAILURE_THRESHOLD` | Disable deferral when failure rate exceeds this | 0.9 |
| `defer_failure_window` / `PROSODY_DEFER_FAILURE_WINDOW` | Measure failure rate over this time window | 5m     |
| `defer_store_cache_size` / `PROSODY_DEFER_STORE_CACHE_SIZE` | Maximum deferred store cache entries per Cassandra defer store | 8192 |

## Kafka Message Loader (All Modes)

The shared loader resolves Kafka messages for deferral and keyed state:

| Option / Environment Variable | Description | Default |
|--------------------------------|-------------|---------|
| `loader_cache_size` / `PROSODY_LOADER_CACHE_SIZE` | Maximum messages retained by the shared Kafka loader | 1024 |
| `loader_seek_timeout` / `PROSODY_LOADER_SEEK_TIMEOUT` | Timeout for Kafka loader seek operations | 30s |
| `loader_discard_threshold` / `PROSODY_LOADER_DISCARD_THRESHOLD` | Sequential-read distance before the loader seeks | 100 |

## Monopolization Detection (Pipeline Mode)

| Option / Environment Variable           | Description                             | Default |
|-----------------------------------------|-----------------------------------------|---------|
| `monopolization_enabled` / `PROSODY_MONOPOLIZATION_ENABLED` | Enable hot key protection   | true    |
| `monopolization_threshold` / `PROSODY_MONOPOLIZATION_THRESHOLD` | Max handler time as fraction of window | 0.9 |
| `monopolization_window` / `PROSODY_MONOPOLIZATION_WINDOW` | Measurement window            | 5m      |
| `monopolization_cache_size` / `PROSODY_MONOPOLIZATION_CACHE_SIZE` | Max distinct keys to track  | 8192    |

## Fair Scheduling (All Modes)

| Option / Environment Variable           | Description                                                      | Default |
|-----------------------------------------|------------------------------------------------------------------|---------|
| `scheduler_failure_weight` / `PROSODY_SCHEDULER_FAILURE_WEIGHT` | Fraction of processing time reserved for retries | 0.3    |
| `scheduler_max_wait` / `PROSODY_SCHEDULER_MAX_WAIT` | Messages waiting this long get maximum priority          | 2m      |
| `scheduler_wait_weight` / `PROSODY_SCHEDULER_WAIT_WEIGHT` | Priority boost for waiting messages (higher = more aggressive) | 200.0 |
| `scheduler_cache_size` / `PROSODY_SCHEDULER_CACHE_SIZE` | Max distinct keys to track                             | 8192    |

## Telemetry

Prosody emits message, timer, and producer lifecycle events to a Kafka topic for observability:

| Option / Environment Variable           | Description                                    | Default                    |
|-----------------------------------------|------------------------------------------------|----------------------------|
| `telemetry_topic` / `PROSODY_TELEMETRY_TOPIC` | Kafka topic to produce telemetry events to | `prosody.telemetry-events` |
| `telemetry_enabled` / `PROSODY_TELEMETRY_ENABLED` | Enable or disable the telemetry emitter  | true                       |

Mock mode disables telemetry automatically, regardless of `telemetry_enabled`.

## Cassandra

Persistent storage for timers, deferral, deduplication, and keyed state. It is not needed when `mock: true`.

| Option / Environment Variable           | Description                        | Default |
|-----------------------------------------|------------------------------------|---------|
| `cassandra_nodes` / `PROSODY_CASSANDRA_NODES` | Servers to connect to (host:port) | -      |
| `cassandra_keyspace` / `PROSODY_CASSANDRA_KEYSPACE` | Keyspace name              | prosody |
| `cassandra_user` / `PROSODY_CASSANDRA_USER` | Username                          | -       |
| `cassandra_password` / `PROSODY_CASSANDRA_PASSWORD` | Password                   | -       |
| `cassandra_datacenter` / `PROSODY_CASSANDRA_DATACENTER` | Prefer this datacenter for queries | - |
| `cassandra_rack` / `PROSODY_CASSANDRA_RACK` | Prefer this rack for queries      | -       |
| `cassandra_retention` / `PROSODY_CASSANDRA_RETENTION` | Delete data older than this | 1y     |

## Keyed State

Register keyed-state collections before you subscribe. Persistence is backed by Cassandra and is not needed when `mock: true`. See [Keyed State](README.md#keyed-state) for handler usage. Where an option and an environment variable are paired, an explicitly set option wins. Otherwise, the environment variable applies, then the default.

| Option / Environment Variable | Description | Default |
|-------------------------------|-------------|---------|
| `state_collections` / - | Keyed-state collections to register before subscribe (array of definitions or config hashes; duplicate names rejected) | (none) |
| `subsystem` / `PROSODY_SUBSYSTEM` | Subsystem name used to advertise JSON collections whose definitions set `published: true` | (none) |
| `state_cache_dir` / `PROSODY_STATE_CACHE_DIR` | Disk workspace for the local keyed-state cache; each live client needs its own directory. Set a mounted path in production | per-client temp dir |
| `state_owned_cache_size` / `PROSODY_STATE_OWNED_CACHE_SIZE` | Capacity of the owning keyed-state cache; accepts sizes such as `64 MiB` or `500 MB` | storage-engine default |
| `state_read_cache_size` / `PROSODY_STATE_READ_CACHE_SIZE` | Capacity of the published-state read cache; accepts sizes such as `1 MiB` | `state_owned_cache_size` or `PROSODY_STATE_OWNED_CACHE_SIZE` when set; otherwise 1 MiB |
| `state_read_cache` / `PROSODY_STATE_READ_CACHE_TTL` | Default published-read cache TTL. Use `false` or the environment value `none` to bypass the cache | 5s |
| `state_recovery_delay` / `PROSODY_STATE_RECOVERY_DELAY` | Whole-second delay between staging a provisional cell and the recovery sweep; every collection TTL must strictly exceed it | 30s |

Prefer the definition constructors from the [API reference](README.md#api-reference). They serialize into `state_collections`, so you can reuse the same object with `context.state`. Each entry has these fields:

Published collections require `subsystem`. Keep it configured for one deployment after removing `published: true` so readers can observe the collection's retirement.

| Field | Description | Default |
|-------|-------------|---------|
| `name` | Collection name; non-empty and unique within the client | (required) |
| `kind` | `"value"`, `"map"`, or `"deque"` | (required) |
| `payload` | `"json"` (JSON values) or `"message"` (the full Kafka message the handler received) | (required) |
| `ttl_seconds` | Per-write TTL in whole seconds (at least 1; must exceed the recovery delay) | (none) |
| `read_uncommitted` | Opt out of transactional staging | false |
| `published` | Allow read-only access from other consumer groups; JSON collections only | false |
| `read_cache` | Published-read cache override: a positive duration, `false`, or inherit when omitted | inherit |
| `keyset_limit` | Map-only; ordered-scan bound in `0..=4096` (`0` disables ordered-scan tracking) | 128 |
| `capacity` | Deque-only window bound (at least 1); keeps at most N slots, enforced lazily on push. Runtime-only and mutable across deploys — not persisted | unbounded |

Constructors set these via keyword arguments (`ttl:`, `keyset_limit:`, `capacity:`, `read_uncommitted:`, `published:`, `read_cache:`). `read_cache` is a positive duration in seconds, `false` to bypass the cache, or `nil` to inherit the client default.
