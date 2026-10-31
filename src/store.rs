//! Prywatny magazyn wydarzeń kalendarza.
//!
//! Format jest prosty i czytelny dla człowieka: po jednym wydarzeniu na linię,
//! pola rozdzielone tabulatorem, a tytuł i notatka escapowane (`\\`, `\t`, `\n`).
//! Plik leży w katalogu danych użytkownika i **nigdy** nie jest wysyłany.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use libluna_core::config::save_atomic_private;

use crate::time::{local_at, CivilDate};

/// Wydarzenie użytkownika.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: u64,
    pub date: CivilDate,
    /// Minuta od północy; `None` = wydarzenie całodniowe.
    pub start_minute: Option<u32>,
    /// Czas trwania w minutach (0 dla całodniowych).
    pub duration_min: u32,
    pub title: String,
    pub note: String,
}

impl Event {
    pub fn all_day(date: CivilDate, title: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            id: 0,
            date,
            start_minute: None,
            duration_min: 0,
            title: title.into(),
            note: note.into(),
        }
    }

    pub fn timed(
        date: CivilDate,
        start_minute: u32,
        duration_min: u32,
        title: impl Into<String>,
        note: impl Into<String>,
    ) -> Self {
        Self {
            id: 0,
            date,
            start_minute: Some(start_minute.min(1_439)),
            duration_min,
            title: title.into(),
            note: note.into(),
        }
    }

    /// `HH:MM` albo `—` dla całodniowych.
    pub fn start_label(&self, use_24h: bool) -> String {
        match self.start_minute {
            Some(minute) if use_24h => format!("{:02}:{:02}", minute / 60, minute % 60),
            Some(minute) => {
                let hour = minute / 60;
                let suffix = if hour < 12 { "AM" } else { "PM" };
                let hour = match hour % 12 {
                    0 => 12,
                    h => h,
                };
                format!("{hour}:{:02} {suffix}", minute % 60)
            }
            None => "—".to_owned(),
        }
    }

    pub fn end_minute(&self) -> Option<u32> {
        self.start_minute
            .map(|start| (start + self.duration_min).min(24 * 60))
    }
}

/// Trwały zbiór wydarzeń.
#[derive(Debug, Clone)]
pub struct EventStore {
    events: Vec<Event>,
    next_id: u64,
    path: Option<PathBuf>,
}

impl Default for EventStore {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            next_id: 1,
            path: Self::default_path(),
        }
    }
}

