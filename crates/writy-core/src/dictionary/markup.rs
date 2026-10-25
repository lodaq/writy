use quick_xml::{Reader, events::Event};
use std::path::{Component, Path};

#[derive(Default)]
pub(super) struct Content {
    pub text: String,
    pub markup: Option<String>,
    pub audio: Vec<String>,
    pub images: Vec<String>,
}

/// Produces plain text, a restricted Pango subset, and vetted local resources.
/// Arbitrary HTML, attributes, scripts, CSS and network resources are never rendered.
pub(super) fn extract(input: &str, root: &Path) -> Content {
    let mut content = Content {
        markup: safe_pango(input),
        ..Default::default()
    };
    let mut reader = Reader::from_str(input);
    reader.config_mut().check_end_names = false;
    let mut hidden = 0_usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag)) => {
                let name = String::from_utf8_lossy(tag.name().as_ref()).to_ascii_lowercase();
                if matches!(name.as_str(), "script" | "style" | "iframe" | "object") {
                    hidden += 1;
                    continue;
                }
                if hidden > 0 {
                    continue;
                }
                if matches!(
                    name.as_str(),
                    "br" | "div" | "p" | "li" | "tr" | "blockquote"
                ) {
                    content.text.push('\n');
                }
                for attribute in tag.attributes().flatten() {
                    let key = String::from_utf8_lossy(attribute.key.as_ref()).to_ascii_lowercase();
                    if !matches!(key.as_str(), "src" | "href") {
                        continue;
                    }
                    let value = attribute
                        .decode_and_unescape_value(reader.decoder())
                        .map(|v| v.into_owned())
                        .unwrap_or_default();
                    if let Some(path) = value
                        .strip_prefix("sound://")
                        .and_then(|v| resource(root, v, true))
                    {
                        push_unique(&mut content.audio, path);
                    }
                    if matches!(name.as_str(), "img" | "image")
                        && key == "src"
                        && let Some(path) = resource(root, &value, false)
                    {
                        push_unique(&mut content.images, path);
                    }
                }
            }
            Ok(Event::End(tag)) => {
                let name = String::from_utf8_lossy(tag.name().as_ref()).to_ascii_lowercase();
                if matches!(name.as_str(), "script" | "style" | "iframe" | "object") {
                    hidden = hidden.saturating_sub(1);
                }
                if hidden == 0 && matches!(name.as_str(), "div" | "p" | "li" | "tr") {
                    content.text.push('\n');
                }
            }
            Ok(Event::Text(text)) if hidden == 0 => {
                let decoded = text.decode().unwrap_or_default();
                content.text.push_str(&decode_entities(&decoded));
            }
            Ok(Event::GeneralRef(reference)) if hidden == 0 => {
                let name = reference.decode().unwrap_or_default();
                content
                    .text
                    .push_str(&decode_entities(&format!("&{name};")));
            }
            Ok(Event::CData(text)) if hidden == 0 => content
                .text
                .push_str(&String::from_utf8_lossy(text.as_ref())),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    content.text = content
        .text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    content
}

