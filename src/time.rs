//! Czas cywilny i strefy czasowe.
//!
//! Matematyka kalendarza jest czysta (algorytm Howarda Hinnanta). Czas lokalny
//! w dowolnej strefie IANA czytamy przez `localtime_r` z libc: ustawiamy `TZ`
//! na czas jednego zapytania i przywracamy poprzednią wartość. Dzięki temu
//! strefy mają poprawne DST, a nie trzeba wozić własnej kopii bazy `tzdata`.

use std::ffi::{CStr, CString, OsStr};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

// `tzset` nie jest eksportowane przez każdą wersję crate'a `libc`, a jest
// potrzebne po zmianie `TZ` (POSIX: `localtime_r` samo nie odświeża strefy).
extern "C" {
    fn tzset();
}

/// Sekund w dobie.
pub const SECONDS_PER_DAY: i64 = 86_400;

/// Data w kalendarzu gregoriańskim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct CivilDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl CivilDate {
    pub const fn from_ymd(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }

    /// Data z liczby dni od 1970-01-01.
    pub fn from_unix_days(days: i64) -> Self {
        let (year, month, day) = civil_from_days(days);
        Self { year, month, day }
    }

    /// Liczba dni od 1970-01-01 (ujemna dla dat wcześniejszych).
    pub fn unix_days(self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    /// Dzień tygodnia: `0` = poniedziałek … `6` = niedziela.
    pub fn weekday(self) -> u32 {
        ((self.unix_days() + 3).rem_euclid(7)) as u32
    }

    pub fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    pub fn days_in_month(year: i32, month: u32) -> u32 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if Self::is_leap_year(year) {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }

    pub fn days_in_its_month(self) -> u32 {
        Self::days_in_month(self.year, self.month)
    }

    pub fn is_valid(self) -> bool {
        (1..=12).contains(&self.month)
            && self.day >= 1
            && self.day <= Self::days_in_month(self.year, self.month)
    }

    pub fn add_days(self, delta: i64) -> Self {
        Self::from_unix_days(self.unix_days().saturating_add(delta))
    }

    /// Najbliższy poniedziałek na początku tygodnia zawierającego tę datę.
    pub fn start_of_week(self) -> Self {
        self.add_days(-(self.weekday() as i64))
    }

    /// ISO 8601: `RRRR-MM-DD`.
    pub fn to_iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Parsuje ścisłe `RRRR-MM-DD` (bez godzin i stref).
    pub fn parse_iso(text: &str) -> Option<Self> {
        let text = text.trim();
        let bytes = text.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        if !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| matches!(i, 4 | 7) || b.is_ascii_digit())
        {
            return None;
        }
        let year = text[0..4].parse::<i32>().ok()?;
        let month = text[5..7].parse::<u32>().ok()?;
        let day = text[8..10].parse::<u32>().ok()?;
        let date = Self { year, month, day };
        date.is_valid().then_some(date)
    }
}

/// Czas lokalny w wybranej strefie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTime {
    pub date: CivilDate,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    /// `0` = poniedziałek … `6` = niedziela.
    pub weekday: u32,
    /// Przesunięcie względem UTC w sekundach (uwzględnia DST).
    pub offset_seconds: i32,
    /// Skrót strefy (np. `CET`), jeśli system go podaje.
    pub abbr: String,
}

impl LocalTime {
    pub fn minute_of_day(&self) -> u32 {
        self.hour * 60 + self.minute
    }

    pub fn second_of_day(&self) -> u32 {
        self.hour * 3600 + self.minute * 60 + self.second
    }

    /// `HH:MM` lub `HH:MM:SS`; w trybie 12-godzinnym z `AM`/`PM`.
    pub fn format_clock(&self, use_24h: bool, show_seconds: bool) -> String {
        if use_24h {
            if show_seconds {
                format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
            } else {
                format!("{:02}:{:02}", self.hour, self.minute)
            }
        } else {
            let suffix = if self.hour < 12 { "AM" } else { "PM" };
            let hour = match self.hour % 12 {
                0 => 12,
                h => h,
            };
            if show_seconds {
                format!("{hour}:{:02}:{:02} {suffix}", self.minute, self.second)
            } else {
                format!("{hour}:{:02} {suffix}", self.minute)
            }
        }
    }

