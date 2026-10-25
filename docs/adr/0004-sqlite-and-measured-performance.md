# 4. SQLite first and measured performance

Date: 2026-10-24

## Status
Accepted. Supersedes ADR 0003's mandatory FST design, implementation sequence,
and unconditional performance/memory guarantees.

## Context

The first implementation needs structured multilingual entries, source/license
metadata, repeatable imports, and exact/prefix lookup. No representative corpus
or benchmark currently demonstrates that an FST is required. The previous ADR
claimed zero cold-start latency, dictionary-size-independent RAM, and 0.1 ms
lookups without measurement. Those claims are not an acceptance contract.

## Decision

Use SQLite as the initial local dictionary store through the Rust core. Enable
write-ahead logging so existing readers see committed entries during imports;
index alias foreign keys so replacing a source does not repeatedly scan the
alias table. Bound lock waits to 250 ms rather than blocking desktop services
for five seconds. This was added after a real repeated-pack import exposed
reader stalls on 2026-10-25. Keep
lookup and import behind the dictionary module so an additional index can be
introduced without changing result types or the Linux UI.

Validate the Linux input/focus/edit path early and deliver reading and writing
as vertical slices. Morphology→dictionary→linter→engine is not a required project
sequence. Preserve Arabic surface spelling; normalization assists lookup and
must not silently rewrite the user's text.

Treat lookup latency, startup, and memory as measured properties. Publish the
hardware, corpus, process boundary, cache condition, sample size, and p50/p95.
Choose a specialized prefix/fuzzy index only when a representative benchmark
shows the current approach misses an agreed target.

## Alternatives considered

- **FST immediately:** compact static lookup structure, but adds an index build,
  metadata/slice coordination, and another representation before a demonstrated
  bottleneck. Revisit after measurements.
- **Raw files only:** simple for a single source; less convenient for merging
  structured senses, attribution, and transactional imports.

## Consequences

SQLite adds a local database dependency and migrations require care. It reduces
the number of custom storage mechanisms in the first release. Neither the
choice of SQLite nor memory mapping guarantees a particular total RSS or cold
lookup time. ADR 0003 is retained unchanged as historical context.
