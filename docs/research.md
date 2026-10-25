# Research and evidence

Checked 2026-10-24. These sources explain feasibility and trade-offs; they do not
constitute runtime validation of Writy.

## Platform and stack

- [Hyprland plugin documentation](https://wiki.hypr.land/Plugins/Using-Plugins/)
  documents loading native plugins and matching them to the compositor build.
  Writy's bridge must be rebuilt when the supported Hyprland build changes.
- [Hyprland plugin development](https://wiki.hypr.land/Plugins/Development/Getting-started/)
  is the source for the bridge API. Compiler success alone does not establish
  that interception, focus changes, or text replacement work in a live session.
- [GTK4 layer shell](https://github.com/wmww/gtk4-layer-shell) supplies GTK
  integration for layer-shell surfaces. This is the Linux HUD implementation,
  not a portable window-placement guarantee.
- [Layer-shell protocol source](https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-layer-shell-unstable-v1.xml)
  defines layer roles, anchoring, and keyboard interactivity. It does not expose
  another client's text selection or caret.
- [AT-SPI documentation](https://gnome.pages.gitlab.gnome.org/at-spi2-core/)
  describes the accessibility interfaces used for field and text information.
  Actual coverage must be checked separately in every supported application.
  [Process identity](https://gnome.pages.gitlab.gnome.org/at-spi2-core/libatspi/method.Accessible.get_process_id.html),
  [caret offsets](https://gnome.pages.gitlab.gnome.org/at-spi2-core/libatspi/method.Text.get_caret_offset.html),
  and [single-operation text replacement](https://gnome.pages.gitlab.gnome.org/at-spi2-core/libatspi/method.EditableText.set_text_contents.html)
  underpin the broker. AT-SPI does not provide an atomic compare-and-set between
  reading and replacing application text; a final concurrent-edit race remains.
- [SQLite use cases](https://www.sqlite.org/whentouse.html) include local desktop
  application storage. A single local dictionary database is a practical first
  choice; FST indexing remains an optimization to evaluate with measurements.

## Language services and data

- [Harper](https://github.com/Automattic/harper) supplies English proofreading.
  It does not establish an Arabic grammar-correction capability for Writy.
- [Ollama API](https://docs.ollama.com/api/introduction) supports locally hosted
  generation; model installation and resource requirements are separate from
  installing Writy.
- [OpenAI-compatible chat request format](https://platform.openai.com/docs/api-reference/chat/create)
  describes the HTTP shape supported by compatible providers. Compatibility is
  provider-specific; Writy does not promise every endpoint implements every field.
- [Kaikki dictionary extracts](https://kaikki.org/dictionary/) provide structured
  Wiktionary data and attribution information. Coverage varies by language and
  entry; six complete lexical categories cannot be assumed for every word.
- [Wiktionary reuse conditions](https://en.wiktionary.org/wiki/Wiktionary:Copyrights)
  must accompany any distributed extract. The application license does not
  replace the license of dictionaries, media, or models.

## Claims requiring measurements

Record the hardware, compositor version, corpus size, provider/model, warm or
cold cache, sample count, and p50/p95 when measuring lookup latency, startup,
memory, or generation latency. Track the daemon, bridge, child processes, and
model service separately. There is no verified guarantee of zero latency,
universal app support, constant RAM, or useful AI suggestions in this repository.

The 50 cases in `tests/fixtures/evaluation.json` are an evaluation script for a
human reviewer. Populate ratings only after running the case in the named
application and assessing the displayed result. Automated fixture checks do not
count as a usefulness study.

Insert-mode lexical enrichment uses [Ollama structured outputs](https://docs.ollama.com/capabilities/structured-outputs)
with an explicit JSON schema and validates the result before use. A valid
structure does not establish lexical correctness: the installed 4B model can
confuse parts of speech and invent derived words. Keep generated content visibly
labeled and evaluate it in the human trial. Rewriting remains a separate request
after a combined rewrite/lexical benchmark changed the intended meaning.
