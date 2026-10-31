//! Stan aplikacji i pętla klatki.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use libluna_core::clock_state::{ClockActivity, StopwatchActivity, TimerActivity};
use libluna_core::{Point, Rect, ThemeSettings};
use libluna_i18n::{load_catalogs_from_dirs, preferred_locale, set_locale, tr};
use libluna_platform::{run as run_app, AppConfig, Frame};
use libluna_widgets::{Calendar, Entry, Toast, WindowControl};

use crate::holidays;
use crate::prefs::{self, Prefs};
use crate::store::{Event, EventStore};
use crate::time::{local_at, now_unix, CivilDate, LocalTime};

/// Strona aplikacji.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Clock,
    Timer,
    Stopwatch,
    Calendar,
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Clock,
        Tab::Timer,
        Tab::Stopwatch,
        Tab::Calendar,
        Tab::Settings,
    ];

    pub const fn title_key(self) -> &'static str {
        match self {
            Tab::Clock => "Clock",
            Tab::Timer => "Timer",
            Tab::Stopwatch => "Stopwatch",
            Tab::Calendar => "Calendar",
            Tab::Settings => "Settings",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|tab| *tab == self).unwrap_or(0)
    }

    pub fn from_index(index: usize) -> Tab {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    pub fn from_name(name: &str) -> Option<Tab> {
        match name.trim().to_ascii_lowercase().as_str() {
            "clock" => Some(Tab::Clock),
            "timer" => Some(Tab::Timer),
            "stopwatch" => Some(Tab::Stopwatch),
            "calendar" => Some(Tab::Calendar),
            "settings" => Some(Tab::Settings),
            _ => None,
        }
    }
}

/// Pole formularza wydarzenia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Title,
    Time,
    Note,
}

/// Formularz dodawania/edycji wydarzenia.
#[derive(Clone)]
pub struct EventForm {
    pub editing: Option<u64>,
    pub date: CivilDate,
    pub title: Entry,
    pub time: Entry,
    pub note: Entry,
    pub duration_index: usize,
    pub focus: FormField,
    pub error: Option<&'static str>,
}

impl EventForm {
    pub fn new(date: CivilDate) -> Self {
        let mut title = Entry::new(tr("Event title"));
        title.set_focused(true);
        Self {
            editing: None,
            date,
            title,
            time: Entry::new("HH:MM"),
            note: Entry::new(tr("Note (optional)")),
            duration_index: 1,
            focus: FormField::Title,
            error: None,
        }
    }

    pub fn from_event(event: &Event) -> Self {
        let mut title = Entry::new(tr("Event title"));
        title.set_text(&event.title);
        title.set_focused(true);
        let mut time = Entry::new("HH:MM");
        if let Some(minute) = event.start_minute {
            time.set_text(format!("{:02}:{:02}", minute / 60, minute % 60));
        }
        let mut note = Entry::new(tr("Note (optional)"));
        note.set_text(&event.note);
        let duration_index = DURATION_OPTIONS
            .iter()
            .position(|value| *value == event.duration_min)
            .unwrap_or(1);
        Self {
            editing: Some(event.id),
            date: event.date,
            title,
            time,
            note,
            duration_index,
            focus: FormField::Title,
            error: None,
        }
    }

    pub fn duration(&self) -> u32 {
        DURATION_OPTIONS[self.duration_index.min(DURATION_OPTIONS.len() - 1)]
    }

    pub fn focus_next(&mut self, backwards: bool) {
        self.focus = match (self.focus, backwards) {
            (FormField::Title, false) | (FormField::Note, true) => FormField::Time,
            (FormField::Time, false) | (FormField::Title, true) => FormField::Note,
            (FormField::Note, false) | (FormField::Time, true) => FormField::Title,
        };
        self.title.set_focused(self.focus == FormField::Title);
        self.time.set_focused(self.focus == FormField::Time);
        self.note.set_focused(self.focus == FormField::Note);
    }

    pub fn entry_mut(&mut self, field: FormField) -> &mut Entry {
        match field {
            FormField::Title => &mut self.title,
            FormField::Time => &mut self.time,
            FormField::Note => &mut self.note,
        }
    }

    /// Pole, na którym aktualnie stoi kursor (bez podwójnego wypożyczenia `self`).
    pub fn focused_mut(&mut self) -> &mut Entry {
        let field = self.focus;
        self.entry_mut(field)
    }
}

/// Dostępne czasy trwania wydarzenia (minuty).
pub const DURATION_OPTIONS: [u32; 10] = [15, 30, 45, 60, 90, 120, 180, 240, 480, 1440];

/// Rodzaj wyboru w modalnej liście.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKind {
    Zone,
    Country,
}

/// Wyszukiwana lista (strefy czasowe albo kraje).
#[derive(Clone)]
pub struct Picker {
    pub kind: PickerKind,
    pub query: Entry,
    pub selected: usize,
}

