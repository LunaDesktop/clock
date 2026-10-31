//! Luna Clock — zegar, minutnik, stoper i kalendarz środowiska Luna.
//!
//! Aplikacja jest całkowicie lokalna: wydarzenia mieszkają w pliku użytkownika,
//! a święta liczymy z wbudowanego zestawu reguł. Nie wysyłamy żadnych danych
//! i nie wymagamy konta ani sieci.

pub mod app;
pub mod holidays;
pub mod prefs;
pub mod store;
pub mod time;
pub mod ui;

use std::error::Error;

/// Binarka `luna-clock`.
pub fn run_clock() -> Result<(), Box<dyn Error>> {
    app::run()
}
