//! Headless-podglądy `luna-clock` do dokumentacji (bez kompozytora).
//!
//! Uruchom: `cargo run --bin preview`
//! → `docs/images/clock-{clock,timer,stopwatch,calendar,settings}.png`.

use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use libluna_core::{Size, Theme};
use libluna_i18n::{set_locale, Locale};
use libluna_render::{Scene, SoftwareRenderer, TextEngine};
use luna_clock::app::{App, Tab, TimerState};
use luna_clock::prefs::Prefs;
use luna_clock::store::EventStore;
use luna_clock::time::{local_at, CivilDate};
use luna_clock::ui;

const W: u32 = 1120;
const H: u32 = 760;

fn main() -> Result<(), Box<dyn Error>> {
    set_locale(Locale::new("pl"));

    let mut store = EventStore::from_text("", None);
    store.set_path(None);
    let mut prefs = Prefs::default();
    prefs.country = "PL".to_owned();
    prefs.auto_theme = true;
    prefs.dark_hour = 19;
    prefs.light_hour = 7;
    prefs.world_clocks = vec![
        "Europe/Warsaw".to_owned(),
        "America/New_York".to_owned(),
        "Asia/Tokyo".to_owned(),
    ];

    let date = CivilDate::from_ymd(2026, 10, 31);
    let mut app = App::new(prefs, store, Some(date), None);
    let unix = date.unix_days() * 86_400 + 19 * 3_600 + 24 * 60 + 8;
    app.now = local_at(unix, Some("Europe/Warsaw"));
    app.selected_date = date;
    app.select_date(date);
    app.events.add(
        date,
        Some(19 * 60 + 30),
        90,
        "Kolacja z Anią",
        "Bistro na rogu",
    );
    app.events
        .add(date, None, 0, "Urodziny Ali", "Kupić kwiaty");
    app.events.add(
        CivilDate::from_ymd(2026, 10, 12),
        Some(9 * 60),
        60,
        "Wizyta u lekarza",
        "",
    );
    app.zone_times = app
        .prefs
        .world_clocks
        .iter()
        .map(|zone| (zone.clone(), local_at(unix, Some(zone))))
        .collect();

    let theme = Theme::dark();
    let text = TextEngine::system();
    let size = Size::new(W as f64, H as f64);

    let tabs = [
        (Tab::Clock, "clock-clock.png"),
        (Tab::Timer, "clock-timer.png"),
        (Tab::Stopwatch, "clock-stopwatch.png"),
        (Tab::Calendar, "clock-calendar.png"),
        (Tab::Settings, "clock-settings.png"),
    ];
    for (tab, name) in tabs {
        app.set_tab(tab);
        if tab == Tab::Timer {
            app.timer = TimerState::new(25);
            app.timer.remaining = Duration::from_secs(17 * 60 + 42);
            app.timer.duration = Duration::from_secs(25 * 60);
        }
        if tab == Tab::Stopwatch {
            app.stopwatch.reset();
        }
        let mut scene = Scene::new();
        ui::paint(&mut app, &mut scene, size, &theme, &text);
        save(&scene, W, H, &theme, name)?;
    }

    Ok(())
}

fn save(scene: &Scene, w: u32, h: u32, theme: &Theme, name: &str) -> Result<(), Box<dyn Error>> {
    let mut renderer = SoftwareRenderer::new(w, h);
    renderer.draw(scene, 1.0, theme.colors.window_bg);
    let png = renderer.encode_png().ok_or("nie udało się zakodować PNG")?;
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/images")
        .join(name);
    std::fs::write(&path, png)?;
    println!("saved {}", path.display());
    Ok(())
}
