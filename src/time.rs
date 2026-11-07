//! Czas cywilny i strefy czasowe.
//!
//! Matematyka kalendarza jest czysta (algorytm Howarda Hinnanta). Czas lokalny
//! w dowolnej strefie IANA czytamy przez `localtime_r` z libc: ustawiamy `TZ`
//! na czas jednego zapytania i przywracamy poprzednią wartość. Dzięki temu
//! strefy mają poprawne DST, a nie trzeba wozić własnej kopii bazy `tzdata`.

use std::ffi::{CStr, CString, OsStr};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub use libluna_core::civil::{civil_from_days, days_from_civil, CivilDate, SECONDS_PER_DAY};

// `tzset` nie jest eksportowane przez każdą wersję crate'a `libc`, a jest
// potrzebne po zmianie `TZ` (POSIX: `localtime_r` samo nie odświeża strefy).
extern "C" {
    fn tzset();
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

/// Czy pokazywać sam dzień i miesiąc, czy też rok.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateFields {
    /// Dzień i miesiąc (bez roku).
    DayMonth,
    /// Dzień, miesiąc i rok.
    #[default]
    DayMonthYear,
}

/// Czy miesiąc zapisywać cyframi, czy nazwą.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateStyle {
    /// `10.10.2026`
    #[default]
    Numeric,
    /// `10 października 2026`
    Words,
}

/// Kolejność dnia i miesiąca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateOrder {
    /// Dzień → miesiąc (`10.10.2026`).
    #[default]
    DayFirst,
    /// Miesiąc → dzień, jak w zapisie angielskim (`10/31/2026`).
    MonthFirst,
}

/// Separator w formacie cyfrowym.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateSeparator {
    #[default]
    Dot,
    Dash,
    Slash,
}

impl DateSeparator {
    pub fn ch(self) -> char {
        match self {
            DateSeparator::Dot => '.',
            DateSeparator::Dash => '-',
            DateSeparator::Slash => '/',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DateSeparator::Dot => ".",
            DateSeparator::Dash => "-",
            DateSeparator::Slash => "/",
        }
    }

    pub fn next(self) -> Self {
        match self {
            DateSeparator::Dot => DateSeparator::Dash,
            DateSeparator::Dash => DateSeparator::Slash,
            DateSeparator::Slash => DateSeparator::Dot,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            DateSeparator::Dot => "dot",
            DateSeparator::Dash => "dash",
            DateSeparator::Slash => "slash",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value.trim() {
            "dot" => Some(DateSeparator::Dot),
            "dash" => Some(DateSeparator::Dash),
            "slash" => Some(DateSeparator::Slash),
            _ => None,
        }
    }
}

impl DateFields {
    pub fn label(self) -> String {
        match self {
            DateFields::DayMonth => libluna_i18n::tr("Day and month"),
            DateFields::DayMonthYear => libluna_i18n::tr("Day, month and year"),
        }
    }

    pub fn next(self) -> Self {
        match self {
            DateFields::DayMonth => DateFields::DayMonthYear,
            DateFields::DayMonthYear => DateFields::DayMonth,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            DateFields::DayMonth => "day_month",
            DateFields::DayMonthYear => "day_month_year",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value.trim() {
            "day_month" => Some(DateFields::DayMonth),
            "day_month_year" => Some(DateFields::DayMonthYear),
            _ => None,
        }
    }
}

impl DateStyle {
    pub fn label(self) -> String {
        match self {
            DateStyle::Numeric => libluna_i18n::tr("Digits"),
            DateStyle::Words => libluna_i18n::tr("Words"),
        }
    }

    pub fn next(self) -> Self {
        match self {
            DateStyle::Numeric => DateStyle::Words,
            DateStyle::Words => DateStyle::Numeric,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            DateStyle::Numeric => "numeric",
            DateStyle::Words => "words",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value.trim() {
            "numeric" => Some(DateStyle::Numeric),
            "words" => Some(DateStyle::Words),
            _ => None,
        }
    }
}

impl DateOrder {
    pub fn label(self) -> String {
        match self {
            DateOrder::DayFirst => libluna_i18n::tr("Day then month"),
            DateOrder::MonthFirst => libluna_i18n::tr("Month then day"),
        }
    }

    pub fn next(self) -> Self {
        match self {
            DateOrder::DayFirst => DateOrder::MonthFirst,
            DateOrder::MonthFirst => DateOrder::DayFirst,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            DateOrder::DayFirst => "day_first",
            DateOrder::MonthFirst => "month_first",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value.trim() {
            "day_first" => Some(DateOrder::DayFirst),
            "month_first" => Some(DateOrder::MonthFirst),
            _ => None,
        }
    }
}

/// Ustawienia formatu daty (wspólne dla nagłówka, kalendarza i list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DateFormat {
    pub fields: DateFields,
    pub style: DateStyle,
    pub order: DateOrder,
    pub separator: DateSeparator,
}

impl DateFormat {
    /// Data bez dnia tygodnia (np. do listy świąt).
    pub fn date(&self, date: CivilDate) -> String {
        match self.style {
            DateStyle::Numeric => {
                let day = format!("{:02}", date.day);
                let month = format!("{:02}", date.month);
                let sep = self.separator.ch();
                let body = if self.order == DateOrder::MonthFirst {
                    format!("{month}{sep}{day}")
                } else {
                    format!("{day}{sep}{month}")
                };
                if self.fields == DateFields::DayMonthYear {
                    format!("{body}{sep}{}", date.year)
                } else {
                    body
                }
            }
            DateStyle::Words => {
                let month = libluna_i18n::tr(month_name_key(date.month));
                let body = if self.order == DateOrder::MonthFirst {
                    format!("{month} {}", date.day)
                } else {
                    format!("{} {month}", date.day)
                };
                if self.fields == DateFields::DayMonthYear {
                    if self.order == DateOrder::MonthFirst {
                        format!("{body}, {}", date.year)
                    } else {
                        format!("{body} {}", date.year)
                    }
                } else {
                    body
                }
            }
        }
    }

    /// Data z dniem tygodnia (nagłówek zegara, szczegóły dnia).
    pub fn full(&self, date: CivilDate) -> String {
        format!(
            "{}, {}",
            libluna_i18n::tr(weekday_name_key(date.weekday())),
            self.date(date)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_formats_follow_preferences() {
        let date = CivilDate::from_ymd(2026, 10, 31);
        let mut format = DateFormat::default();
        assert_eq!(format.date(date), "31.10.2026");

        format.fields = DateFields::DayMonth;
        assert_eq!(format.date(date), "31.10");

        format.style = DateStyle::Words;
        let october = libluna_i18n::tr(month_name_key(10));
        assert_eq!(format.date(date), format!("31 {october}"));

        format.fields = DateFields::DayMonthYear;
        format.order = DateOrder::MonthFirst;
        assert_eq!(format.date(date), format!("{october} 31, 2026"));

        format.style = DateStyle::Numeric;
        format.separator = DateSeparator::Slash;
        assert_eq!(format.date(date), "10/31/2026");
        format.order = DateOrder::DayFirst;
        assert_eq!(format.date(date), "31/10/2026");
        format.separator = DateSeparator::Dash;
        assert_eq!(format.date(date), "31-10-2026");

        // Dzień tygodnia jest dodawany tylko w wariancie `full`.
        assert!(format.full(date).starts_with(&libluna_i18n::tr("Saturday")));
    }

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
