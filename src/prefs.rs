//! Preferencje `luna-clock` (`$XDG_CONFIG_HOME/luna/clock.conf`).
//!
//! Format jest wspólny dla całej Luny (`klucz = wartość`). Motyw automatyczny
//! zapisujemy jako **godziny lokalne**: od `dark_hour` obowiązuje wariant
//! ciemny, a od `light_hour` — jasny.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use libluna_core::config::{config_file, parse_pairs, save_atomic};

use crate::time::{DateFields, DateFormat, DateOrder, DateSeparator, DateStyle, LocalTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prefs {
    /// Czy przełączać systemowy wariant motywu o wybranych godzinach.
    pub auto_theme: bool,
    /// Godzina (0–23), od której ma być ciemno.
    pub dark_hour: u32,
    /// Godzina (0–23), od której ma być jasno.
    pub light_hour: u32,
    /// Pokazuj święta wybranego kraju.
    pub show_holidays: bool,
    /// Kod kraju (`PL`, `DE`…); pusty = wykryj automatycznie.
    pub country: String,
    pub show_seconds: bool,
    pub use_24h: bool,
    /// Strefy dodatkowych zegarów (IANA).
    pub world_clocks: Vec<String>,
    /// Domyślny czas minutnika w minutach.
    pub timer_minutes: u32,
    /// Format daty: zakres pól, cyfry/napisy, kolejność i separator.
    pub date_format: DateFormat,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            auto_theme: false,
            dark_hour: 19,
            light_hour: 7,
            show_holidays: true,
            country: String::new(),
            show_seconds: true,
            use_24h: true,
            world_clocks: Vec::new(),
            timer_minutes: 5,
            date_format: DateFormat::default(),
        }
    }
}

