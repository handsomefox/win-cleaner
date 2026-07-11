//! Screen rendering. Each module draws one screen (or shared pieces) and
//! reports user intents back to `app.rs` instead of mutating app state.

pub(crate) mod about;
pub(crate) mod components;
pub(crate) mod history;
pub(crate) mod progress;
pub(crate) mod results;
pub(crate) mod scan;
pub(crate) mod select;
