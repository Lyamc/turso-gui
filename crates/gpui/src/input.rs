use gpui::{App, ClipboardItem, KeyDownEvent};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveField {
    None,
    Path,
    Query,
    Cell,
    BrowseFilter(usize),
    SqlFilter(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyEffect {
    None,
    Changed,
    Submit,
}

pub fn pop_grapheme(buffer: &mut String) {
    if let Some((idx, _)) = buffer.grapheme_indices(true).next_back() {
        buffer.truncate(idx);
    }
}

pub fn apply_keystroke(buffer: &mut String, ev: &KeyDownEvent, cx: &App) -> KeyEffect {
    let ks = &ev.keystroke;
    if ks.modifiers.control || ks.modifiers.platform {
        match ks.key.as_str() {
            "v" => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    buffer.push_str(&text.replace('\r', "\n"));
                    return KeyEffect::Changed;
                }
            }
            "c" => {
                cx.write_to_clipboard(ClipboardItem::new_string(buffer.clone()));
            }
            _ => {}
        }
        return KeyEffect::None;
    }

    match ks.key.as_str() {
        "backspace" => {
            pop_grapheme(buffer);
            KeyEffect::Changed
        }
        "enter" | "return" => {
            if ks.modifiers.shift {
                buffer.push('\n');
                KeyEffect::Changed
            } else {
                KeyEffect::Submit
            }
        }
        "space" => {
            buffer.push(' ');
            KeyEffect::Changed
        }
        "tab" | "up" | "down" | "left" | "right" | "escape" | "home" | "end" | "delete"
        | "pageup" | "pagedown" | "shift" | "control" | "alt" | "cmd" | "win" | "super" | "fn" => {
            KeyEffect::None
        }
        _ => {
            if let Some(ch) = &ks.key_char {
                if !ks.modifiers.control
                    && !ks.modifiers.platform
                    && !ch.is_empty()
                    && !ch.chars().any(char::is_control)
                {
                    buffer.push_str(ch);
                    return KeyEffect::Changed;
                }
            }
            KeyEffect::None
        }
    }
}

pub fn display_value(value: &str, placeholder: &str, focused: bool) -> (String, bool) {
    if focused {
        (format!("{value}|"), false)
    } else if value.is_empty() {
        (placeholder.to_string(), true)
    } else {
        (value.to_string(), false)
    }
}