impl Picker {
    pub fn new(kind: PickerKind) -> Self {
        let placeholder = match kind {
            PickerKind::Zone => tr("Search time zones"),
            PickerKind::Country => tr("Search countries"),
        };
        let mut query = Entry::new(placeholder)
            .leading(libluna_widgets::EntryIcon::Search)
            .clearable(true);
        query.set_focused(true);
        Self {
            kind,
            query,
            selected: 0,
        }
    }
}

/// Formularz importu pliku `.ics`.
#[derive(Clone)]
pub struct ImportForm {
    pub path: Entry,
    pub error: Option<&'static str>,
}

impl ImportForm {
    pub fn new() -> Self {
        let mut path = Entry::new("~/calendar.ics");
        path.set_focused(true);
        Self { path, error: None }
    }
}

/// Jednostka regulacji minutnika.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerUnit {
    Hours,
    Minutes,
    Seconds,
}

/// Stan minutnika.
#[derive(Debug, Clone)]
pub struct TimerState {
    pub duration: Duration,
    pub remaining: Duration,
    pub running: bool,
    pub deadline: Option<Instant>,
    pub finished: bool,
}

impl TimerState {
    pub fn new(minutes: u32) -> Self {
        let duration = Duration::from_secs((minutes.clamp(1, 24 * 60) as u64) * 60);
        Self {
            duration,
            remaining: duration,
            running: false,
            deadline: None,
            finished: false,
        }
    }

    pub fn set_duration(&mut self, duration: Duration) {
        let duration = duration.clamp(Duration::from_secs(1), Duration::from_secs(86_400));
        self.duration = duration;
        self.remaining = duration;
        self.running = false;
        self.deadline = None;
        self.finished = false;
    }

    pub fn adjust(&mut self, unit: TimerUnit, delta: i64) {
        let step = match unit {
            TimerUnit::Hours => 3_600,
            TimerUnit::Minutes => 60,
            TimerUnit::Seconds => 1,
        };
        let seconds = (self.duration.as_secs() as i64 + delta * step).clamp(1, 86_400) as u64;
        self.set_duration(Duration::from_secs(seconds));
    }

    pub fn start_pause(&mut self) {
        if self.running {
            self.remaining = self.remaining();
            self.running = false;
            self.deadline = None;
            return;
        }
        if self.remaining.is_zero() {
            self.remaining = self.duration;
        }
        self.finished = false;
        self.running = true;
        self.deadline = Some(Instant::now() + self.remaining);
    }

    pub fn reset(&mut self) {
        self.running = false;
        self.deadline = None;
        self.remaining = self.duration;
        self.finished = false;
    }

    pub fn remaining(&self) -> Duration {
        match self.deadline {
            Some(deadline) if self.running => deadline.saturating_duration_since(Instant::now()),
            _ => self.remaining,
        }
    }

    pub fn progress(&self) -> f64 {
        let total = self.duration.as_secs_f64().max(0.001);
        1.0 - (self.remaining().as_secs_f64() / total).clamp(0.0, 1.0)
    }

    /// Aktualizuje stan po czasie; zwraca `true`, gdy minutnik właśnie skończył.
    pub fn tick(&mut self) -> bool {
        if !self.running {
            return false;
        }
        let remaining = self.remaining();
        if remaining.is_zero() {
            self.running = false;
            self.deadline = None;
            self.remaining = Duration::ZERO;
            self.finished = true;
            return true;
        }
        false
    }
}

/// Jedno okrążenie stopera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lap {
    pub total: Duration,
    pub delta: Duration,
}

/// Stoper.
#[derive(Debug, Clone, Default)]
pub struct Stopwatch {
    pub running: bool,
    started: Option<Instant>,
    accumulated: Duration,
    pub laps: Vec<Lap>,
}

impl Stopwatch {
    pub fn start_pause(&mut self) {
        if self.running {
            self.accumulated = self.elapsed();
            self.started = None;
            self.running = false;
        } else {
            self.started = Some(Instant::now());
            self.running = true;
        }
    }

    pub fn reset(&mut self) {
        self.running = false;
        self.started = None;
        self.accumulated = Duration::ZERO;
        self.laps.clear();
    }

    pub fn elapsed(&self) -> Duration {
        match self.started {
            Some(start) if self.running => self.accumulated + start.elapsed(),
            _ => self.accumulated,
        }
    }

    pub fn lap(&mut self) {
        let total = self.elapsed();
        let previous = self.laps.last().map(|lap| lap.total).unwrap_or_default();
        self.laps.push(Lap {
            total,
            delta: total.saturating_sub(previous),
        });
    }

    pub fn is_empty(&self) -> bool {
        !self.running && self.accumulated.is_zero() && self.laps.is_empty()
    }
}

