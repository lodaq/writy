# Writy glossary

**Reading assistance**: Help understanding selected or manually entered English
or Arabic text through dictionary entries and translation.

**Writing assistance**: English spelling, grammar, and phrasing suggestions
while composing prose. Arabic writing correction is a future capability.

**Search mode**: Explicit lookup from Writy's focused search window.

**Select mode**: Reading assistance for the current primary selection.

**Insert mode**: Ambient assistance for newly typed English prose, while the
current application and field are eligible.

**HUD**: A compact result surface dedicated to a linguistic category, such as
translation, definitions, or writing suggestions. A HUD normally leaves keyboard
focus in the application being assisted.

**Lexical entry**: Source-attributed information about a headword and its senses.
An entry may contain translations, definitions, examples, pronunciations,
collocations, related words, and usage labels; missing categories remain absent.

**Sense**: One meaning or use of a headword. Different senses can have different
translations and examples.

**Suggestion**: A proposed correction or alternative phrasing. It only changes
application text after the user accepts it.

**Completion**: A proposed continuation of an unfinished word or phrase.

**Word family**: Words linked by derivation, such as *decide*, *decision*, and
*decisive*. Inflections such as *decided* are grammatical forms of a word and
should be distinguished from derivations.

**Collocation**: Words frequently used together, such as *make a decision* or
*قرار حاسم*.

**Provider**: A configured source of generated translations or rewrites. A
provider may run locally or remotely.

**Prose mode**: An explicit per-session permission to offer writing assistance
inside a terminal. It does not identify shell commands or hidden prompts.

**Opaque input**: A field whose semantic role, surrounding text, or caret cannot
be verified. Assistance in opaque input is best effort and may be disabled.

**Generation**: A version identifying the text and focus context of a request.
Results from older generations must not replace current results or text.