    /// `UTC+02:00` / `UTC-05:00` / `UTC`.
    pub fn offset_label(&self) -> String {
        format_offset(self.offset_seconds)
    }
}

/// Formatuje przesunięcie strefy jako `UTC±HH:MM`.
pub fn format_offset(offset_seconds: i32) -> String {
    if offset_seconds == 0 {
        return "UTC".to_owned();
    }
    let sign = if offset_seconds < 0 { '-' } else { '+' };
    let total = offset_seconds.unsigned_abs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    format!("UTC{sign}{hours:02}:{minutes:02}")
}

/// Bieżący czas uniksowy (sekundy).
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Bieżący czas lokalny w strefie procesu.
pub fn local_now() -> LocalTime {
    local_at(now_unix(), None)
}

/// Czas lokalny w strefie `zone` (IANA). `None` = strefa procesu.
pub fn local_now_in(zone: Option<&str>) -> LocalTime {
    local_at(now_unix(), zone)
}

/// Czas lokalny dla znacznika `unix` w strefie `zone` (IANA).
pub fn local_at(unix: i64, zone: Option<&str>) -> LocalTime {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let previous = std::env::var_os("TZ");
    let changed = zone.is_some();
    if let Some(zone) = zone {
        set_tz(Some(OsStr::new(zone)));
    }

    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let time = unix as libc::time_t;
    let ok = unsafe { !libc::localtime_r(&time, &mut tm).is_null() };

    let result = if ok {
        let date = CivilDate::from_ymd(
            tm.tm_year + 1900,
            (tm.tm_mon + 1).clamp(1, 12) as u32,
            tm.tm_mday.clamp(1, 31) as u32,
        );
        let abbr = if tm.tm_zone.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(tm.tm_zone) }
                .to_string_lossy()
                .into_owned()
        };
        LocalTime {
            date,
            hour: tm.tm_hour.clamp(0, 23) as u32,
            minute: tm.tm_min.clamp(0, 59) as u32,
            second: tm.tm_sec.clamp(0, 60) as u32,
            weekday: ((tm.tm_wday as i32 + 6).rem_euclid(7)) as u32,
            offset_seconds: tm.tm_gmtoff as i32,
            abbr,
        }
    } else {
        // `localtime_r` zawodzi tylko przy uszkodzonej strefie — pokaż UTC.
        LocalTime {
            date: CivilDate::from_unix_days(unix.div_euclid(SECONDS_PER_DAY)),
            hour: (unix.rem_euclid(SECONDS_PER_DAY) / 3600) as u32,
            minute: ((unix.rem_euclid(SECONDS_PER_DAY) % 3600) / 60) as u32,
            second: (unix.rem_euclid(SECONDS_PER_DAY) % 60) as u32,
            weekday: CivilDate::from_ymd(1970, 1, 1)
                .add_days(unix.div_euclid(SECONDS_PER_DAY))
                .weekday(),
            offset_seconds: 0,
            abbr: "UTC".to_owned(),
        }
    };

    if changed {
        set_tz(previous.as_deref());
    }
    result
}

fn set_tz(zone: Option<&OsStr>) -> bool {
    let key = b"TZ\0";
    let ok = match zone {
        Some(zone) => match zone.to_str().and_then(|zone| CString::new(zone).ok()) {
            Some(value) => unsafe { libc::setenv(key.as_ptr().cast(), value.as_ptr(), 1) == 0 },
            None => false,
        },
        None => unsafe { libc::unsetenv(key.as_ptr().cast()) == 0 },
    };
    unsafe { tzset() };
    ok
}