/// Akcja interfejsu zarejestrowana podczas rysowania klatki.
///
/// Widgety są „immediate mode”: zbieramy prostokąty i akcje w jednym miejscu,
/// więc obsługa kliknięć nie musi powtarzać całego układu strony.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Window(WindowControl),
    Tab(Tab),
    AddZone,
    RemoveZone(usize),
    TimerUnit(TimerUnit, i64),
    TimerPreset(u32),
    TimerToggle,
    TimerReset,
    WatchToggle,
    WatchLap,
    WatchReset,
    MonthPrev,
    MonthNext,
    Today,
    Day(u32),
    NewEvent,
    EditEvent(u64),
    DeleteEvent(u64),
    FormSave,
    FormCancel,
    FormFocus(FormField),
    FormDuration(i64),
    PickerApply(usize),
    PickerCancel,
    ImportOpen,
    ImportSubmit,
    ImportCancel,
    AutoTheme,
    ApplyTheme,
    ShowHolidays,
    ShowSeconds,
    Use24h,
    DarkHour(i64),
    LightHour(i64),
    PickCountry,
    DetectCountry,
    ExportIcs,
}

/// Stan odświeżania po klatce.
pub struct Update {
    pub animating: bool,
    pub delay: Option<Duration>,
}

/// Aplikacja zegara i kalendarza.
pub struct App {
    pub tab: Tab,
    pub prefs: Prefs,
    pub events: EventStore,
    pub now: LocalTime,
    pub now_unix: i64,
    pub zone_times: Vec<(String, LocalTime)>,
    pub timer: TimerState,
    pub stopwatch: Stopwatch,
    pub calendar: Calendar,
    pub selected_date: CivilDate,
    pub form: Option<EventForm>,
    pub picker: Option<Picker>,
    pub importer: Option<ImportForm>,
    pub zones_all: Vec<String>,
    pub detected_country: Option<&'static str>,
    pub pointer: Option<Point>,
    pub controls_hover: Option<WindowControl>,
    pub pending_close: bool,
    pub pending_minimize: bool,
    pub pending_maximize: bool,
    pub pending_move: bool,
    pub toast: Toast,
    toast_until: Option<Instant>,
    pub clipboard_text: Option<String>,
    pub pending_paste: bool,
    /// Prostokąty i akcje z ostatniej klatki (kliknięcia trafiają w nie).
    pub hits: Vec<(Action, Rect)>,
    /// Prostokąt otwartego okna modalnego (klik poza zamyka).
    pub modal_rect: Option<Rect>,
    /// Ostatnio zastosowany cel motywu automatycznego.
    theme_target: Option<bool>,
}

impl App {
    pub fn new(
        prefs: Prefs,
        events: EventStore,
        start_date: Option<CivilDate>,
        activity: Option<ClockActivity>,
    ) -> Self {
        let now = local_at(now_unix(), None);
        let selected = start_date.unwrap_or(now.date);
        let mut calendar = Calendar::new(selected.year, selected.month);
        calendar.set_selected(selected.day);
        let timer = TimerState::new(prefs.timer_minutes);
        let zones_all = libluna_core::region::system_timezones();
        let mut prefs = prefs;
        if prefs.world_clocks.is_empty() {
            prefs.world_clocks = prefs::default_world_clocks();
        }
        let mut app = Self {
            tab: if start_date.is_some() {
                Tab::Calendar
            } else {
                Tab::Clock
            },
            prefs,
            events,
            now,
            now_unix: now_unix(),
            zone_times: Vec::new(),
            timer,
            stopwatch: Stopwatch::default(),
            calendar,
            selected_date: selected,
            form: None,
            picker: None,
            importer: None,
            zones_all,
            detected_country: holidays::detect_country(),
            pointer: None,
            controls_hover: None,
            pending_close: false,
            pending_minimize: false,
            pending_maximize: false,
            pending_move: false,
            toast: Toast::default(),
            toast_until: None,
            clipboard_text: None,
            pending_paste: false,
            hits: Vec::new(),
            modal_rect: None,
            theme_target: Some(libluna_core::appearance::system_dark()),
        };
        if let Some(activity) = activity {
            app.restore_activity(&activity);
        }
        app.refresh_clock();
        app
    }

