# Direct keyless translation

Checked 2026-11-12 with synthetic English and Arabic text. Writy makes HTTPS
requests itself, using its existing HTTP client; it does not invoke a translator
command or install another runtime dependency.

Google uses the client endpoint at `clients5.google.com/translate_a/t` with
`client=dict-chrome-ex`. The older `translate_a/single?client=gtx` endpoint
returned HTTP 429 during these checks. Input text is in the POST body.

Bing fetches its translator page, parses the temporary key, token, IG and IID,
then posts to `www.bing.com/ttranslatev3`. Browser request headers were needed
in the successful checks. The old Edge token endpoint returned HTTP 404.
Session tokens are not saved or logged. Redirects are not followed; an unexpected
regional redirect fails into local fallback instead of forwarding prose.

Yandex's older request format returned HTTP 403. Its client endpoint at
`translate.yandex.net/api/v1/tr.json/translate` succeeded with `srv=android`, a
client user agent and a per-request identifier. Input text is in the POST body.
No account, API key, cookie import or browser profile is used.

The opt-in `keyless_live_translation_both_directions` Rust test passed for all
three engines in both English→Arabic and Arabic→English. Each input had two
sentences, including a negation; every output included both sentences. This is a
connectivity and parsing spot check, not a translation-quality benchmark.

Offline routing is tested for all three engines. Output bytes and text length
are bounded; empty, malformed and error responses fail into the existing local
fallback. The whole keyless operation shares the configured timeout. Google
input is limited to 5000 characters and Bing to 1000 UTF-16 units; longer inputs
fall back without sending partial text. No chunking or token cache is needed for
this first integration. These unofficial services can change or block requests.

Reference implementations:

- [Bing page authentication and headers](https://github.com/readest/readest/blob/main/apps/readest-app/src/services/translators/providers/azureShared.ts)
- [Yandex client requests](https://github.com/Animenosekai/translate/blob/master/translatepy/translators/yandex.py)