impl Prefs {
    pub fn path() -> Option<PathBuf> {
        config_file("clock.conf")
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|path| Self::load_from(&path).ok())
            .unwrap_or_default()
    }

    fn load_from(path: &Path) -> io::Result<Self> {
        Ok(Self::parse(&fs::read_to_string(path)?))
    }

    pub fn parse(text: &str) -> Self {
        let mut prefs = Self::default();
        for (key, value) in parse_pairs(text) {
            match key.as_str() {
                "auto_theme" => prefs.auto_theme = truthy(&value),
                "dark_hour" => {
                    if let Ok(hour) = value.parse::<u32>() {
                        prefs.dark_hour = hour.min(23);
                    }
                }
                "light_hour" => {
                    if let Ok(hour) = value.parse::<u32>() {
                        prefs.light_hour = hour.min(23);
                    }
                }
                "show_holidays" => prefs.show_holidays = truthy(&value),
                "country" => {
                    let code = value.trim().to_ascii_uppercase();
                    prefs.country =
                        if code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic()) {
                            code
                        } else {
                            String::new()
                        };
                }
                "show_seconds" => prefs.show_seconds = truthy(&value),
                "use_24h" => prefs.use_24h = truthy(&value),
                "date_fields" => {
                    if let Some(fields) = DateFields::from_key(&value) {
                        prefs.date_format.fields = fields;
                    }
                }
                "date_style" => {
                    if let Some(style) = DateStyle::from_key(&value) {
                        prefs.date_format.style = style;
                    }
                }
                "date_order" => {
                    if let Some(order) = DateOrder::from_key(&value) {
                        prefs.date_format.order = order;
                    }
                }
                "date_separator" => {
                    if let Some(separator) = DateSeparator::from_key(&value) {
                        prefs.date_format.separator = separator;
                    }
                }
                "timer_minutes" => {
                    if let Ok(minutes) = value.parse::<u32>() {
                        prefs.timer_minutes = minutes.clamp(1, 24 * 60);
                    }
                }
                "world_clock" => {
                    if let Some(zone) = valid_zone(&value) {
                        if !prefs
                            .world_clocks
                            .iter()
                            .any(|existing| existing.eq_ignore_ascii_case(&zone))
                        {
                            prefs.world_clocks.push(zone);
                        }
                    }
                }
                _ => {}
            }
        }
        if prefs.world_clocks.is_empty() {
            prefs.world_clocks = default_world_clocks();
        }
        prefs
    }

    pub fn render(&self) -> String {
        let mut out = String::from(
            "# Luna Clock — zegar, minutnik i kalendarz\n\
             # Wszystkie dane zostają na tym urządzeniu.\n",
        );
        out.push_str(&format!("auto_theme = {}\n", u8::from(self.auto_theme)));
        out.push_str(&format!("dark_hour = {}\n", self.dark_hour.min(23)));
        out.push_str(&format!("light_hour = {}\n", self.light_hour.min(23)));
        out.push_str(&format!(
            "show_holidays = {}\n",
            u8::from(self.show_holidays)
        ));
        if !self.country.is_empty() {
            out.push_str(&format!("country = {}\n", self.country));
        }
        out.push_str(&format!("show_seconds = {}\n", u8::from(self.show_seconds)));
        out.push_str(&format!("use_24h = {}\n", u8::from(self.use_24h)));
        out.push_str(&format!(
            "date_fields = {}\n",
            self.date_format.fields.key()
        ));
        out.push_str(&format!("date_style = {}\n", self.date_format.style.key()));
        out.push_str(&format!("date_order = {}\n", self.date_format.order.key()));
        out.push_str(&format!(
            "date_separator = {}\n",
            self.date_format.separator.key()
        ));
        out.push_str(&format!(
            "timer_minutes = {}\n",
            self.timer_minutes.clamp(1, 1440)
        ));
        for zone in &self.world_clocks {
            if let Some(zone) = valid_zone(zone) {
                out.push_str(&format!("world_clock = {zone}\n"));
            }
        }
        out
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        let _ = self.save_to(&path);
    }

    fn save_to(&self, path: &Path) -> io::Result<()> {
        save_atomic(path, &self.render())
    }

    /// Czy o `hour` ma obowiązywać ciemny wariant.
    pub fn is_dark_at(&self, hour: u32) -> bool {
        if self.dark_hour == self.light_hour {
            return false;
        }
        let hour = hour.min(23);
        if self.dark_hour < self.light_hour {
            // Ciemno w środku dnia (np. 8 → 18).
            hour >= self.dark_hour && hour < self.light_hour
        } else {
            // Przedział ciemny przechodzi przez północ (np. 19 → 7).
            hour >= self.dark_hour || hour < self.light_hour
        }
    }

    /// Czy teraz ma obowiązywać ciemny wariant (czas lokalny).
    pub fn scheduled_dark(&self, now: &LocalTime) -> bool {
        self.is_dark_at(now.hour)
    }

    /// Dodaje strefę (bez duplikatów). Zwraca `false`, gdy już była.
    pub fn add_zone(&mut self, zone: &str) -> bool {
        let Some(zone) = valid_zone(zone) else {
            return false;
        };
        if self
            .world_clocks
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&zone))
        {
            return false;
        }
        self.world_clocks.push(zone);
        true
    }

    pub fn remove_zone(&mut self, index: usize) {
        if index < self.world_clocks.len() {
            self.world_clocks.remove(index);
        }
    }
}

/// Domyślne zegary: strefa systemowa i UTC (bez duplikatów).
pub fn default_world_clocks() -> Vec<String> {
    let current = libluna_core::region::current_timezone();
    let mut zones = Vec::new();
    if valid_zone(&current).is_some() {
        zones.push(current);
    }
    if !zones.iter().any(|zone| zone == "UTC") {
        zones.push("UTC".to_owned());
    }
    zones
}

fn truthy(value: &str) -> bool {
    matches!(value.trim(), "1" | "true" | "yes" | "on")
}

/// Bezpieczna nazwa strefy (tak jak w `region.conf`).
fn valid_zone(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.starts_with('/')
        || value.contains("..")
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '+'))
    {
        return None;
    }
    Some(value.to_owned())
}