    /// Odtwarza minutnik/stoper po ponownym otwarciu aplikacji.
    pub fn restore_activity(&mut self, activity: &ClockActivity) {
        let now = libluna_core::clock_state::now_unix();
        if let Some(timer) = &activity.timer {
            if timer.is_active(now) {
                let remaining = timer.remaining_at(now);
                self.timer.duration = Duration::from_secs(timer.total_secs.max(1));
                if remaining > 0 && timer.running {
                    self.timer.remaining = Duration::from_secs(remaining);
                    self.timer.running = true;
                    self.timer.deadline = Some(Instant::now() + Duration::from_secs(remaining));
                    self.timer.finished = false;
                } else if remaining == 0 && timer.running {
                    self.timer.remaining = Duration::ZERO;
                    self.timer.running = false;
                    self.timer.deadline = None;
                    self.timer.finished = true;
                } else {
                    self.timer.remaining =
                        Duration::from_secs(timer.remaining_secs.min(timer.total_secs));
                    self.timer.running = false;
                    self.timer.deadline = None;
                    self.timer.finished = timer.finished;
                }
            }
        }
        if let Some(stopwatch) = &activity.stopwatch {
            if stopwatch.is_active(now) {
                let mut previous = Duration::ZERO;
                let laps = stopwatch
                    .laps
                    .iter()
                    .map(|total| {
                        let total = Duration::from_millis(*total);
                        let lap = Lap {
                            total,
                            delta: total.saturating_sub(previous),
                        };
                        previous = total;
                        lap
                    })
                    .collect();
                self.stopwatch = Stopwatch {
                    running: stopwatch.running,
                    started: stopwatch.running.then(Instant::now),
                    accumulated: Duration::from_millis(stopwatch.elapsed_at(now)),
                    laps,
                };
            }
        }
    }

    /// Stan minutnika do zapisania (tylko gdy coś odlicza albo czeka).
    pub fn timer_activity(&self) -> Option<TimerActivity> {
        if self.timer.finished {
            // Zakończony minutnik nie jest już widgetem — jego ślad zostaje w powiadomieniach.
            return None;
        }
        let active = self.timer.running || self.timer.remaining() < self.timer.duration;
        if !active {
            return None;
        }
        let remaining = self.timer.remaining();
        let remaining_secs = ((remaining.as_millis() + 999) / 1000) as u64;
        let now = libluna_core::clock_state::now_unix();
        Some(TimerActivity {
            total_secs: self.timer.duration.as_secs().max(1),
            running: self.timer.running,
            end_unix: if self.timer.running {
                now + remaining_secs as i64
            } else {
                0
            },
            remaining_secs,
            finished: self.timer.finished,
        })
    }

    pub fn stopwatch_activity(&self) -> Option<StopwatchActivity> {
        if self.stopwatch.is_empty() {
            return None;
        }
        Some(StopwatchActivity {
            elapsed_millis: self.stopwatch.elapsed().as_millis() as u64,
            running: self.stopwatch.running,
            updated_unix: libluna_core::clock_state::now_unix(),
            laps: self
                .stopwatch
                .laps
                .iter()
                .map(|lap| lap.total.as_millis() as u64)
                .collect(),
        })
    }

    pub fn clock_activity(&self, with_pid: bool) -> ClockActivity {
        ClockActivity {
            pid: with_pid.then(std::process::id),
            updated_unix: libluna_core::clock_state::now_unix(),
            timer: self.timer_activity(),
            stopwatch: self.stopwatch_activity(),
        }
    }

    /// Zapisuje stan dla panelu. `with_pid=false` wywołujemy przed zamknięciem okna.
    pub fn persist_activity(&self, with_pid: bool) {
        let _ = self.clock_activity(with_pid).save();
    }

    pub fn window_title(&self) -> String {
        tr("Clock")
    }

    pub fn country_code(&self) -> &str {
        if !self.prefs.country.is_empty() {
            &self.prefs.country
        } else {
            self.detected_country.unwrap_or("")
        }
    }

    pub fn country_label(&self) -> String {
        let code = self.country_code();
        if code.is_empty() {
            return tr("Not selected");
        }
        format!("{} ({code})", tr(holidays::country_name(code)))
    }