/// Nazwa miesiąca jako klucz i18n (`tr` zamieni ją na język interfejsu).
pub fn month_name_key(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        _ => "December",
    }
}

/// Nazwa dnia tygodnia jako klucz i18n (`0` = poniedziałek).
pub fn weekday_name_key(weekday: u32) -> &'static str {
    match weekday % 7 {
        0 => "Monday",
        1 => "Tuesday",
        2 => "Wednesday",
        3 => "Thursday",
        4 => "Friday",
        5 => "Saturday",
        _ => "Sunday",
    }
}

/// Skrót dnia tygodnia (kolumna kalendarza).
pub fn weekday_short_key(weekday: u32) -> &'static str {
    match weekday % 7 {
        0 => "Mon",
        1 => "Tue",
        2 => "Wed",
        3 => "Thu",
        4 => "Fri",
        5 => "Sat",
        _ => "Sun",
    }
}

pub fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let m = month as i64;
    let d = day as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if m <= 2 {
        y += 1;
    }
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrips_for_known_dates() {
        for (y, m, d) in [
            (1970, 1, 1),
            (2000, 2, 29),
            (2024, 2, 29),
            (2026, 10, 31),
            (1900, 3, 1),
            (2038, 1, 19),
        ] {
            let days = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(days), (y, m, d));
            assert_eq!(
                CivilDate::from_unix_days(days).to_iso(),
                format!("{y:04}-{m:02}-{d:02}")
            );
        }
    }

    #[test]
    fn weekdays_and_iso() {
        assert_eq!(CivilDate::from_ymd(1970, 1, 1).weekday(), 3); // czwartek
        assert_eq!(CivilDate::from_ymd(2024, 1, 1).weekday(), 0); // poniedziałek
        assert_eq!(CivilDate::from_ymd(2026, 10, 31).to_iso(), "2026-10-31");
        assert_eq!(
            CivilDate::parse_iso("2026-10-31"),
            Some(CivilDate::from_ymd(2026, 10, 31))
        );
        assert_eq!(CivilDate::parse_iso("2026-02-30"), None);
        assert_eq!(CivilDate::parse_iso("31.10.2026"), None);
    }

    #[test]
    fn month_lengths() {
        assert_eq!(CivilDate::days_in_month(2024, 2), 29);
        assert_eq!(CivilDate::days_in_month(2023, 2), 28);
        assert_eq!(CivilDate::days_in_month(2026, 10), 31);
        assert_eq!(CivilDate::days_in_month(2026, 4), 30);
    }

    #[test]
    fn utc_conversion_uses_localtime_r() {
        let local = local_at(0, Some("UTC"));
        assert_eq!(local.date, CivilDate::from_ymd(1970, 1, 1));
        assert_eq!((local.hour, local.minute, local.second), (0, 0, 0));
        assert_eq!(local.offset_seconds, 0);
        assert_eq!(local.offset_label(), "UTC");
    }

    #[test]
    fn warsaw_observes_summer_time_when_zoneinfo_exists() {
        if !std::path::Path::new("/usr/share/zoneinfo/Europe/Warsaw").exists() {
            return;
        }
        // 2024-07-01 12:00 UTC = 14:00 CEST.
        let local = local_at(1_719_835_200, Some("Europe/Warsaw"));
        assert_eq!(local.date, CivilDate::from_ymd(2024, 7, 1));
        assert_eq!(local.hour, 14);
        assert_eq!(local.offset_seconds, 7200);
        assert_eq!(local.format_clock(true, false), "14:00");
        assert_eq!(local.format_clock(false, false), "2:00 PM");
    }

    #[test]
    fn offset_labels() {
        assert_eq!(format_offset(0), "UTC");
        assert_eq!(format_offset(3600), "UTC+01:00");
        assert_eq!(format_offset(-19_800), "UTC-05:30");
        assert_eq!(format_offset(19_800), "UTC+05:30");
    }
}