impl EventStore {
    /// `$XDG_DATA_HOME/luna/clock/events.tsv` (albo `~/.local/share/...`).
    pub fn default_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_DATA_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
            })?;
        Some(base.join("luna").join("clock").join("events.tsv"))
    }

    pub fn load() -> Self {
        Self::from_path(Self::default_path())
    }

    pub fn from_path(path: Option<PathBuf>) -> Self {
        let text = path
            .as_deref()
            .and_then(|path| fs::read_to_string(path).ok())
            .unwrap_or_default();
        Self::from_text(&text, path)
    }

    pub fn from_text(text: &str, path: Option<PathBuf>) -> Self {
        let mut events = Vec::new();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            if let Some(event) = parse_line(line) {
                events.push(event);
            }
        }
        events.sort_by_key(event_sort_key);
        let next_id = events
            .iter()
            .map(|event| event.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        Self {
            events,
            next_id,
            path,
        }
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    pub fn add(
        &mut self,
        date: CivilDate,
        start_minute: Option<u32>,
        duration_min: u32,
        title: impl Into<String>,
        note: impl Into<String>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.events.push(Event {
            id,
            date,
            start_minute,
            duration_min: if start_minute.is_some() {
                duration_min.clamp(0, 10_080)
            } else {
                0
            },
            title: title.into(),
            note: note.into(),
        });
        self.events.sort_by_key(event_sort_key);
        id
    }

    pub fn update(&mut self, event: Event) -> bool {
        let Some(slot) = self
            .events
            .iter_mut()
            .find(|existing| existing.id == event.id)
        else {
            return false;
        };
        *slot = Event {
            duration_min: if event.start_minute.is_some() {
                event.duration_min.clamp(0, 10_080)
            } else {
                0
            },
            ..event
        };
        self.events.sort_by_key(event_sort_key);
        true
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.events.len();
        self.events.retain(|event| event.id != id);
        self.events.len() != before
    }

    pub fn get(&self, id: u64) -> Option<&Event> {
        self.events.iter().find(|event| event.id == id)
    }

    pub fn on(&self, date: CivilDate) -> impl Iterator<Item = &Event> {
        self.events.iter().filter(move |event| event.date == date)
    }

    pub fn count_on(&self, date: CivilDate) -> usize {
        self.on(date).count()
    }

    pub fn in_month(&self, year: i32, month: u32) -> impl Iterator<Item = &Event> {
        self.events
            .iter()
            .filter(move |event| event.date.year == year && event.date.month == month)
    }

    /// Zapis atomowy — przerwany proces nie zostawia pustego pliku.
    ///
    /// Plik dostaje prawa `0600`, bo wydarzenia są danymi prywatnymi.
    pub fn save(&self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        save_atomic_private(path, &self.render())
    }

    pub fn render(&self) -> String {
        let mut out = String::from(
            "# Luna Clock — wydarzenia kalendarza (tylko lokalnie)\n\
             # id<TAB>data<TAB>start<TAB>minuty<TAB>tytuł<TAB>notatka\n\
             # start = - dla wydarzenia całodniowego\n",
        );
        for event in &self.events {
            out.push_str(&event.id.to_string());
            out.push('\t');
            out.push_str(&event.date.to_iso());
            out.push('\t');
            match event.start_minute {
                Some(minute) => out.push_str(&minute.to_string()),
                None => out.push('-'),
            }
            out.push('\t');
            out.push_str(&event.duration_min.to_string());
            out.push('\t');
            out.push_str(&escape(&event.title));
            out.push('\t');
            out.push_str(&escape(&event.note));
            out.push('\n');
        }
        out
    }

    /// Import podstawowych `VEVENT` z pliku iCalendar. Zwraca liczbę nowych wpisów.
    pub fn import_ics(&mut self, text: &str) -> usize {
        let lines = unfold_ics(text);
        let mut added = 0;
        let mut start = None;
        for (index, line) in lines.iter().enumerate() {
            let line = line.trim();
            if line.eq_ignore_ascii_case("BEGIN:VEVENT") {
                start = Some(index);
                continue;
            }
            if !line.eq_ignore_ascii_case("END:VEVENT") {
                continue;
            }
            let Some(begin) = start.take() else {
                continue;
            };
            let Some(parsed) = parse_vevent(&lines[begin..=index]) else {
                continue;
            };
            let duplicate = self.events.iter().any(|event| {
                event.date == parsed.date
                    && event.start_minute == parsed.start_minute
                    && event.title == parsed.title
            });
            if duplicate {
                continue;
            }
            let id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            self.events.push(Event { id, ..parsed });
            added += 1;
        }
        self.events.sort_by_key(event_sort_key);
        added
    }

    pub fn export_ics(&self) -> String {
        let mut out = String::from(
            "BEGIN:VCALENDAR\r\n\
             VERSION:2.0\r\n\
             PRODID:-//Luna//Luna Clock//EN\r\n\
             CALSCALE:GREGORIAN\r\n",
        );
        for event in &self.events {
            out.push_str("BEGIN:VEVENT\r\n");
            out.push_str(&format!("UID:{}@luna-clock\r\n", event.id));
            match event.start_minute {
                None => out.push_str(&format!(
                    "DTSTART;VALUE=DATE:{:04}{:02}{:02}\r\n",
                    event.date.year, event.date.month, event.date.day
                )),
                Some(minute) => {
                    out.push_str(&format!(
                        "DTSTART:{:04}{:02}{:02}T{:02}{:02}00\r\n",
                        event.date.year,
                        event.date.month,
                        event.date.day,
                        minute / 60,
                        minute % 60
                    ));
                    out.push_str(&format!("DURATION:PT{}M\r\n", event.duration_min.max(1)));
                }
            }
            out.push_str(&format!("SUMMARY:{}\r\n", escape_ics(&event.title)));
            if !event.note.is_empty() {
                out.push_str(&format!("DESCRIPTION:{}\r\n", escape_ics(&event.note)));
            }
            out.push_str("END:VEVENT\r\n");
        }
        out.push_str("END:VCALENDAR\r\n");
        out
    }
}

fn event_sort_key(event: &Event) -> (i64, u32, u64) {
    (
        event.date.unix_days(),
        event.start_minute.unwrap_or(0),
        event.id,
    )
}

fn parse_line(line: &str) -> Option<Event> {
    let mut fields = line.split('\t');
    let id = fields.next()?.trim().parse::<u64>().ok()?;
    let date = CivilDate::parse_iso(fields.next()?)?;
    let start = fields.next()?.trim();
    let start_minute = if start == "-" {
        None
    } else {
        let minute = start.parse::<u32>().ok()?;
        if minute > 1_439 {
            return None;
        }
        Some(minute)
    };
    let duration_min = fields.next()?.trim().parse::<u32>().ok()?;
    if duration_min > 10_080 {
        return None;
    }
    let title = unescape(fields.next().unwrap_or(""));
    if title.trim().is_empty() {
        return None;
    }
    let note = unescape(fields.next().unwrap_or(""));
    Some(Event {
        id,
        date,
        start_minute,
        duration_min: if start_minute.is_some() {
            duration_min
        } else {
            0
        },
        title,
        note,
    })
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(ch),
        }
    }
    out
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn unfold_ics(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        let raw = raw.trim_end_matches('\r');
        if let Some(rest) = raw.strip_prefix([' ', '\t']) {
            if let Some(last) = lines.last_mut() {
                last.push_str(rest);
                continue;
            }
        }
        lines.push(raw.to_owned());
    }
    lines
}

