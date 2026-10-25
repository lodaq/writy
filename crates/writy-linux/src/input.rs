use std::collections::HashMap;
use xkbcommon::xkb::{self, compose};

pub enum Decoded {
    Text(String),
    Reset,
    Ignore,
}
struct Keyboard {
    state: xkb::State,
    compose: Option<compose::State>,
}
pub struct Decoder {
    context: xkb::Context,
    keyboards: HashMap<String, Keyboard>,
}
impl Default for Decoder {
    fn default() -> Self {
        Self {
            context: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            keyboards: HashMap::new(),
        }
    }
}
impl Decoder {
    pub fn keymap(&mut self, device: String, map: String) -> bool {
        let Some(map) = xkb::Keymap::new_from_string(
            &self.context,
            map,
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) else {
            return false;
        };
        let locale = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_CTYPE"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or("C.UTF-8".into());
        let compose = compose::Table::new_from_locale(
            &self.context,
            std::ffi::OsStr::new(&locale),
            compose::COMPILE_NO_FLAGS,
        )
        .ok()
        .map(|t| compose::State::new(&t, compose::STATE_NO_FLAGS));
        self.keyboards.insert(
            device,
            Keyboard {
                state: xkb::State::new(&map),
                compose,
            },
        );
        true
    }
    pub fn modifiers(
        &mut self,
        device: &str,
        depressed: u32,
        latched: u32,
        locked: u32,
        group: u32,
    ) {
        if let Some(k) = self.keyboards.get_mut(device) {
            k.state.update_mask(depressed, latched, locked, 0, 0, group);
        }
    }
    pub fn reset(&mut self) {
        for k in self.keyboards.values_mut() {
            if let Some(c) = k.compose.as_mut() {
                c.reset();
            }
        }
    }
    pub fn key(&mut self, device: &str, keycode: u32, state: u32) -> Decoded {
        if state == 0 {
            return Decoded::Ignore;
        }
        let Some(k) = self.keyboards.get_mut(device) else {
            return Decoded::Reset;
        };
        let code = xkb::Keycode::new(keycode.saturating_add(8));
        let sym = k.state.key_get_one_sym(code);
        // Print is a non-editing screenshot key; keep visible suggestions for capture.
        if sym.raw() == xkb::keysyms::KEY_Print {
            return Decoded::Ignore;
        }
        // Modifiers do not invalidate prose; navigation, deletion and shortcuts do.
        if (0xffe1..=0xffee).contains(&sym.raw())
            || matches!(sym.raw(),0xfe03..=0xfe05|0xfe11..=0xfe13|0xff7e)
        {
            return Decoded::Ignore;
        }
        if [xkb::MOD_NAME_CTRL, xkb::MOD_NAME_ALT, xkb::MOD_NAME_LOGO]
            .iter()
            .any(|m| k.state.mod_name_is_active(m, xkb::STATE_MODS_EFFECTIVE))
        {
            return Decoded::Reset;
        }
        if let Some(c) = k.compose.as_mut() {
            c.feed(sym);
            match c.status() {
                compose::Status::Composing => return Decoded::Ignore,
                compose::Status::Composed => {
                    let text = c.utf8().unwrap_or_default();
                    c.reset();
                    return Decoded::Text(text);
                }
                compose::Status::Cancelled => {
                    c.reset();
                    return Decoded::Reset;
                }
                compose::Status::Nothing => {}
            }
        }
        let text = k.state.key_get_utf8(code);
        if text.is_empty() || text.chars().any(char::is_control) {
            Decoded::Reset
        } else {
            Decoded::Text(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_is_decoded_from_device_keymap_not_assumed_us() {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let map = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us,ara",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .unwrap();
        let mut d = Decoder::default();
        assert!(d.keymap("kbd".into(), map.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1)));
        assert!(matches!(d.key("kbd",16,1), Decoded::Text(s) if s=="q"));
        d.modifiers("kbd", 0, 0, 0, 1);
        assert!(matches!(d.key("kbd",16,1), Decoded::Text(s) if s=="ض"));
        assert!(matches!(d.key("kbd", 99, 1), Decoded::Ignore));
        assert!(matches!(d.key("kbd", 14, 1), Decoded::Reset));
        assert!(matches!(d.key("missing", 16, 1), Decoded::Reset));
    }
}
