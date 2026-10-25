# 3. FST Dictionary Indexing and Engine Pipeline Sequence

Date: 2026-10-24

## Status
Accepted

## Context
Writy requires instant, sub-millisecond retrieval of lexical entries across multiple StarDict dictionaries and offline ZIM archives without loading hundreds of megabytes of raw text into process heap memory. Additionally, typo matching and autocomplete require fuzzy and prefix searches.

## Decision
1. **Memory-Mapped FST Indexing**: Dictionaries are compiled upon import into memory-mapped Finite State Transducers (`fst` crate) stored in `~/.local/share/writy/dicts/*.fst`. The FST maps normalized terms to file slices `(offset, length)` in the corresponding `.dict` files.
2. **Zero-RAM Virtual Memory**: Operating system kernel page caching (`mmap`) handles dictionary pages on-demand with near-zero resident heap overhead.
3. **Automata-Powered Lookups**: Prefix lookups for word autocomplete and Levenshtein automata (edit distance ≤ 1) for fuzzy dictionary matching execute directly on the FST without scanning raw text.
4. **Engine Pipeline Sequence**: The implementation of `writy-core` executes in strict sequence:
   - Module 1: `morphology` (Arabic clitic peeling & normalization)
   - Module 2: `dict` (StarDict reader/indexer, OpenZIM reader, `LexicalCard` aggregator)
   - Module 3: `linter` (Harper engine integration)
   - Module 4: `engine` (Unified State Machine and `UiState`)

## Consequences
- Dictionary lookups take under 0.1 ms with zero cold-start latency.
- RAM usage remains under 30 MB regardless of dictionary size.
- Pre-compilation happens once per dictionary import.