fn parse_vevent(lines: &[String]) -> Option<Event> {
    let mut inside = false;
    let mut date: Option<CivilDate> = None;
    let mut start_minute = None;
    let mut end: Option<(CivilDate, u32)> = None;
    let mut duration: Option<i64> = None;
    let mut title = None;
    let mut note = String::new();

    for line in lines {
        let line = line.trim_end();
        if line.eq_ignore_ascii_case("BEGIN:VEVENT") {
            inside = true;
            date = None;
            start_minute = None;
            end = None;
            duration = None;
            title = None;
            note.clear();
            continue;
        }
        if line.eq_ignore_ascii_case("END:VEVENT") {
            if !inside {
                continue;
            }
            let date = date?;
            let title = title
                .map(|value: String| value.trim().to_owned())
                .filter(|value| !value.is_empty())?;
            let duration_min = match (start_minute, end, duration) {
                (None, _, _) => 0,
                (Some(_), _, Some(seconds)) => (seconds / 60).clamp(1, 10_080) as u32,
                (Some(start), Some((end_date, end_minute)), None) => {
                    let start_abs = date.unix_days() * 1_440 + start as i64;
                    let end_abs = end_date.unix_days() * 1_440 + end_minute as i64;
                    ((end_abs - start_abs).max(1)).min(10_080) as u32
                }
                (Some(_), None, None) => 60,
            };
            return Some(Event {
                id: 0,
                date,
                start_minute,
                duration_min,
                title,
                note: note.trim().to_owned(),
            });
        }
        if !inside {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let key = name
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_uppercase();
        match key.as_str() {
            "DTSTART" => {
                if let Some(parsed) = parse_ics_datetime(name, value) {
                    date = Some(parsed.0);
                    start_minute = parsed.1;
                }
            }
            "DTEND" => {
                end = parse_ics_datetime(name, value)
                    .and_then(|(date, minute)| minute.map(|minute| (date, minute)));
            }
            "DURATION" => duration = parse_ics_duration(value),
            "SUMMARY" => title = Some(unescape_ics(value)),
            "DESCRIPTION" => note = unescape_ics(value),
            _ => {}
        }
    }
    None
}

/// Zwraca datę i minutę (albo `None` dla całodniowego).
fn parse_ics_datetime(name: &str, value: &str) -> Option<(CivilDate, Option<u32>)> {
    let raw = value.trim();
    let had_z = raw.ends_with('Z');
    let value = raw.trim_end_matches('Z');
    let date = CivilDate::parse_iso(&format!(
        "{}-{}-{}",
        value.get(0..4)?,
        value.get(4..6)?,
        value.get(6..8)?
    ))?;
    let is_date_only = name.to_ascii_uppercase().contains("VALUE=DATE") || value.len() == 8;
    if is_date_only {
        return Some((date, None));
    }
    let hour = value.get(9..11)?.parse::<u32>().ok()?.min(23);
    let minute = value.get(11..13)?.parse::<u32>().ok()?.min(59);
    let minute = hour * 60 + minute;
    let naive = date.unix_days() * 86_400 + minute as i64 * 60;

    // Wydarzenia z `Z` lub `TZID` sprowadzamy do czasu lokalnego, żeby godzina
    // w kalendarzu zgadzała się z zegarkiem użytkownika.
    if had_z {
        let local = local_at(naive, None);
        return Some((local.date, Some(local.hour * 60 + local.minute)));
    }
    if value.is_ascii() && name.to_ascii_uppercase().contains("TZID=") {
        let tzid = name
            .split(';')
            .find_map(|part| part.trim().strip_prefix("TZID="))
            .unwrap_or_default()
            .trim_matches('"');
        if !tzid.is_empty() {
            // Najpierw przesunięcie w strefie źródłowej, potem czas lokalny użytkownika.
            let offset = local_at(naive, Some(tzid)).offset_seconds as i64;
            let local = local_at(naive - offset, None);
            return Some((local.date, Some(local.hour * 60 + local.minute)));
        }
    }
    Some((date, Some(minute)))
}

fn parse_ics_duration(value: &str) -> Option<i64> {
    let value = value.trim().to_ascii_uppercase();
    let mut total = 0i64;
    let mut number = String::new();
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            number.push(ch);
            continue;
        }
        let n = number.parse::<i64>().ok()?;
        number.clear();
        match ch {
            'W' => total += n * 7 * 86_400,
            'D' => total += n * 86_400,
            'H' => total += n * 3_600,
            'M' => total += n * 60,
            'S' => total += n,
            _ => {}
        }
    }
    (total > 0).then_some(total)
}

