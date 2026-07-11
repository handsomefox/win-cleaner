//! Category glyphs from Lucide (ISC, see `assets/icons/LICENSE`). Their
//! stroke color is baked to the muted theme tone, so they embed as plain
//! static image sources.

use eframe::egui::ImageSource;
use eframe::egui::include_image;

use crate::strings::UiText;

/// Maps a category display name to its glyph.
pub(crate) fn category_icon(texts: &UiText, name: &str) -> ImageSource<'static> {
    if name == texts.category_browsers {
        include_image!("../assets/icons/globe.svg")
    } else if name == texts.category_chat {
        include_image!("../assets/icons/message-circle.svg")
    } else if name == texts.category_development {
        include_image!("../assets/icons/code.svg")
    } else if name == texts.category_gaming {
        include_image!("../assets/icons/gamepad-2.svg")
    } else if name == texts.category_media {
        include_image!("../assets/icons/music.svg")
    } else if name == texts.category_system {
        include_image!("../assets/icons/settings.svg")
    } else if name == texts.category_creative {
        include_image!("../assets/icons/palette.svg")
    } else {
        include_image!("../assets/icons/box.svg")
    }
}
