# CLAUDE.md

Development patterns and practices for prosody-rb: Ruby bindings for the
Prosody Kafka client library. A magnus/rb-sys extension (`ext/prosody`) wraps
the published `prosody` Rust crate; the Ruby library (`lib/prosody`) carries
the public API, with RBS signatures in `sig/`. `ARCHITECTURE.md` explains the
Ruby/Rust bridge in depth — read it before touching the extension.

## Design Principles

These come before everything else. Every change is judged against them.

**Write code that is simple, clear, well-factored, elegant, easy to
understand, correct, and idiomatic.** A reader should grasp the intent without
effort. If a change makes the code harder to read, the change is wrong, even
if it is faster or shorter. If two designs are correct, pick the one that is
easier to delete.

**Make invalid states unrepresentable in the type system.** When a compiler
or type checker can prove a contract, no test, comment, or convention has to.
In Rust, prefer distinct types for distinct concepts, restricted constructors,
and `enum` sum types over flag fields. In the RBS signatures, give the public
surface precise types instead of loose ones. If a bug class can be made
uncompilable, do that instead of writing a runtime check.

**Delete more than you add.** Every change should leave the codebase smaller,
simpler, or both. If you must add code, look first for duplication you can
fold, abstractions that no longer pay rent, dead branches, and stale comments.
The end-state diff should net negative whenever the task allows. Line count is
not the only axis: plain duplicated arms often read better than generic
machinery.

**Identify, document, and enforce invariants.** For every load-bearing piece
of state: name the invariant, write it down near the type or function that
owns it, enforce it in the type system if you can, otherwise assert it at the
boundary, and cover it with a test. If you cannot name the invariant, you do
not yet understand the code well enough to change it.

**Leave the codebase better than you found it.** Drive-by simplifications are
encouraged when they are scoped to the area you are already touching. Do not
sprawl — but do not walk past obvious cleanup either.

## Definition of Done

No change is complete until every line below holds. These are acts, not
aspirations — perform each one; do not merely agree with it:

1. `cargo clippy --manifest-path ext/prosody/Cargo.toml` and the same with
   `--tests` — zero warnings. `cargo doc` — zero warnings. `make format-check`
   passes (rustfmt, taplo, standardrb).
2. `bundle exec rake rbs` and `bundle exec rake steep` — zero errors. They
   validate the signatures and type-check the library, the typed examples,
   and the negative fixtures.
3. `make test 2>&1 | tee /tmp/rspec-output.txt` — re-running slow suites is
   expensive; grep the file, not the pipe.
4. Every new or converted test was proved falsifiable once: inject the
   failure, watch it go red, revert.
5. Every deleted test names its surviving stronger test in the commit message.
6. Everything the change replaces is gone — code, tests, signatures, doc
   vocabulary (see Redesign hygiene). "The new thing works" is half done.
7. Every claim written this session — doc cross-reference, "covered by" note,
   exemplar path — was verified to resolve, not recalled from memory.
8. The diff is net-negative, or each addition is individually justified.

## Development Setup

Start required services with:

```bash
docker-compose up -d
```

Services:

- Kafka: localhost:9094
- Cassandra: localhost:9042

Common commands (`make help` lists them):

```bash
make compile-dev   # Compile the Rust extension (development mode)
make compile       # Compile the Rust extension (release mode)
make test          # Run the RSpec suite (needs Kafka and Cassandra)
make test-tracing  # Run the tracing suite against a local OTel collector
make format        # Format Rust, TOML, and Ruby code
make format-check  # Check formatting without modifying files
```

`rake` runs the full default gate: `compile spec standard rbs steep`.
Recompile with `make compile-dev` after any Rust change, or the specs run
against stale native code.

## Critical Rules

**Error Handling (Rust):**

- Never use `expect`, `unwrap`, `panic`, or `ok()` - forbidden by lints
- Propagate errors with `?` unless explicitly authorized to swallow
- Use `thiserror` for structured errors; box only when Clippy warns

**Memory (Rust):**

