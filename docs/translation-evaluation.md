# Offline English–Arabic translator spot check

Checked 2026-10-25 on this machine: Quadro T2000 4 GB VRAM, 31 GiB RAM.
This is a four-passage spot check, not a translation-quality benchmark or a
statistically stable speed comparison. Shared machine load may affect timing.
No Writy provider settings were changed.

| Engine | Configuration | Observed request time | Finding |
| --- | --- | --- | --- |
| TranslateGemma 4B | Existing Ollama `translategemma:4b` Q4_K_M, 2048 context, temperature 0.2, output budget 384 tokens; Writy-style single-user translation prompt | First request 11.90 s including 10.38 s model load; subsequent requests 1.08–3.93 s | All four outputs used the target language and included the passage's sentences. Arabic “لم أوافق” became “I disagree” rather than “I did not approve”: meaning/tense drift. Screenshot passage was translated into Arabic, with some awkward phrasing. |
| Argos Translate | 1.11.0, CTranslate2 4.8.3, CPU; official direct en→ar and ar→en package versions 1.0; MiniSBD sentence splitter | 0.09–0.22 s for these short requests, excluding Python startup/import | English→Arabic was understandable, but quote entities leaked as `&quot;`. Both Arabic→English passages lost an entire sentence. |

The Argos omissions also occurred with the default Stanza splitter, so they were
not specific to MiniSBD. For example, translating
“لم أوافق على التغييرات. يرجى الانتظار حتى الغد.” returned only
“Please wait till tomorrow.” This drops the non-approval statement entirely.

Argos dependencies were installed in an isolated Python 3.12 environment at
`~/.local/share/writy/translation-eval/venv`; CPU PyTorch was installed separately
to avoid bringing in CUDA runtime packages. Both official translation packs were
installed, followed by the MiniSBD sentence models. Translation succeeded with
Python socket connections blocked during all four evaluation calls. This is a
process-level check of Python network calls, not an OS network-namespace test.
The default Stanza configuration attempted metadata networking even with models
present; MiniSBD was preloaded and selected with `ARGOS_CHUNK_TYPE=MINISBD` for
the offline run.

Raw inputs, outputs and timings:

- [TranslateGemma](translation-evaluation/translategemma.json)
- [Argos with preloaded MiniSBD and Python networking blocked](translation-evaluation/argos.json)
- [Argos Arabic recheck with default Stanza](translation-evaluation/argos-stanza.json)

## Decision

TranslateGemma is the stronger candidate in this small comparison for Writy's
selection translation. Its observed semantic drift still needs user review.
Argos is a promising fast candidate, but its demonstrated sentence omissions
must be resolved before making it the default. Neither this sample nor a target-
script check establishes semantic correctness. Keep dictionary lookup and model
translation separately attributed.

## Official references

- [Argos installation, package API and offline purpose](https://github.com/argosopentech/argos-translate)
- [Official Argos package index](https://github.com/argosopentech/argospm-index)
- [TranslateGemma Ollama prompt format and model](https://ollama.com/library/translategemma:4b)