fn unescape_ics(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') | Some('N') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn escape_ics(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_store() -> EventStore {
        EventStore {
            events: vec![
                Event {
                    id: 2,
                    date: CivilDate::from_ymd(2026, 10, 31),
                    start_minute: None,
                    duration_min: 0,
                    title: "Urodziny Ali".to_owned(),
                    note: String::new(),
                },
                Event {
                    id: 1,
                    date: CivilDate::from_ymd(2026, 10, 31),
                    start_minute: Some(9 * 60 + 30),
                    duration_min: 45,
                    title: "Lekarz\tbilans".to_owned(),
                    note: "Wziąć\nwyniki".to_owned(),
                },
            ],
            next_id: 3,
            path: None,
        }
    }

    #[test]
    fn escapes_and_roundtrips_events() {
        let store = sample_store();
        let text = store.render();
        let loaded = EventStore::from_text(&text, None);
        assert_eq!(loaded.events(), store.events());
        assert_eq!(loaded.next_id, 3);
        assert!(text.contains("Lekarz\\tbilans"));
        assert!(text.contains("Wziąć\\nwyniki"));
    }

    #[test]
    fn add_update_remove_and_queries() {
        let mut store = EventStore {
            path: None,
            ..EventStore::default()
        };
        let day = CivilDate::from_ymd(2026, 10, 31);
        let first = store.add(day, Some(600), 30, "Spotkanie", "");
        let second = store.add(day, None, 0, "Cały dzień", "");
        assert_eq!(store.count_on(day), 2);
        assert_eq!(store.on(day).next().unwrap().start_minute, None);

        let mut edited = store.get(first).unwrap().clone();
        edited.title = "Spotkanie (nowe)".to_owned();
        assert!(store.update(edited));
        assert_eq!(store.get(first).unwrap().title, "Spotkanie (nowe)");

        assert!(store.remove(second));
        assert_eq!(store.count_on(day), 1);
        assert_eq!(store.in_month(2026, 10).count(), 1);
        assert_eq!(store.in_month(2026, 11).count(), 0);
    }

    #[test]
    fn imports_ics_with_folding_and_tzid() {
        let ics = concat!(
            "BEGIN:VCALENDAR\r\n",
            "BEGIN:VEVENT\r\n",
            "DTSTART;VALUE=DATE:20261224\r\n",
            "SUMMARY:Wigilia\r\n",
            "DESCRIPTION:Długa \r\n",
            " notatka\r\n",
            "END:VEVENT\r\n",
            "BEGIN:VEVENT\r\n",
            "DTSTART;TZID=UTC:20261225T180000\r\n",
            "DTEND;TZID=UTC:20261225T200000\r\n",
            "SUMMARY:Kolacja\r\n",
            "END:VEVENT\r\n",
            "END:VCALENDAR\r\n",
        );
        let mut store = EventStore {
            path: None,
            ..EventStore::default()
        };
        assert_eq!(store.import_ics(ics), 2);
        let all_day = store
            .events()
            .iter()
            .find(|event| event.title == "Wigilia")
            .unwrap();
        assert_eq!(all_day.start_minute, None);
        assert_eq!(all_day.note, "Długa notatka");
        let timed = store
            .events()
            .iter()
            .find(|event| event.title == "Kolacja")
            .unwrap();
        let utc_evening = CivilDate::from_ymd(2026, 12, 25).unix_days() * 86_400 + 18 * 3_600;
        let local = local_at(utc_evening, None);
        assert_eq!(timed.start_minute, Some(local.hour * 60 + local.minute));
        assert_eq!(timed.duration_min, 120);
    }

    #[test]
    fn export_produces_icalendar() {
        let store = sample_store();
        let text = store.export_ics();
        assert!(text.contains("BEGIN:VCALENDAR"));
        assert!(text.contains("DTSTART;VALUE=DATE:20261031"));
        assert!(text.contains("SUMMARY:Urodziny Ali"));
        assert!(text.contains("DURATION:PT45M"));
    }

    #[cfg(unix)]
    #[test]
    fn saved_events_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "luna-clock-store-{}-{}",
            std::process::id(),
            crate::time::now_unix()
        ));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("events.tsv");
        let store = EventStore {
            events: sample_store().events,
            next_id: 3,
            path: Some(path.clone()),
        };
        store.save().unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "wydarzenia muszą być prywatne");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_bad_lines() {
        let store = EventStore::from_text(
            "1\t2026-13-01\t-\t0\tZły miesiąc\t\n\
             2\t2026-10-31\t9999\t0\tZła minuta\t\n\
             3\t2026-10-31\t-\t0\t\t\n",
            None,
        );
        assert!(store.is_empty());
    }
}