    pub fn holidays_for(&self, date: CivilDate) -> Vec<&'static str> {
        if !self.prefs.show_holidays {
            return Vec::new();
        }
        let country = self.country_code();
        if country.is_empty() {
            return Vec::new();
        }
        holidays::holidays_on(country, date)
    }

    pub fn holiday_dates(&self, year: i32, month: u32) -> Vec<CivilDate> {
        if !self.prefs.show_holidays || self.country_code().is_empty() {
            return Vec::new();
        }
        holidays::holidays_in_month(self.country_code(), year, month)
            .into_iter()
            .map(|holiday| holiday.date)
            .collect()
    }

    pub fn events_on(&self, date: CivilDate) -> Vec<&Event> {
        self.events.on(date).collect()
    }

    pub fn refresh_clock(&mut self) {
        let unix = now_unix();
        self.now_unix = unix;
        self.now = local_at(unix, None);
        self.zone_times = self
            .prefs
            .world_clocks
            .iter()
            .map(|zone| (zone.clone(), local_at(unix, Some(zone))))
            .collect();
    }

    /// Przełącza motyw systemowy zgodnie z harmonogramem. Zwraca `true`, gdy
    /// właśnie zapisano nowy wariant.
    pub fn tick_theme(&mut self, system_dark: bool) -> bool {
        if !self.prefs.auto_theme {
            self.theme_target = Some(system_dark);
            return false;
        }
        let want_dark = self.prefs.scheduled_dark(&self.now);
        if self.theme_target == Some(want_dark) {
            return false;
        }
        let _ = libluna_core::appearance::set_system_dark(want_dark);
        self.theme_target = Some(want_dark);
        self.show_toast(if want_dark {
            tr("Automatic dark theme")
        } else {
            tr("Automatic light theme")
        });
        true
    }

    pub fn update(&mut self, dt: f64) -> Update {
        let mut animating = self.toast.advance(dt, 0.18);
        if self
            .toast_until
            .is_some_and(|until| Instant::now() >= until)
        {
            self.toast.set_open_animated(false);
            self.toast_until = None;
        }

        let unix = now_unix();
        let second_changed = unix != self.now_unix;
        if second_changed {
            self.refresh_clock();
            self.tick_theme(libluna_core::appearance::system_dark());
        }

        if self.timer.tick() {
            self.show_toast(tr("Timer finished"));
            self.persist_activity(true);
        }
        if second_changed && (self.timer.running || self.stopwatch.running) {
            // Odśwież stan dla panelu raz na sekundę (minutnik działa też po zamknięciu).
            self.persist_activity(true);
        }

        let delay = if self.stopwatch.running {
            Some(Duration::from_millis(20))
        } else if self.timer.running {
            Some(Duration::from_millis(100))
        } else if self.toast.is_open() || self.toast.is_animating() {
            Some(Duration::from_millis(100))
        } else {
            // Odśwież dokładnie przy zmianie sekundy (zegar cyfrowy).
            let fraction = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.subsec_millis() as u64)
                .unwrap_or(0);
            Some(Duration::from_millis((1000 - fraction).clamp(50, 1000)))
        };
        animating |= self.toast.is_animating();
        Update { animating, delay }
    }

    pub fn show_toast(&mut self, message: impl Into<String>) {
        self.toast.set_message(message);
        self.toast.set_open_animated(true);
        self.toast_until = Some(Instant::now() + Duration::from_secs(3));
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.form = None;
        self.picker = None;
        self.importer = None;
    }

    pub fn month_prev(&mut self) {
        self.calendar.prev_month();
        self.sync_selected_day();
    }

    pub fn month_next(&mut self) {
        self.calendar.next_month();
        self.sync_selected_day();
    }

    pub fn go_today(&mut self) {
        let today = self.now.date;
        self.calendar = Calendar::new(today.year, today.month);
        self.calendar.set_selected(today.day);
        self.selected_date = today;
    }

    pub fn select_day(&mut self, day: u32) {
        self.calendar.set_selected(day);
        self.selected_date = CivilDate::from_ymd(
            self.calendar.year(),
            self.calendar.month(),
            self.calendar.selected(),
        );
    }

    fn sync_selected_day(&mut self) {
        self.selected_date = CivilDate::from_ymd(
            self.calendar.year(),
            self.calendar.month(),
            self.calendar.selected(),
        );
    }

    pub fn select_date(&mut self, date: CivilDate) {
        self.calendar = Calendar::new(date.year, date.month);
        self.calendar.set_selected(date.day);
        self.selected_date = date;
    }

    pub fn open_new_event(&mut self) {
        self.form = Some(EventForm::new(self.selected_date));
        self.picker = None;
        self.importer = None;
    }

    pub fn open_edit_event(&mut self, id: u64) {
        if let Some(event) = self.events.get(id) {
            self.form = Some(EventForm::from_event(event));
            self.picker = None;
            self.importer = None;
        }
    }

    pub fn save_form(&mut self) -> bool {
        let Some(form) = self.form.as_ref() else {
            return false;
        };
        let title = form.title.text().trim().to_owned();
        let note = form.note.text().trim().to_owned();
        let time = match parse_time_input(form.time.text()) {
            Ok(time) => time,
            Err(()) => {
                if let Some(form) = self.form.as_mut() {
                    form.error = Some("Use HH:MM");
                }
                return false;
            }
        };
        if title.is_empty() {
            if let Some(form) = self.form.as_mut() {
                form.error = Some("Enter a title");
            }
            return false;
        }
        let Some(form) = self.form.take() else {
            return false;
        };
        let duration = form.duration();
        let saved = match form.editing {
            Some(id) => {
                let mut event = Event {
                    id,
                    date: form.date,
                    start_minute: time,
                    duration_min: if time.is_some() { duration } else { 0 },
                    title,
                    note,
                };
                event.duration_min = if event.start_minute.is_some() {
                    event.duration_min
                } else {
                    0
                };
                self.events.update(event)
            }
            None => {
                self.events.add(form.date, time, duration, title, note);
                true
            }
        };
        if saved {
            let _ = self.events.save();
            self.selected_date = form.date;
            self.select_date(form.date);
            self.show_toast(tr("Event saved"));
        }
        saved
    }

    pub fn delete_event(&mut self, id: u64) {
        if self.events.remove(id) {
            let _ = self.events.save();
            self.show_toast(tr("Event deleted"));
        }
    }

    pub fn open_picker(&mut self, kind: PickerKind) {
        self.picker = Some(Picker::new(kind));
        self.form = None;
        self.importer = None;
    }

    pub fn picker_values(&self) -> Vec<String> {
        let Some(picker) = &self.picker else {
            return Vec::new();
        };
        let query = picker.query.text().trim().to_lowercase();
        match picker.kind {
            PickerKind::Zone => self
                .zones_all
                .iter()
                .filter(|zone| query.is_empty() || zone.to_lowercase().contains(&query))
                .cloned()
                .collect(),
            PickerKind::Country => {
                let mut values: Vec<String> = holidays::COUNTRIES
                    .iter()
                    .filter(|country| {
                        query.is_empty()
                            || country.code.to_lowercase().contains(&query)
                            || country.name.to_lowercase().contains(&query)
                            || tr(country.name).to_lowercase().contains(&query)
                    })
                    .map(|country| country.code.to_owned())
                    .collect();
                values.sort_by_cached_key(|code| tr(holidays::country_name(code)).to_lowercase());
                values
            }
        }
    }

    pub fn move_picker(&mut self, delta: i64) {
        let len = self.picker_values().len();
        if len == 0 {
            return;
        }
        if let Some(picker) = self.picker.as_mut() {
            picker.selected = (picker.selected as i64 + delta).clamp(0, len as i64 - 1) as usize;
        }
    }

    pub fn apply_picker(&mut self) {
        let values = self.picker_values();
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        let kind = picker.kind;
        let Some(value) = values.get(picker.selected).cloned() else {
            return;
        };
        match kind {
            PickerKind::Zone => {
                if self.prefs.add_zone(&value) {
                    self.prefs.save();
                    self.refresh_clock();
                    self.show_toast(format!("{}: {}", tr("Added"), value));
                } else {
                    self.show_toast(tr("This time zone is already on the list"));
                }
                self.picker = None;
            }
            PickerKind::Country => {
                self.prefs.country = value;
                self.prefs.save();
                self.picker = None;
                self.show_toast(tr("Country changed"));
            }
        }
    }

    pub fn remove_zone(&mut self, index: usize) {
        self.prefs.remove_zone(index);
        self.prefs.save();
        self.refresh_clock();
    }

    pub fn use_detected_country(&mut self) {
        self.prefs.country.clear();
        self.prefs.save();
        self.show_toast(match self.detected_country {
            Some(code) => format!("{}: {}", tr("Detected"), tr(holidays::country_name(code))),
            None => tr("Could not detect the country"),
        });
    }

    pub fn cycle_duration(&mut self, delta: i64) {
        if let Some(form) = self.form.as_mut() {
            let len = DURATION_OPTIONS.len() as i64;
            form.duration_index = (form.duration_index as i64 + delta).rem_euclid(len) as usize;
        }
    }

    pub fn open_importer(&mut self) {
        self.importer = Some(ImportForm::new());
        self.form = None;
        self.picker = None;
    }

    pub fn import_ics(&mut self) -> bool {
        let Some(importer) = self.importer.as_ref() else {
            return false;
        };
        let path = expand_home(importer.path.text().trim());
        if path.as_os_str().is_empty() {
            if let Some(importer) = self.importer.as_mut() {
                importer.error = Some("Enter a file path");
            }
            return false;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => {
                if let Some(importer) = self.importer.as_mut() {
                    importer.error = Some("Cannot read this file");
                }
                return false;
            }
        };
        let added = self.events.import_ics(&text);
        let _ = self.events.save();
        self.importer = None;
        if added == 0 {
            self.show_toast(tr("No new events found"));
        } else {
            self.show_toast(format!("{}: {added}", tr("Imported events")));
        }
        added > 0
    }

    pub fn export_ics(&mut self) {
        let Some(base) =
            EventStore::default_path().and_then(|path| path.parent().map(Path::to_path_buf))
        else {
            self.show_toast(tr("Cannot save the file"));
            return;
        };
        let path = base.join("calendar.ics");
        match libluna_core::config::save_atomic_private(&path, &self.events.export_ics()) {
            Ok(()) => self.show_toast(format!("{}: {}", tr("Exported"), path.display())),
            Err(_) => self.show_toast(tr("Cannot save the file")),
        }
    }

    pub fn exported_path(&self) -> PathBuf {
        EventStore::default_path()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("calendar.ics")
    }

    pub fn apply_theme_now(&mut self) -> bool {
        let dark = self.prefs.scheduled_dark(&self.now);
        let _ = libluna_core::appearance::set_system_dark(dark);
        self.theme_target = Some(dark);
        dark
    }
}

