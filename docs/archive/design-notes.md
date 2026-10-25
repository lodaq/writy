# Writy

A tiny, fast, open-source writing assistant, pop-up dictionary, and bidirectional translator for Linux (Arch + Hyprland), designed with a modular architecture for future Windows and macOS support.

A system-wide, offline alternative to **Grammarly**, **Google Translate**, and **Ejoy** that works in every application, browser, terminal, and text editor.

- **Bidirectional English ↔ Arabic translation** (selection-based).
- **English writing assistant:** Built-in [Harper](https://github.com/Automattic/harper) engine for instant grammar & spelling, plus local [Ollama](https://ollama.ai) models for rephrasing and sentence translation.
- **Offline & Private:** Local dictionaries, memory-mapped indices, local LLMs. Zero telemetry. Typing buffer stays in volatile memory and never touches disk.
- **Rich dictionary entries:** Native formatting, pronunciations, audio playback, and local images—rendered without a heavy web engine.
- **Dual popup system:** Lightweight, non-focus-stealing layer-shell popups (Typing on the left, Translation/Dictionary on the right).

> **Status:** Active design and initial implementation phase.

---

## Table of Contents

- [Features Overview](#features-overview)
- [The Dual Popup](#the-dual-popup)
  - [Visual Design & Palette](#visual-design--palette)
  - [Rich Content Rendering](#rich-content-rendering)
- [How It Works](#how-it-works)
  - [Typing Mode (English)](#1-typing-mode-english-only)
  - [Selection Mode (English ↔ Arabic)](#2-selection-mode-bidirectional)
  - [Rephrasing & Model Integration](#3-rephrasing--local-model)
- [Input & Caret Tracking](#input--caret-tracking)
- [Dictionaries & Knowledge Bases](#dictionaries--knowledge-bases)
- [Configuration](#configuration)
- [System Architecture](#system-architecture)
- [Setup & Installation (Arch + Hyprland)](#setup--installation-arch--hyprland)
- [Keybindings](#keybindings)
- [Roadmap](#roadmap)
- [License](#license)

---

## App Description

Writy provides intelligent writing assistance, popup dictionaries, and bidirectional translation powered by both local/cloud dictionaries/tools and local/cloud AI models in specific modes (**Insert Mode**, **Select Mode**, **Search Mode**) for specific inputs (**Words**, **Sentences**) in **English** and **Arabic**.

---

## Modes & Input Types

| Mode | Trigger | Input Type | Scope & Popups Displayed |
|---|---|---|---|
| **Select Mode** | Highlighting text across any application (`wl-paste`) | **Word** | **Full Lexical Card:** Translation (AR ↔ EN), Definition, Examples, Collocations, Synonyms & Antonyms, Word Family. |
| **Select Mode** | Highlighting text across any application (`wl-paste`) | **Sentence** | **Sentence Translation:** High-fidelity bidirectional translation (AR ↔ EN) via local/cloud model. |
| **Search Mode** | Pressing **`Alt+G`** anywhere (focused modal) | **Word** | **Full Lexical Card:** Instant live lookup for English or Arabic words with all 6 lexical categories. |
| **Search Mode** | Pressing **`Alt+G`** anywhere (focused modal) | **Sentence** | **Sentence Translation & Rephrases:** Live translation and alternative phrasings. |
| **Insert Mode** | Typing in any text editor / app (English) | **Word & Sentence** | **Writing Assistant:** Autocomplete, instant Harper spelling & grammar checks, model rephrases & suggestions. |
| **Insert Mode** | Typing in any text editor / app (Arabic) | **Word & Sentence** | **Arabic Writing & Lexicon:** Word autocomplete, spelling & grammar checks, suggestions, and instant access to translation, definition, examples, collocations, synonyms/antonyms, and word families. |

---

## Popup Categories

When inspecting words or typing sentences, Writy categorizes lexical data into 8 semantic modules:

| Category | Badge | Description & Examples | Color in UI |
|---|---|---|---|
| **Translation** | `trans` / `EN→AR` | Bidirectional direct lexical or sentence translation. | Mint green `#7fe3b0` |
| **Definition** | `def` | Accurate, concise definitions from top learner dictionaries. | Soft white `#e8d6dc` |
| **Examples** | `ex` | Real-world contextual sentences showing how the word is used. | Muted italic `#a9b1d6` |
| **Collocation** | `coll` | Essential word pairings (e.g. *make a decision*, *heavy rain*, *عقد قران*, *قرار حاسم*). | Cyan `#7dcfff` |
| **Synonyms & Antonyms** | `syn` / `ant` | Closely related words and exact opposites (e.g. *syn: rapid, swift* \| *ant: sluggish*). | Yellow `#f5c76b` / Red `#f7768e` |
| **Word Family** | `fam` | Complete morphological derivation tree (e.g. *decide [v], decision [n], decisive [adj], decisively [adv]* \| *كتب، كاتب، مكتوب، مكتبة*). | Purple `#bb9af7` |
| **Suggestions & Rephrases** | `fix` / `alt` | Rule-based grammar fixes and AI rewrites (clearer, concise, formal). | Yellow `#f5c76b` / Cyan `#7dcfff` |
| **Slang & Idioms** | `slang` | Informal expressions and colloquial meanings *(to be added)*. | Orange `#ff9e64` |

---

## Supported Dictionaries & Formats

Writy is designed to load and parse the world's most renowned learner and bilingual reference dictionaries:

- **Cambridge Advanced Learner's Dictionary** (renowned for clear collocations & definitions)
- **Oxford Advanced Learner's Dictionary** (gold standard for examples and word families)
- **Longman Dictionary of Contemporary English (LDOCE)** (unrivaled collocation & spoken grammar corpus)
- **Macmillan English Dictionary** (core vocabulary frequency and metaphorical usage)
- **Merriam-Webster** (American English definitions, etymology, and synonyms)
- **Arabic Lexicons:** Arabic Wiktionary, Al-Mawrid, Lisan al-Arab, and bilingual StarDict/MDict resources.
- **Encyclopedic Knowledge:** Offline Wikipedia & Wiktionary packages (**OpenZIM `.zim`** format).

Supported package formats: **StarDict** (`.ifo`, `.idx`, `.dict`, `.syn`, `res/`), **MDict** (`.mdx`, `.mdd`), **OpenZIM** (`.zim`), and plain TSV wordlists.

---

## Design System: Apple-Inspired Fluid Interfaces

Writy's visual design is rebuilt from the ground up on Apple's **Human Interface Guidelines (HIG)** and WWDC principles of fluid, physical interfaces: **Materials, Vibrancy, Optical Typography, and Fluid Motion**.

```
┌──────────────────────────────────────┐   ┌──────────────────────────────────────────┐
│  [Typing Assistant HUD]              │   │  [Lexical & Translation HUD]             │
│  EN  2 issues                        │   │  EN → AR  decision  ▶  /dɪˈsɪʒn/         │
│  ⇥ Tab    recieve → receive          │   │  1  قرار  qarār                    Oxfd  │
│  ⌥ 1      I received the email.      │   │     n. a choice made after thinking      │
│  ⌥ 2      I got the email.           │   │     ex  she made a tough decision        │
│  ⌥ 3      The email reached me.      │   │  coll  make/reach a decision, tough      │
│  syn      sound, solid, reliable     │   │  syn   choice, verdict  |  ant  indecision│
│  ⌥ 4      Add "recieve" to Lexicon   │   │  fam   decide (v), decisive (adj)        │
└──────────────────────────────────────┘   └──────────────────────────────────────────┘
```

### 1. Materials, Depth & Vibrancy
- **Liquid Glass Materials:** Floating surfaces use deep translucent backdrop blurs (`backdrop-filter: blur(28px) saturate(190%)`) that adapt organically over any desktop wallpaper or window background without stealing focus.
- **Continuous Superellipse (Squircle):** All corners feature Apple's signature smooth continuous curvature (`border-radius: 18px`).
- **Inner Light Rim:** A subtle 1px highlight stroke (`rgba(255, 255, 255, 0.12)`) captures imaginary ambient light along top edges, creating depth without heavy borders.
- **Layered Ambient Shadows:** Multi-stop soft occlusion drop-shadows provide tactile elevation off the desktop plane.

### 2. Apple HIG Semantic Color Palette

| Token | Dark Glass | Light Glass | Semantic Purpose |
|---|---|---|---|
| **Surface** | `rgba(28, 28, 30, 0.75)` | `rgba(255, 255, 255, 0.80)` | Dynamic translucent glass background with 28px blur |
| **Primary Label** | `rgba(255, 255, 255, 0.95)` | `rgba(0, 0, 0, 0.88)` | Headwords, primary definitions, and translations |
| **Secondary Label**| `rgba(255, 255, 255, 0.65)` | `rgba(0, 0, 0, 0.55)` | Descriptions, source dictionaries, and phonetic guides |
| **Tertiary Label** | `rgba(255, 255, 255, 0.40)` | `rgba(0, 0, 0, 0.35)` | Usage notes, metadata, and keyboard hint labels |
| **System Green** | `#30D158` | `#34C759` | Arabic/English translations, grammar approval, diff additions |
| **System Blue** | `#0A84FF` | `#007AFF` | Primary interactive controls, selection focus, audio playback |
| **System Cyan** | `#64D2FF` | `#00C7BE` | Collocations (`coll`), model badges, phonetics |
| **System Purple** | `#BF5AF2` | `#AF52DE` | Word family derivations (`fam`), morphological trees |
| **System Gold** | `#FFD60A` | `#FF9F0A` | Key caps (`⇥ Tab`, `⌥ 1`), synonyms (`syn`) |
| **System Red** | `#FF453A` | `#FF3B30` | Spelling errors, critical grammar violations, antonyms (`ant`) |

### 3. Typography & Micro-Interactions
- **Optical Tracking:** Headers use tight letter-spacing (`-0.015em`), while body definitions use comfortable leading (`1.45`) for maximum legibility.
- **Bilingual Typographic Harmony:** Native integration of **SF Pro** for Latin text and **SF Arabic** / **Noto Sans Arabic** for natural, beautifully joined Arabic calligraphy.
- **Apple Keycaps:** Keyboard shortcuts are rendered as tactile rounded capsules (`⇥ Tab`, `⇧⇥`, `⌥ 1`, `⌥ G`) with subtle bevels.
- **Physical Press Feedback:** Interactive rows respond immediately on pointer-down with an instant tactile scale (`active:scale-[0.97]`).
- **Spring Physics:** Dismissals and expansions follow critically damped springs (`damping: 1.0, response: 0.35s`) for zero latency and natural settling.

### Rich Content Rendering

Dictionary entries are not dumbed down to plain text:

- **Typography:** Retains bold, italics, underlines, sub/superscript, bullet lists, and line breaks.
- **Color Mapping:** Dictionary-embedded CSS colors are normalized to the Writy palette (e.g., parts of speech in mint green, examples in italics).
- **Local Images:** Rendered from dictionary resource folders (`res/` or ZIM blobs), scaled up to a maximum height of 120 px. No web requests are ever made.
- **Audio Pronunciation:** Clicking `▶` plays native dictionary audio files via `mpv` (or fallback TTS).
- **Diff Highlighting:** In `fix` and `alt` rephrasing suggestions, changed or inserted words are subtly highlighted in green.
- **Native Engine:** Parsed into a clean Pango markup subset and GTK `Picture` widgets. **No WebKit or Chromium engine is used**, keeping memory under tens of megabytes and startup instantaneous.

---

## How It Works

### 1. Insert Mode (Typing in English & Arabic)

- **English Typing:**
  1. The typing popup opens upon the first typed character.
  2. In-memory buffer captures the active sentence.
  3. [Harper](https://github.com/Automattic/harper) inspects the sentence synchronously (<1 ms) for spelling errors and rule-based grammar issues.
  4. Suggestions appear with quick-selection keys (`Tab` for top typo fix, `Shift+Tab` for style improvements, `Alt+1`..`Alt+9` for alternatives).
  5. After an idle pause (200 ms), local/cloud models generate natural rephrasings.

- **Arabic Typing:**
  1. Captures the Arabic typing stream and provides word autocomplete and spellchecking (single-letter edits like Hamzas `أ/إ/آ`, Ta Marbuta `ة/ه`, and Alif Maqsura `ى/ي`).
  2. Offers contextual suggestions and rephrases via local/cloud models and programs.
  3. Provides instant popup access to definitions, examples, collocations, synonyms/antonyms, and word families for typed Arabic words.

### 2. Select Mode (Bidirectional AR ↔ EN)

1. When text is highlighted anywhere (detected via Wayland primary selection `wl-paste --primary --watch`), the popup opens immediately.
2. **Word Selection:** Displays the complete **Lexical Card**:
   - Direct bilingual translation (AR ↔ EN).
   - Core definitions from Cambridge / Oxford / Wiktionary.
   - Authentic contextual examples.
   - Essential collocations (e.g. *heavy rain*, *make a decision*).
   - Synonyms and Antonyms (`syn` / `ant`).
   - Word Family derivation tree (`fam`).
   - 2-line Wikipedia encyclopedic summary.
3. **Sentence Selection:** Sends the sentence to the local/cloud model to generate a high-quality translation between Arabic and English, plus stylistic rephrase variants.
4. **Auto-dismiss:** Stays open without a timer while text remains highlighted, and closes immediately when the selection is cleared.

### 3. Search Mode (`Alt+G`)

Press **`Alt+G`** anywhere to summon the focused interactive modal:
1. Type any word or sentence in either **English** or **Arabic** (auto-detected by script).
2. **Word Input:** Instantly renders the 6-module Lexical Card (translation, definition, examples, collocations, synonyms/antonyms, word family).
3. **Sentence Input:** Produces high-fidelity translation and stylistic rephrasings.
4. **Actions:**
   - `Enter`: Copy top translation to clipboard and close.
   - `Ctrl+Enter`: Paste translation directly into previous active application.
   - `Alt+1` … `Alt+9`: Select and copy alternative translations or rephrasings.
   - `Esc`: Dismiss popup immediately.

---

## Input & Caret Tracking

Writy runs globally across terminal editors, native GUI tools, and browsers without needing individual app plugins or extensions.

### Keyboard Interception & Pass-Through

- Utilizes Linux `evdev` to monitor physical input devices and `uinput` to emit virtual keystrokes safely.
- **Zero latency penalty:** Keystrokes pass through transparently to the active window.
- **Fail-safe:** If the Writy process terminates or crashes, the Linux kernel automatically destroys the `uinput` virtual device and releases the `evdev` grab, restoring raw keyboard access instantly.
- **Buffer Flush:** The typing buffer resets immediately upon:
  - Navigation keys: `Enter`, `Escape`, `Arrow keys`, `Home`, `End`, `Page Up/Down`, `Delete`
  - Hotkey combinations: any chords containing `Ctrl`, `Super`, or `Alt`
  - Mouse clicks and window focus changes

### Caret & Focus Detection (AT-SPI2)

- Because Wayland deliberately isolates window contents and does not expose caret coordinates to unprivileged clients, Writy integrates with **AT-SPI2** (Linux Accessibility Bus).
- **Focus & Password Safety:** Queries the focused element's accessibility role. If a field is identified as a password input or a secure terminal, autocomplete and model suggestions are completely disabled.
- **Caret Alignment:** Queries the active text caret bounding box to position the popup adjacent to your cursor.

---

## Dictionaries & Knowledge Bases

Writy ships zero dictionary data by default; you have full control over the sources installed.

### Supported Formats

| Format               | File Types                                           | Usage in Writy                                              |
| -------------------- | ---------------------------------------------------- | ----------------------------------------------------------- |
| **StarDict**         | `.ifo`, `.idx`, `.dict` / `.dict.dz`, `.syn`, `res/` | Primary bilingual & thesaurus format with local media       |
| **OpenZIM**          | `.zim`                                               | Offline Wikipedia & full Wiktionary archives (via `zim-rs`) |
| **Wiktionary JSONL** | Kaikki.org extracts                                  | Rich lexical data (IPA, inflection, etymology, synonyms)    |
| **Plain Wordlists**  | TSV (`word<TAB>count`)                               | High-speed autocomplete and frequency scoring               |

### Required Downloads (First Launch)

On first setup, Writy prompts for the three core required datasets:

1. **English Word Frequency List** (autocomplete & base spellcheck).
2. **English Wiktionary** (definitions, synonyms, English-to-Arabic translations).
3. **Arabic Wiktionary** (Arabic-to-English translations and definitions).

_Optional:_ Offline Wikipedia (`.zim` packages) can be enabled with customized storage limits.

---

## System Architecture

Writy is partitioned into a cross-platform core library and OS-specific frontend/input adapters to allow future ports to Windows and macOS:

```
┌────────────────────────────────────────────────────────────────────────┐
│                              writy-core                                │
│                                                                        │
│   ┌───────────────────┐  ┌───────────────────┐  ┌──────────────────┐   │
│   │   Harper Engine   │  │ Dictionary Engine │  │   Model Engine   │   │
│   │ (Spelling/Grammar)│  │ (StarDict / .zim) │  │(Ollama/Gemini/Cl)│   │
│   └───────────────────┘  └───────────────────┘  └──────────────────┘   │
│                                                                        │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │             Sentence State Machine & Buffer                    │   │
│   └────────────────────────────────────────────────────────────────┘   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
          ┌─────────────────────────┼─────────────────────────┐
          ▼                         ▼                         ▼
┌──────────────────┐      ┌──────────────────┐      ┌──────────────────┐
│   writy-linux    │      │ writy-win (Plan) │      │writy-macos (Plan)│
│                  │      │                  │      │                  │
│ • evdev / uinput │      │ • Low-level hooks│      │ • CGEventTap     │
│ • gtk4-layer-sh  │      │ • WinUI / D2D    │      │ • NSPanel HUD    │
│ • AT-SPI2 caret  │      │ • UI Automation  │      │ • AXUIElement    │
│ • wl-clipboard   │      │ • Win32 Clipbd   │      │ • NSPasteboard   │
└──────────────────┘      └──────────────────┘      └──────────────────┘
```

---

## Configuration

Settings are saved to `~/.config/writy/config.toml` and can be edited directly or through the built-in preferences window (`Ctrl+Alt+S` or `writy settings`).

```toml
enabled = true
insert_popup = true
selection_popup = true
position = "pointer"           # "pointer" | "bottom-right" | "top-right"
max_lines = 7
font = "monospace"
font_size = 13
selection_action = "copy"      # "copy" (paste with Ctrl+V) or "replace" (wtype)

show_images = true
dictionary_colors = false      # false = map to Writy palette
entry_lines = 3
audio_command = "mpv --no-video --really-quiet"

[typing]
min_rephrase_words = 2         # Minimum sentence length for model rephrase
idle_ms = 200                  # Pause before triggering model
engine = "harper"              # Built-in spelling & grammar linting

[model]
backend = "ollama"             # "ollama" | "gemini" | "claude" | "off"
url = "http://localhost:11434" # For ollama
model = "gemma3:4b"            # For ollama: "gemma3:4b" | For gemini: "gemini-3.8-flash" or "gemini-3.5-flash-lite"
keep_alive = "60m"             # Ollama: Automatically unload after idle period
api_key_env = "GEMINI_API_KEY" # For gemini / claude remote backends

[apps]
blocklist = [
  "keepassxc",
  "bitwarden",
  "1password",
  "steam_app_"
]
```

---

## Setup & Installation (Arch + Hyprland)

### 1. Install Dependencies

```bash
sudo pacman -S gtk4 gtk4-layer-shell wl-clipboard wtype ollama mpv at-spi2-core
```

### 2. Configure Permissions for uinput & evdev

Add your user to the `input` group and configure the `uinput` kernel module:

```bash
sudo usermod -aG input $USER
echo uinput | sudo tee /etc/modules-load.d/uinput.conf
echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' \
  | sudo tee /etc/udev/rules.d/99-writy.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

_(Log out and log back in for group permissions to take effect)._

### 3. Build & Run

```bash
git clone https://github.com/your-username/writy.git
cd writy
cargo build --release
./target/release/writy
```

### 4. Hyprland Integration

Writy popups use Wayland `layer-shell` surfaces (namespace `writy`). Because they are layer surfaces rather than standard windows, use `layer_rule` (or `layerrule` in classic `.conf`) to disable animations so the popup appears instantly without transition flicker.

#### If using Lua config (`hyprland.lua` for Hyprland >= 0.55):
```lua
-- Autostart
hl.on("hyprland.start", function()
    hl.exec_cmd("writy")
end)

-- Interactive translator popup hotkey (Alt+G)
hl.bind("ALT", "G", hl.dsp.exec_cmd("writy translate"))

-- Disable popup animations for the writy layer-shell namespace
hl.layer_rule({
    name = "writy-no-anim",
    match = { namespace = "^(writy)$" },
    no_anim = true,
})
```

#### If using classic config (`hyprland.conf`):
```ini
exec-once = writy
bind = ALT, G, exec, writy translate
layerrule = noanim, match:namespace writy
```

---

## Keybindings

| Shortcut          | Context             | Action                                           |
| ----------------- | ------------------- | ------------------------------------------------ |
| `Alt+G`           | Global              | Open interactive English ↔ Arabic translator     |
| `Tab`             | Typing popup active | Accept top spelling / grammar suggestion         |
| `Shift+Tab`       | Typing popup active | Apply Harper style improvement (passive, etc.)   |
| `Alt+1` … `Alt+9` | Any popup active    | Accept numbered rephrase or dictionary item      |
| `Enter`           | Translator popup    | Copy translation to clipboard and close          |
| `Ctrl+Enter`      | Translator popup    | Paste translation into previous active window    |
| `Esc`             | Any popup active    | Dismiss popup (transparently passed to app)      |
| `Ctrl+Alt+W`      | Global              | Pause / Resume Writy                             |
| `Ctrl+Alt+S`      | Global              | Open Settings window (`writy settings`)          |

---

## Roadmap

- [x] Architecture design & spec unification.
- [ ] **Phase 1:** Core pipeline (`writy-core`) — Harper grammar integration, StarDict indexer, and `.zim` reader.
- [ ] **Phase 2:** Dual GTK4 layer-shell popup UI with Pango markup and GTK picture rendering.
- [ ] **Phase 3:** Bidirectional selection translator with Arabic morphology normalization.
- [ ] **Phase 4:** Ollama integration for rephrasing with automatic VRAM unload timers.
- [ ] **Phase 5:** Input grab layer (`evdev`/`uinput`) and AT-SPI2 caret tracking.
- [ ] **Phase 6:** Packaging for Arch Linux (AUR) and cross-platform abstraction prototypes (Windows/macOS).

---

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option. Dictionaries and language models installed by the user retain their respective independent licenses.