fn safe_pango(input: &str) -> Option<String> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().check_end_names = false;
    let mut output = String::new();
    let mut stack = Vec::new();
    let mut hidden = 0_usize;
    fn tag(name: &str) -> Option<&'static str> {
        match name {
            "b" | "strong" => Some("b"),
            "i" | "em" => Some("i"),
            "u" => Some("u"),
            "s" | "strike" => Some("s"),
            "sub" => Some("sub"),
            "sup" => Some("sup"),
            _ => None,
        }
    }
    fn escape(text: &str) -> String {
        quick_xml::escape::escape(text).into_owned()
    }
    loop {
        match reader.read_event() {
            Ok(Event::Start(t)) => {
                let name = String::from_utf8_lossy(t.name().as_ref()).to_ascii_lowercase();
                if matches!(name.as_str(), "script" | "style" | "iframe" | "object") {
                    hidden += 1;
                    continue;
                }
                if hidden > 0 {
                    continue;
                }
                if let Some(t) = tag(&name) {
                    if stack.len() >= 64 {
                        return None;
                    }
                    output.push_str(&format!("<{t}>"));
                    stack.push(t);
                }
                if matches!(name.as_str(), "p" | "div" | "li" | "tr" | "br") {
                    output.push('\n');
                }
            }
            Ok(Event::Empty(t)) => {
                if hidden == 0 && t.name().as_ref() == b"br" {
                    output.push('\n');
                }
            }
            Ok(Event::End(t)) => {
                let name = String::from_utf8_lossy(t.name().as_ref()).to_ascii_lowercase();
                if matches!(name.as_str(), "script" | "style" | "iframe" | "object") {
                    hidden = hidden.saturating_sub(1);
                    continue;
                }
                if hidden > 0 {
                    continue;
                }
                if let Some(t) = tag(&name)
                    && let Some(index) = stack.iter().rposition(|v| *v == t)
                {
                    while stack.len() > index {
                        output.push_str(&format!("</{}>", stack.pop()?));
                    }
                }
                if matches!(name.as_str(), "p" | "div" | "li" | "tr") {
                    output.push('\n');
                }
            }
            Ok(Event::Text(t)) if hidden == 0 => {
                output.push_str(&escape(&decode_entities(&t.decode().ok()?)))
            }
            Ok(Event::GeneralRef(t)) if hidden == 0 => output.push_str(&escape(&decode_entities(
                &format!("&{};", t.decode().ok()?),
            ))),
            Ok(Event::CData(t)) if hidden == 0 => {
                output.push_str(&escape(&String::from_utf8_lossy(t.as_ref())))
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }
    while let Some(t) = stack.pop() {
        output.push_str(&format!("</{t}>"));
    }
    Some(output.trim().to_owned())
}

fn decode_entities(input: &str) -> String {
    let input = input.replace("&nbsp;", " ");
    quick_xml::escape::unescape(&input)
        .map(|v| v.into_owned())
        .unwrap_or(input)
}

pub(super) fn resource(root: &Path, value: &str, audio: bool) -> Option<String> {
    if value.is_empty() || value.contains([':', '\\', '%', '\0']) {
        return None;
    }
    let relative = Path::new(value);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let extension = relative.extension()?.to_str()?.to_ascii_lowercase();
    let allowed = if audio {
        matches!(
            extension.as_str(),
            "ogg" | "wav" | "mp3" | "flac" | "m4a" | "aac"
        )
    } else {
        matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif")
    };
    if !allowed {
        return None;
    }
    let root = root.canonicalize().ok()?;
    let media_root = root.join("res").canonicalize().ok()?;
    if !media_root.starts_with(&root) {
        return None;
    }
    let path = media_root.join(relative).canonicalize().ok()?;
    if !path.starts_with(media_root)
        || !path.is_file()
        || path.metadata().ok()?.len() > 32 * 1024 * 1024
    {
        return None;
    }
    Some(path.to_string_lossy().into_owned())
}

fn push_unique(target: &mut Vec<String>, value: String) {
    if !target.contains(&value) {
        target.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_markup_keeps_basic_formatting_and_drops_active_content() {
        let result=safe_pango("<b>word</b> <i>usage</i><script>secret()</script><span size='999999'> &amp; &lt;x&gt;</span>").unwrap();
        assert_eq!(result, "<b>word</b> <i>usage</i> &amp; &lt;x&gt;");
        assert!(!result.contains("script"));
        assert!(!result.contains("999999"));
    }
    #[test]
    fn malformed_markup_is_balanced_and_entities_stay_text() {
        assert_eq!(
            safe_pango("<b><i>safe</b> text").unwrap(),
            "<b><i>safe</i></b> text"
        );
    }
}