/// Parsuje `HH:MM` (albo puste pole = cały dzień).
pub fn parse_time_input(text: &str) -> Result<Option<u32>, ()> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let mut parts = text.split(':');
    let hour = parts.next().ok_or(())?.trim();
    let minute = parts.next().ok_or(())?;
    if parts.next().is_some() || hour.is_empty() || minute.is_empty() {
        return Err(());
    }
    if hour.len() > 2 || minute.len() > 2 || !hour.chars().all(|c| c.is_ascii_digit()) {
        return Err(());
    }
    if !minute.chars().all(|c| c.is_ascii_digit()) {
        return Err(());
    }
    let hour = hour.parse::<u32>().map_err(|_| ())?;
    let minute = minute.parse::<u32>().map_err(|_| ())?;
    if hour > 23 || minute > 59 {
        return Err(());
    }
    Ok(Some(hour * 60 + minute))
}

/// `MM:SS` albo `HH:MM:SS` (minutnik).
pub fn format_timer(remaining: Duration) -> String {
    let secs = ((remaining.as_millis() + 999) / 1000) as u64;
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// `MM:SS.cc` albo `HH:MM:SS.cc` (stoper).
pub fn format_stopwatch(elapsed: Duration) -> String {
    let centis = (elapsed.as_millis() / 10) % 100;
    let secs = elapsed.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}.{centis:02}")
    } else {
        format!("{minutes:02}:{seconds:02}.{centis:02}")
    }
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

