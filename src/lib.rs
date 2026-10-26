pub mod domain;
pub mod storage;
pub mod providers;
pub mod forecast;
pub mod alerts;
pub mod file_secret_store;
pub mod service;
pub mod i18n;
pub mod ui;

/// Visual preview fixture: real views + mock data + PNG dumps.
#[cfg(feature = "preview")]
pub mod preview;

// Geometry helpers are part of the tested public surface; the integration
// tests address them as `last_token_lib::tray::…`.
pub use ui::tray;