/// Miasto ze strefy IANA (`Europe/Warsaw` → `Warsaw`).
pub fn zone_city(zone: &str) -> String {
    zone.rsplit('/').next().unwrap_or(zone).replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(hour: u32) -> LocalTime {
        LocalTime {
            date: crate::time::CivilDate::from_ymd(2026, 10, 31),
            hour,
            minute: 0,
            second: 0,
            weekday: 5,
            offset_seconds: 0,
            abbr: "UTC".to_owned(),
        }
    }

    #[test]
    fn parses_and_renders_roundtrip() {
        let text = "auto_theme=1\ndark_hour=21\nlight_hour=6\nshow_holidays=0\ncountry=pl\nshow_seconds=0\nuse_24h=0\ntimer_minutes=25\nworld_clock=Europe/Warsaw\nworld_clock=America/New_York\nworld_clock=Europe/Warsaw\n";
        let prefs = Prefs::parse(text);
        assert!(prefs.auto_theme);
        assert_eq!(prefs.dark_hour, 21);
        assert_eq!(prefs.light_hour, 6);
        assert!(!prefs.show_holidays);
        assert_eq!(prefs.country, "PL");
        assert!(!prefs.show_seconds);
        assert!(!prefs.use_24h);
        assert_eq!(prefs.timer_minutes, 25);
        assert_eq!(prefs.world_clocks.len(), 2);
        let reparsed = Prefs::parse(&prefs.render());
        assert_eq!(reparsed, prefs);
    }

    #[test]
    fn parses_and_renders_date_format() {
        let prefs = Prefs::parse(
            "date_fields=day_month\ndate_style=words\ndate_order=month_first\ndate_separator=slash\n",
        );
        assert_eq!(prefs.date_format.fields, DateFields::DayMonth);
        assert_eq!(prefs.date_format.style, DateStyle::Words);
        assert_eq!(prefs.date_format.order, DateOrder::MonthFirst);
        assert_eq!(prefs.date_format.separator, DateSeparator::Slash);

        let reparsed = Prefs::parse(&prefs.render());
        assert_eq!(reparsed.date_format, prefs.date_format);

        // Nieznane wartości nie psują domyślnych.
        let bad = Prefs::parse("date_style=xyz\ndate_separator=??\n");
        assert_eq!(bad.date_format, DateFormat::default());
    }

    #[test]
    fn clamps_and_rejects_bad_zones() {
        let prefs = Prefs::parse(
            "dark_hour=99\nlight_hour=99\ntimer_minutes=0\nworld_clock=../../etc/passwd\nworld_clock=/etc/localtime\n",
        );
        assert_eq!(prefs.dark_hour, 23);
        assert_eq!(prefs.light_hour, 23);
        assert_eq!(prefs.timer_minutes, 1);
        assert!(prefs
            .world_clocks
            .iter()
            .all(|zone| valid_zone(zone).is_some()));
    }

    #[test]
    fn theme_schedule_crosses_midnight() {
        let prefs = Prefs {
            dark_hour: 19,
            light_hour: 7,
            ..Prefs::default()
        };
        assert!(prefs.is_dark_at(19));
        assert!(prefs.is_dark_at(23));
        assert!(prefs.is_dark_at(0));
        assert!(prefs.is_dark_at(6));
        assert!(!prefs.is_dark_at(7));
        assert!(!prefs.is_dark_at(12));
        assert!(!prefs.is_dark_at(18));
        assert!(prefs.scheduled_dark(&local(21)));

        let day = Prefs {
            dark_hour: 8,
            light_hour: 18,
            ..Prefs::default()
        };
        assert!(day.is_dark_at(8));
        assert!(day.is_dark_at(12));
        assert!(day.is_dark_at(17));
        assert!(!day.is_dark_at(18));
        assert!(!day.is_dark_at(20));
    }

    #[test]
    fn zone_city_uses_last_segment() {
        assert_eq!(zone_city("Europe/Warsaw"), "Warsaw");
        assert_eq!(zone_city("America/Argentina/Buenos_Aires"), "Buenos Aires");
        assert_eq!(zone_city("UTC"), "UTC");
    }

    #[test]
    fn local_now_is_sane() {
        let now = crate::time::local_now();
        assert!(now.hour < 24);
        assert!(now.minute < 60);
    }
}