/// Uruchamia aplikację.
pub fn run() -> Result<(), Box<dyn Error>> {
    set_locale(preferred_locale());
    load_catalogs_from_dirs();

    let (start_date, start_tab) = parse_args();
    let settings = ThemeSettings::load();
    let system_dark = libluna_core::appearance::system_dark();
    let prefs = Prefs::load();
    let events = EventStore::load();
    let activity = ClockActivity::load();
    let mut app = App::new(prefs, events, start_date, activity);
    if let Some(tab) = start_tab {
        app.set_tab(tab);
    }
    // Znacznik działającej aplikacji — panel nie pokazuje wtedy dublującego widgetu.
    app.persist_activity(true);

    let config = AppConfig::new(tr("Clock"))
        .app_id("eu.luna.Clock")
        .initial_size(1120, 760)
        .settings(settings)
        .system_dark(system_dark)
        .escape_to_exit(false);

    let mut last_title = String::new();
    run_app(config, move |frame: &mut Frame| {
        let theme = frame.settings().resolve_themed(frame.system_dark());
        crate::ui::handle_events(&mut app, frame.events, frame.size, &theme, frame.text);
        let update = app.update(frame.delta_time());

        let title = app.window_title();
        if title != last_title {
            frame.set_title(title.clone());
            last_title = title;
        }

        crate::ui::paint(&mut app, frame.scene, frame.size, &theme, frame.text);

        if let Some(text) = app.clipboard_text.take() {
            frame.copy_to_clipboard(text);
        }
        if app.pending_close {
            app.pending_close = false;
            // Po zamknięciu panel przejmuje odliczanie i pokazuje widget.
            app.persist_activity(false);
            frame.close();
        }
        if app.pending_minimize {
            app.pending_minimize = false;
            frame.minimize();
        }
        if app.pending_maximize {
            app.pending_maximize = false;
            frame.toggle_maximize();
        }
        if app.pending_move {
            app.pending_move = false;
            frame.begin_move();
        }

        if update.animating {
            frame.request_frame();
        } else if let Some(delay) = update.delay {
            frame.request_frame_after(delay);
        }
    })
}

