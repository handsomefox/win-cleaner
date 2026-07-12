//! Phosphor icon glyphs (MIT, see `assets/fonts/PHOSPHOR-LICENSE`). The font is
//! installed in `theme.rs`, so these `&str` codepoints render anywhere text
//! does. Codepoints are copied verbatim from egui-phosphor's `regular.rs`.

use crate::strings::UiText;

// Category glyphs.
pub(crate) const CAT_ALL: &str = "\u{E464}"; // SQUARES_FOUR
pub(crate) const CAT_BROWSERS: &str = "\u{E288}"; // GLOBE
pub(crate) const CAT_CHAT: &str = "\u{E168}"; // CHAT_CIRCLE
pub(crate) const CAT_DEVELOPMENT: &str = "\u{E1BC}"; // CODE
pub(crate) const CAT_GAMING: &str = "\u{E26E}"; // GAME_CONTROLLER
pub(crate) const CAT_MEDIA: &str = "\u{E340}"; // MUSIC_NOTES
pub(crate) const CAT_SYSTEM: &str = "\u{E270}"; // GEAR
pub(crate) const CAT_CREATIVE: &str = "\u{E6C8}"; // PALETTE
pub(crate) const CAT_EMPTY_FOLDERS: &str = "\u{E8F8}"; // FOLDER_DASHED
pub(crate) const CAT_OTHER: &str = "\u{E390}"; // PACKAGE (also the fallback icon)

// UI glyphs.
pub(crate) const CHECKBOX_CHECKED: &str = "\u{E186}"; // CHECK_SQUARE
pub(crate) const CHECKBOX_UNCHECKED: &str = "\u{E45E}"; // SQUARE
pub(crate) const CHECKBOX_PARTIAL: &str = "\u{ED4C}"; // MINUS_SQUARE
pub(crate) const CHEVRON_EXPANDED: &str = "\u{E136}"; // CARET_DOWN
pub(crate) const CHEVRON_COLLAPSED: &str = "\u{E13A}"; // CARET_RIGHT
pub(crate) const HISTORY: &str = "\u{E1A0}"; // CLOCK_COUNTER_CLOCKWISE
pub(crate) const ABOUT: &str = "\u{E2CE}"; // INFO
pub(crate) const RESCAN: &str = "\u{E036}"; // ARROW_CLOCKWISE
pub(crate) const SEARCH: &str = "\u{E30C}"; // MAGNIFYING_GLASS
pub(crate) const SORT: &str = "\u{E444}"; // SORT_ASCENDING
pub(crate) const DETAILS: &str = "\u{E23A}"; // FILE_TEXT
pub(crate) const CLOSE: &str = "\u{E4F6}"; // X
pub(crate) const WARNING: &str = "\u{E4E0}"; // WARNING
pub(crate) const ERROR: &str = "\u{E4E2}"; // WARNING_CIRCLE
pub(crate) const SUCCESS: &str = "\u{E184}"; // CHECK_CIRCLE
pub(crate) const PREVIEW: &str = "\u{E220}"; // EYE
pub(crate) const CLEAN: &str = "\u{EC54}"; // BROOM
pub(crate) const RECYCLE: &str = "\u{E4A6}"; // TRASH

/// Prefixes a label with its glyph and two spaces of separation.
pub(crate) fn with_label(glyph: &str, label: &str) -> String {
    format!("{glyph}  {label}")
}

/// Maps a category display name to its glyph. Interim helper keyed on the
/// display name; B3 switches this to a `Category` match.
pub(crate) fn category_icon(texts: &UiText, name: &str) -> &'static str {
    if name == texts.category_browsers {
        CAT_BROWSERS
    } else if name == texts.category_chat {
        CAT_CHAT
    } else if name == texts.category_development {
        CAT_DEVELOPMENT
    } else if name == texts.category_gaming {
        CAT_GAMING
    } else if name == texts.category_media {
        CAT_MEDIA
    } else if name == texts.category_system {
        CAT_SYSTEM
    } else if name == texts.category_creative {
        CAT_CREATIVE
    } else if name == texts.category_empty_folders {
        CAT_EMPTY_FOLDERS
    } else {
        CAT_OTHER
    }
}