- **Never leak memory.** `std::mem::forget`, `Box::leak`, and `ManuallyDrop`
  without an explicit reclamation path are forbidden. If a test must simulate
  "Drop never ran", seed the underlying state directly; forgetting is never
  the shortcut.
- **No unbounded keyed RAM.** Any in-memory structure keyed by message key or
  collection must have a fixed capacity bound. Every in-memory map names its
  removal path; self-draining maps are fine, but the drain is still named.

**Allocation (tiger style — https://tigerstyle.dev/):**

- No hot-path allocation that is not upfront and bounded. A steady-state path
  (per message, per timer fire, per handler call) must not allocate a buffer
  whose size is discovered at runtime and grown as needed.
- Pick the buffer by what is known about the size: compile-time constant →
  stack array; runtime-varying but almost always small → `SmallVec` sized to
  the common case; genuinely unbounded → `Vec::with_capacity` sized once.
- `with_capacity` excuses the sizing, never the allocation. A per-call heap
  allocation on a steady-state path is the defect itself.
- Never add a gratuitous allocation to satisfy the borrow checker. Reach for
  a function item, an index, or a borrow before a scratch `Vec`.
- No amortized resize buffers on the hot path. If a reusable scratch buffer
  is unavoidable, allocate it once at construction with a fixed bound.
- Simplicity is not sacrificed for this. When zero-alloc and simple genuinely
  conflict, keep it simple and leave a comment naming the allocation.

**Code Quality:**

- Lint, doc, type, and format gates live in Definition of Done — zero
  warnings tolerated.
- Never suppress warnings with `#[allow(...)]` without permission. All clippy
  and rustc warnings are fixed properly, not suppressed.
- Never introduce `dyn` without permission — prefer generics and associated
  types. The type-erased surface this binding consumes already lives in the
  published `prosody` crate.

**Ruby/Rust boundary:**

- Rust cannot call Ruby methods from Rust-created threads. Only Ruby-created
  threads run Ruby code safely. `ARCHITECTURE.md` documents the bridge that
  upholds this; never bypass it.
- Release the GVL for blocking work in the extension so other Ruby threads
  can run.

**JSON codec:**

- This binding never defines its own payload codec. Payload encoding and
  decoding belong to the `prosody` crate's codec; the binding passes payload
  bytes through it.
- `serde_json`, `simd_json`, and the `json!` macro are banned in Rust
  production code here for payload handling. Tests may use `serde_json::Value`
  as a concrete payload type.

**Redesign hygiene:**

When a design is replaced, remove *all* of it in the same change —
half-deleted designs are where bloat and bug re-introduction live:

- Sweep the old design's vocabulary from every doc comment, signature, and
  example. A stale doc can instruct a reader to re-introduce a fixed bug.
- Code whose only caller is its own test is dead — delete both together.
- Struct fields threaded through configs but only read at construction are
  residue from a superseded design — remove them end-to-end.
- Do not build surface ahead of a caller: delete zero-caller paths, or make
  them owner-confirmed, tested features.

**Debugging Discipline:**

- Never claim "found the issue" without rigorous proof
- Evidence first (logs, tests, reproducible behavior) → hypothesis → test → verify

**Documentation:**

- **All code comments, technical documentation, READMEs, plans, commit
  messages, and PR descriptions must conform to ASD-STE100 (Simplified
  Technical English).** Apply these primary STE rules in everything you
  write for this project:
  - Use the active voice. Write instructions in the imperative.
  - Write short sentences: 20 words or fewer for instructions, 25 words or
    fewer for descriptions.
  - Write one instruction per sentence. Keep one topic per paragraph, with a
    maximum of six sentences.
  - Use a word with only one meaning, and use the same word for the same
    thing everywhere.
  - Use simple verb tenses. Do not use an "-ing" form as a verb when a simple
    tense is correct.
  - Do not make a noun cluster of more than three nouns.
  - Technical names and technical verbs from this project's domain are
    permitted. Use them consistently.
- Write doc comments for a reader unfamiliar with the codebase. Lead with
  what the thing is, how to use it, and what guarantee it gives — not the
  internal mechanism.
- Short declarative sentences, one idea each. At most one parenthetical aside
  per comment, never nested.
- Never argue with an imagined reviewer. State what the code does and the
  invariant it upholds. Mention a rejected alternative only when a maintainer
  would plausibly reintroduce it, as its own plain sentence.
- No invented compound jargon. Spell the idea out in ordinary words;
  established terms keep their standard form.
- State an invariant at the type or function that owns it, once. Reference
  the owning type elsewhere instead of restating.
- Be concise. Bad or needless docs hurt readability — prefer fewer, sharper
  words.
- Never cite a plan's or spec's section number, phase number, or ordinal in
  durable docs — code comments, CLAUDE.md, PR and commit text. Name the
  concept instead.
- Avoid vague metaphor filler in prose, comments, and commit/PR text ("north
  star", "surface area", "lean into", "double-click", "first-class citizen").
  Say the concrete thing instead.

**Style:**

- Prefer `use` statements over fully qualified prefixes
- Methods without `self` should be functions (except `new` and similar)
- Ask before large structural changes
- Default to `pub(crate)`/`pub(super)` in Rust; make something `pub` only as
  a deliberate API decision.
- Keep trait constraints as local as possible: put a constraint on the
  function that needs it, not the struct.
- When a proposed simplification is examined and rejected, record the ruling
  in one sentence at the site so the next pass does not re-litigate it.
- Ruby code follows standardrb; the formatter is the arbiter.

**Git:**

- Never add self-attribution to branch names, commits, PR titles, PR
  descriptions, or code comments.
- Use conventional commits for commit titles and PR titles (e.g., `fix:`,
  `feat:`, `docs:`, `refactor:`).
- PR titles and descriptions are written for a reader who is not intimately
  familiar with the project. Lead with what changed and why.
- Never hard-wrap paragraphs in GitHub PR descriptions, PR comments, or issue
  text. Each prose paragraph is one single line; blank lines separate
  paragraphs.
- PR descriptions never include a test plan or a list of verification steps.
- Do not reference internal phase numbers, task IDs, or spec sections in
  commits or code comments.
- Never run `git reset` or `git checkout` that would destroy uncommitted or
  committed changes without explicit human permission. Prefer `git stash`, an
  explicit commit, or `git restore --staged <path>`.
- Use `gh` for GitHub operations (PRs, issues, API).

## Error Classification

Distinguish permanent from transient errors for retry logic:

```rust
#[derive(Debug, Clone, Copy)]
pub enum ErrorType {
    Permanent,  // Business logic - don't retry
    Transient,  // Network/timeout - retry with backoff
}
```

A permanent error discards the in-flight message. An error the caller's code
causes (bad input, wrong argument shape) classifies as transient unless the
caller explicitly declares it permanent — a transient error retries and stays
visible, so no message is silently lost.

## Concurrency Invariants (inherited from prosody)

- **One handler per key, system-wide.** The framework guarantees at most one
  message or timer handler for a given key executes anywhere in the cluster
  at any moment. Never design for concurrent writers on the same key — that
  scenario cannot occur.
- **At most one partition owner.** Kafka partition assignment guarantees one
  consumer group member owns each partition at a time.
- These invariants are why distributed locks and optimistic concurrency are
  never needed for per-key state. The framework provides the exclusivity;
  binding code and examples can assume it.

## Code Organization

**Maximum file size: 500 lines.** A file that exceeds it is subdivided into
modules. Split along a seam the code already has, and give each module a doc
comment naming what it owns. Re-export from the parent so the split is
invisible to callers. A split that only balances line counts is worse than
the long file; find the real seam.

**Prefer one-word module names** (`bridge`, `client`, `handler`,
`scheduler`). A two-word name usually means the module owns more than one
concern, or the name restates its parent's path. A compound name is right
only when the compound is the domain term.

**Order within Rust files (topological by dependencies):**
Constants → Statics → Types → Implementations → Functions → Errors (bottom)

## Types, Signatures, and Examples

- `sig/` — public RBS signatures; `sig-private/` — internal signatures.
  Every API change updates the signatures in the same commit.
- `lib/prosody/native_stubs.rb` — documented Ruby stubs for the classes and
  methods the Rust extension implements. Editors and documentation tools read
  them; the runtime does not (the native extension provides the real
  definitions), and Steep ignores the file. When the extension's public
  surface changes, update the matching stub and its YARD doc in the same
  commit.
- Steep targets (`Steepfile`): `lib` checks the implementation;
  `consumer_types` (`typecheck/`) verifies a payload type flows through the
  public API; `typed_examples` checks every runnable example;
  `negative_types` (`typecheck_negative/`) holds deliberately-invalid calls
  with committed expectations — the gate fails if an expected diagnostic
  disappears.
- `examples/*.rb` each carry a sibling `.rbs` signature and are type-checked
  by the `typed_examples` target.
- Run `bundle exec rake rbs` to validate signatures and
  `bundle exec rake steep` to type-check.

## Testing

Test suites live in `spec/` (RSpec). Integration specs need the Docker
Compose services. Run with `make test`; write output to a file:

```bash
make test 2>&1 | tee /tmp/rspec-output.txt
grep -i fail /tmp/rspec-output.txt
```

**Test principles:**

- Drive tests by invariants, not by paths. Name the invariant (round-trip,
  parity, idempotence) and prefer few broad tests over many narrow example
  tests. Use realistic inputs, not happy-path toys.
- A test must be able to fail. When you write or convert a test, prove it can
  go red once: inject the failure, watch it fail, revert.
- Never delete a test without naming, in the commit, the surviving test that
  covers the same invariant at least as strongly.
- Never use `sleep` except for backpressure simulation. Wait on events,
  channels, or notifications with a deadline — the deadline is a hang-guard,
  never the assertion.
- Root-cause every intermittent failure. A passing re-run proves nothing.
  Extract the reproducer and land it as a deterministic regression test.
- In Rust tests, use `assert` or a `Result` with `?` — never
  `expect`/`unwrap`, never swallow errors.

## Common Patterns (Rust)

- Use `parking_lot` over `std::sync`
- For concurrent hash sets/maps, use `scc` (`scc::HashSet` / `scc::HashMap`),
  never a `Mutex<HashSet>` / `Mutex<HashMap>`; pair it with
  `ahash::RandomState`. In async code prefer its async interface.
- Use `tokio::sync` primitives (`Notify`, channels, `select!`) for async
- Independent I/O runs concurrently, never serially. Drive N independent
  reads through a bounded `buffered(N)` (order-preserving) or
  `buffer_unordered(N)` (unordered). Reserve serial `await` for genuinely
  dependent reads, where each result determines the next.
- Drive futures over non-tokio primitives through the cooperative budget:
  wrap each per-item future with `tokio::task::coop::cooperative` inside the
  producing closure, so a drain of ready items cannot starve the worker.
- Mark builders with `#[must_use]`
- Use `LazyLock` for expensive static initialization

## Tracing / OpenTelemetry

- Instrument with `#[instrument]`, never a hand-built `info_span!` +
  `.instrument(...)`. Use `skip_all` plus explicit `fields`, and `err` to
  record failures on the span.
- Span level is audience: spans the user's own code causes export at info;
  framework-internal spans use `level = "debug"`.
- Record unsigned integers as `i64` — the OTel layer stringifies
  `u64`/`usize`. Record attribute values with `%` (Display) where the type
  allows.
- Import tracing macros from `tracing` directly — never `use tracing::log::…`;
  no bridge is installed, so events logged through it silently vanish.
- Never cache a `Span` — cache an `opentelemetry::Context` and recreate spans
  on read. Cloning a span creates another reference to the same underlying
  span; finishing one finishes all.

## Workflows

When launching multi-agent workflows:

- Select model and effort per task by complexity — do not let every agent
  inherit the session model. Never downgrade a stage whose output gates a
  commit or ship decision.
- Disable the advisor in every agent prompt.
- Keep structured-output schemata trivially simple: flat objects with a few
  short bounded fields; put detail in report files.

## Research

- Automatically use context7 for code generation and library documentation.