/// `--date RRRR-MM-DD` i `--tab clock|timer|stopwatch|calendar|settings`.
fn parse_args() -> (Option<CivilDate>, Option<Tab>) {
    let mut date = None;
    let mut tab = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--date" => {
                if let Some(value) = args.next() {
                    date = CivilDate::parse_iso(&value);
                }
            }
            value if value.starts_with("--date=") => {
                date = CivilDate::parse_iso(&value["--date=".len()..]);
            }
            "--tab" => {
                if let Some(value) = args.next() {
                    tab = Tab::from_name(&value);
                }
            }
            value if value.starts_with("--tab=") => {
                tab = Tab::from_name(&value["--tab=".len()..]);
            }
            _ => {}
        }
    }
    (date, tab)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_time_input() {
        assert_eq!(parse_time_input(""), Ok(None));
        assert_eq!(parse_time_input("  "), Ok(None));
        assert_eq!(parse_time_input("9:05"), Ok(Some(9 * 60 + 5)));
        assert_eq!(parse_time_input("23:59"), Ok(Some(23 * 60 + 59)));
        assert_eq!(parse_time_input("24:00"), Err(()));
        assert_eq!(parse_time_input("12:60"), Err(()));
        assert_eq!(parse_time_input("12"), Err(()));
        assert_eq!(parse_time_input("ab:cd"), Err(()));
    }

    #[test]
    fn formats_timer_and_stopwatch() {
        assert_eq!(format_timer(Duration::from_secs(0)), "00:00");
        assert_eq!(format_timer(Duration::from_secs(59)), "00:59");
        assert_eq!(format_timer(Duration::from_secs(61)), "01:01");
        assert_eq!(format_timer(Duration::from_secs(3661)), "01:01:01");
        assert_eq!(format_stopwatch(Duration::from_millis(65_432)), "01:05.43");
        assert_eq!(
            format_stopwatch(Duration::from_millis(3_665_432)),
            "01:01:05.43"
        );
    }

    #[test]
    fn timer_adjusts_and_starts() {
        let mut timer = TimerState::new(5);
        assert_eq!(timer.duration, Duration::from_secs(300));
        timer.adjust(TimerUnit::Minutes, -1);
        assert_eq!(timer.duration, Duration::from_secs(240));
        timer.adjust(TimerUnit::Hours, 1);
        assert_eq!(timer.duration, Duration::from_secs(3_840));
        timer.start_pause();
        assert!(timer.running);
        timer.start_pause();
        assert!(!timer.running);
        timer.reset();
        assert_eq!(timer.remaining, timer.duration);
        assert!(!timer.finished);
    }

    #[test]
    fn stopwatch_laps_and_reset() {
        let mut watch = Stopwatch::default();
        assert!(watch.is_empty());
        watch.start_pause();
        assert!(watch.running);
        watch.lap();
        watch.lap();
        assert_eq!(watch.laps.len(), 2);
        watch.start_pause();
        assert!(!watch.running);
        watch.reset();
        assert!(watch.is_empty());
    }

    #[test]
    fn empty_event_form_reports_missing_title() {
        let mut store = EventStore::from_text("", None);
        store.set_path(None);
        let prefs = Prefs::default();
        let mut app = App::new(prefs, store, None, None);
        app.open_new_event();
        assert!(!app.save_form());
        assert_eq!(
            app.form.as_ref().and_then(|form| form.error),
            Some("Enter a title")
        );
    }

    #[test]
    fn event_form_saves_without_a_file() {
        let mut store = EventStore::from_text("", None);
        store.set_path(None);
        let mut app = App::new(Prefs::default(), store, None, None);
        app.open_new_event();
        {
            let form = app.form.as_mut().unwrap();
            form.title.set_text("Kawa z Anią");
            form.time.set_text("10:30");
            form.note.set_text("ulubiona kawiarnia");
        }
        assert!(app.save_form());
        let day = app.selected_date;
        let events = app.events_on(day);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].start_minute, Some(10 * 60 + 30));
        assert_eq!(events[0].title, "Kawa z Anią");
    }

    #[test]
    fn tab_roundtrip() {
        for (index, tab) in Tab::ALL.iter().enumerate() {
            assert_eq!(Tab::from_index(index), *tab);
            assert_eq!(tab.index(), index);
            assert_eq!(Tab::from_name(tab.title_key()), Some(*tab));
        }
        assert_eq!(Tab::from_name("timer"), Some(Tab::Timer));
        assert_eq!(Tab::from_name("nonsense"), None);
    }

    #[test]
    fn restores_a_running_timer() {
        let now = libluna_core::clock_state::now_unix();
        let activity = ClockActivity {
            pid: None,
            updated_unix: now,
            timer: Some(TimerActivity {
                total_secs: 300,
                running: true,
                end_unix: now + 120,
                remaining_secs: 0,
                finished: false,
            }),
            stopwatch: None,
        };
        let mut store = EventStore::from_text("", None);
        store.set_path(None);
        let app = App::new(Prefs::default(), store, None, Some(activity));
        assert!(app.timer.running);
        let remaining = app.timer.remaining().as_secs();
        assert!((119..=120).contains(&remaining));
    }

    #[test]
    fn restores_a_stopwatch() {
        let now = libluna_core::clock_state::now_unix();
        let activity = ClockActivity {
            pid: None,
            updated_unix: now,
            timer: None,
            stopwatch: Some(StopwatchActivity {
                elapsed_millis: 5_000,
                running: false,
                updated_unix: now,
                laps: vec![1_000, 4_000],
            }),
        };
        let mut store = EventStore::from_text("", None);
        store.set_path(None);
        let app = App::new(Prefs::default(), store, None, Some(activity));
        assert_eq!(app.stopwatch.elapsed(), Duration::from_secs(5));
        assert_eq!(app.stopwatch.laps.len(), 2);
    }

    #[test]
    fn fresh_timer_and_stopwatch_do_not_create_activity() {
        let mut store = EventStore::from_text("", None);
        store.set_path(None);
        let app = App::new(Prefs::default(), store, None, None);
        assert!(app.timer_activity().is_none());
        assert!(app.stopwatch_activity().is_none());
    }
}
