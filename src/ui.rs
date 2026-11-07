//! Rysowanie i obsługa zdarzeń interfejsu.

use std::time::Duration;

use libluna_core::style::weight;
use libluna_core::{
    ButtonState, Color, Event, Insets, KeyEvent, Point, PointerButton, Rect, Size, Theme,
    WindowEvent,
};
use libluna_render::{Scene, TextEngine};
use libluna_widgets::{
    clipboard_command, Button, ButtonVariant, ControlsSide, Entry, HeaderBar, PaintCtx, Switch,
    TextClipboardCommand, TitleAlign, ViewSwitcher, WindowControl,
};

use crate::app::{
    format_stopwatch, format_timer, Action, App, FormField, PickerKind, Tab, TimerUnit,
};
use crate::holidays;
use crate::prefs::zone_city;
use crate::time::{month_name_key, weekday_short_key, CivilDate, DateFormat};

// Keysyms XKB.
const KEY_ESCAPE: u32 = 0xff1b;
const KEY_ENTER: u32 = 0xff0d;
const KEY_KP_ENTER: u32 = 0xff8d;
const KEY_TAB: u32 = 0xff09;
const KEY_BACKSPACE: u32 = 0xff08;
const KEY_DELETE: u32 = 0xffff;
const KEY_HOME: u32 = 0xff50;
const KEY_LEFT: u32 = 0xff51;
const KEY_UP: u32 = 0xff52;
const KEY_RIGHT: u32 = 0xff53;
const KEY_DOWN: u32 = 0xff54;
const KEY_END: u32 = 0xff57;
const KEY_SPACE: u32 = 0x20;

/// Wspólny układ okna: pasek tytułu z zakładkami i treść pod nim.
#[derive(Debug, Clone, Copy)]
struct Chrome {
    header: Rect,
    tabs: Rect,
    content: Rect,
}

fn chrome(size: Size, theme: &Theme, text: &TextEngine) -> Chrome {
    let header = Rect::from_xywh(0.0, 0.0, size.width, theme.metrics.headerbar.height);
    let header_bar = HeaderBar::new()
        .title(libluna_i18n::tr("Clock"))
        .title_align(TitleAlign::Start)
        .controls(ControlsSide::Right);
    let available = header_bar.controls_layout(header, theme).content;

    // Zostawiamy tytułowi miejsce po lewej, a przełącznik stron wyrównujemy
    // w pozostałej części paska (jak AdwViewSwitcher w headerbarze).
    let title_width = text
        .measure(
            &libluna_i18n::tr("Clock"),
            theme.typography.body.size,
            weight::BOLD,
        )
        .width;
    let title_end = available.x() + title_width + 20.0;
    let free = (available.right() - title_end).max(0.0);
    let tabs_w = free.min(560.0);
    let tabs = Rect::from_xywh(
        title_end + (free - tabs_w) / 2.0,
        header.center().y - 15.0,
        tabs_w,
        30.0,
    );

    let top = header.bottom() + 16.0;
    let content = Rect::from_xywh(
        24.0,
        top,
        (size.width - 48.0).max(0.0),
        (size.height - top - 16.0).max(0.0),
    );
    Chrome {
        header,
        tabs,
        content,
    }
}

fn paint_card(ctx: &mut PaintCtx, rect: Rect) {
    let theme = *ctx.theme;
    ctx.scene
        .fill_rounded_rect(rect, theme.radius.large, theme.colors.card_bg);
    ctx.scene
        .stroke_rounded_rect(rect, theme.radius.large, 1.0, theme.colors.border);
}

fn push_hit(hits: &mut Vec<(Action, Rect)>, action: Action, rect: Rect) {
    hits.push((action, rect));
}

fn hit(app: &mut App, action: Action, rect: Rect) {
    push_hit(&mut app.hits, action, rect);
}

fn is_hovered(app: &App, rect: Rect) -> bool {
    app.pointer.is_some_and(|point| rect.contains(point))
}

fn paint_button_state(
    ctx: &mut PaintCtx,
    rect: Rect,
    label: &str,
    variant: ButtonVariant,
    hovered: bool,
) {
    let mut button = Button::new(label).variant(variant);
    button.set_hovered(hovered);
    button.paint(ctx, rect);
}

fn paint_button(ctx: &mut PaintCtx, app: &App, rect: Rect, label: &str, variant: ButtonVariant) {
    paint_button_state(ctx, rect, label, variant, is_hovered(app, rect));
}

fn paint_small_button(ctx: &mut PaintCtx, app: &App, rect: Rect, label: &str) {
    paint_button(ctx, app, rect, label, ButtonVariant::Flat);
}

fn paint_small_button_state(ctx: &mut PaintCtx, rect: Rect, label: &str, hovered: bool) {
    paint_button_state(ctx, rect, label, ButtonVariant::Flat, hovered);
}

fn paint_switch(ctx: &mut PaintCtx, app: &App, rect: Rect, on: bool) {
    let mut switch = Switch::new(on);
    switch.set_hovered(is_hovered(app, rect));
    switch.paint(ctx, rect);
}

fn heading(ctx: &mut PaintCtx, x: f64, cy: f64, text: &str) {
    let theme = ctx.theme;
    ctx.label_left(
        x,
        cy,
        text,
        theme.typography.heading.size,
        weight::BOLD,
        theme.colors.text,
    );
}

fn caption(ctx: &mut PaintCtx, x: f64, cy: f64, text: &str, color: Color) {
    let theme = ctx.theme;
    ctx.label_left(
        x,
        cy,
        text,
        theme.typography.caption.size,
        weight::NORMAL,
        color,
    );
}

fn wrap_lines(ctx: &PaintCtx, text: &str, width: f64, size: f64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if !current.is_empty() && ctx.measure(&candidate, size, weight::NORMAL) > width {
            lines.push(current);
            current = word.to_owned();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn paint_wrapped(
    ctx: &mut PaintCtx,
    x: f64,
    top: f64,
    width: f64,
    text: &str,
    size: f64,
    color: Color,
    line_height: f64,
) -> f64 {
    let lines = wrap_lines(ctx, text, width, size);
    let mut y = top;
    for line in lines {
        ctx.label_left(x, y, &line, size, weight::NORMAL, color);
        y += line_height;
    }
    y
}

fn format_date(format: DateFormat, date: CivilDate, weekday: bool) -> String {
    if weekday {
        format.full(date)
    } else {
        format.date(date)
    }
}

fn format_month(year: i32, month: u32) -> String {
    format!("{} {}", libluna_i18n::tr(month_name_key(month)), year)
}

fn duration_label(minutes: u32) -> String {
    if minutes >= 60 && minutes % 60 == 0 {
        format!("{} h", minutes / 60)
    } else if minutes > 60 {
        format!("{} h {} min", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} min")
    }
}

fn status_label(app: &App) -> String {
    if app.timer.finished {
        libluna_i18n::tr("Finished").to_owned()
    } else if app.timer.running {
        libluna_i18n::tr("Running").to_owned()
    } else if app.timer.remaining() < app.timer.duration {
        libluna_i18n::tr("Paused").to_owned()
    } else {
        libluna_i18n::tr("Ready").to_owned()
    }
}

/// Rysuje całą klatkę i zbiera regiony kliknięć.
pub fn paint(app: &mut App, scene: &mut Scene, size: Size, theme: &Theme, text: &TextEngine) {
    app.hits.clear();
    app.modal_rect = None;

    let layout = chrome(size, theme, text);
    let mut ctx = PaintCtx::new(scene, theme, text);

    ctx.scene
        .fill_rect(Rect::from_size(size), theme.colors.window_bg);

    HeaderBar::new()
        .title(libluna_i18n::tr("Clock"))
        .title_align(TitleAlign::Start)
        .controls(ControlsSide::Right)
        .paint(&mut ctx, layout.header, app.controls_hover);

    paint_tabs(app, &mut ctx, layout.tabs);
    paint_page(app, &mut ctx, layout.content);

    if app.form.is_some() {
        app.hits.clear();
        paint_form(app, &mut ctx, size);
    } else if app.picker.is_some() {
        app.hits.clear();
        paint_picker(app, &mut ctx, size);
    } else if app.importer.is_some() {
        app.hits.clear();
        paint_importer(app, &mut ctx, size);
    }

    app.toast.paint(&mut ctx, size);
}

fn paint_tabs(app: &mut App, ctx: &mut PaintCtx, rect: Rect) {
    let labels: Vec<String> = Tab::ALL
        .iter()
        .map(|tab| libluna_i18n::tr(tab.title_key()))
        .collect();
    let mut switcher = ViewSwitcher::new(labels);
    switcher.set_selected(app.tab.index());
    if let Some(point) = app.pointer {
        switcher.set_hovered(switcher.index_at(rect, point));
    }
    switcher.paint(ctx, rect);
    for (index, segment) in switcher.segments(rect).into_iter().enumerate() {
        hit(app, Action::Tab(Tab::from_index(index)), segment);
    }
}

fn paint_page(app: &mut App, ctx: &mut PaintCtx, area: Rect) {
    match app.tab {
        Tab::Clock => paint_clock(app, ctx, area),
        Tab::Timer => paint_timer(app, ctx, area),
        Tab::Stopwatch => paint_stopwatch(app, ctx, area),
        Tab::Calendar => paint_calendar(app, ctx, area),
        Tab::Settings => paint_settings(app, ctx, area),
    }
}

// ── Zegar ──

fn paint_clock(app: &mut App, ctx: &mut PaintCtx, area: Rect) {
    let gap = 16.0;
    let wide = area.width() >= 760.0;
    let (local, world) = if wide {
        let left_w = (area.width() * 0.46).floor();
        (
            Rect::from_xywh(area.x(), area.y(), left_w, area.height()),
            Rect::from_xywh(
                area.x() + left_w + gap,
                area.y(),
                area.width() - left_w - gap,
                area.height(),
            ),
        )
    } else {
        let top_h = (area.height() * 0.49).floor();
        (
            Rect::from_xywh(area.x(), area.y(), area.width(), top_h),
            Rect::from_xywh(
                area.x(),
                area.y() + top_h + gap,
                area.width(),
                area.height() - top_h - gap,
            ),
        )
    };

    paint_card(ctx, local);
    paint_card(ctx, world);

    let theme = *ctx.theme;
    caption(
        ctx,
        local.x() + 24.0,
        local.y() + 30.0,
        &libluna_i18n::tr("Local time"),
        theme.colors.dim_text,
    );

    let time = app
        .now
        .format_clock(app.prefs.use_24h, app.prefs.show_seconds);
    let mut size = (local.width() * 0.21).clamp(42.0, 88.0);
    while ctx.measure(&time, size, weight::BOLD) > local.width() - 48.0 && size > 30.0 {
        size -= 2.0;
    }
    ctx.label_center(
        local.center().x,
        local.y() + local.height() * 0.38,
        &time,
        size,
        weight::BOLD,
        theme.colors.text,
    );
    ctx.label_center(
        local.center().x,
        local.y() + local.height() * 0.38 + size * 0.72,
        &format_date(app.prefs.date_format, app.now.date, true),
        theme.typography.body.size,
        weight::NORMAL,
        theme.colors.text,
    );

    let zone = if app.now.abbr.is_empty() {
        app.now.offset_label()
    } else {
        format!("{} · {}", app.now.abbr, app.now.offset_label())
    };
    ctx.label_center(
        local.center().x,
        local.bottom() - 56.0,
        &zone,
        theme.typography.caption.size,
        weight::MEDIUM,
        theme.colors.dim_text,
    );

    // Najbliższe wydarzenie z kalendarza — spójne z zakładką Kalendarz.
    let events = app.events_on(app.now.date);
    let next = events
        .iter()
        .find(|event| {
            event
                .start_minute
                .is_some_and(|minute| minute >= app.now.minute_of_day())
        })
        .or_else(|| events.first());
    let next_text = match next {
        Some(event) => match event.start_minute {
            Some(_) => format!(
                "{} {} · {}",
                libluna_i18n::tr("Next"),
                event.start_label(app.prefs.use_24h),
                event.title
            ),
            None => format!("{} · {}", libluna_i18n::tr("All day"), event.title),
        },
        None => libluna_i18n::tr("No events today").to_owned(),
    };
    ctx.label_center(
        local.center().x,
        local.bottom() - 28.0,
        &next_text,
        theme.typography.caption.size,
        weight::NORMAL,
        theme.colors.dim_text,
    );

    paint_world_clocks(app, ctx, world);
}

fn paint_world_clocks(app: &mut App, ctx: &mut PaintCtx, card: Rect) {
    let theme = *ctx.theme;
    heading(
        ctx,
        card.x() + 20.0,
        card.y() + 30.0,
        &libluna_i18n::tr("World clocks"),
    );

    let pad = 20.0;
    let row_h = 54.0;
    let add_h = 34.0;
    let list_top = card.y() + 62.0;
    let list_bottom = card.bottom() - pad - add_h - 10.0;
    let max_rows = (((list_bottom - list_top) / row_h).floor() as usize).max(0);

    if app.zone_times.is_empty() {
        caption(
            ctx,
            card.x() + pad,
            list_top + 14.0,
            &libluna_i18n::tr("Add a time zone to see it here."),
            theme.colors.dim_text,
        );
    }

    let zone_times = app.zone_times.clone();
    for (index, (zone, time)) in zone_times.iter().take(max_rows).enumerate() {
        let row = Rect::from_xywh(
            card.x() + pad,
            list_top + index as f64 * row_h,
            card.width() - pad * 2.0,
            row_h - 6.0,
        );
        if is_hovered(app, row) {
            ctx.scene.fill_rounded_rect(
                row,
                theme.radius.medium,
                theme.colors.shade.scale_alpha(0.7),
            );
        }
        ctx.label_left(
            row.x() + 4.0,
            row.y() + row.height() * 0.34,
            &zone_city(zone),
            theme.typography.body.size,
            weight::MEDIUM,
            theme.colors.text,
        );
        caption(
            ctx,
            row.x() + 4.0,
            row.y() + row.height() * 0.72,
            &format!("{} · {}", zone, time.offset_label()),
            theme.colors.dim_text,
        );
        ctx.label_right(
            row.right() - 38.0,
            row.center().y,
            &time.format_clock(app.prefs.use_24h, false),
            theme.typography.heading.size,
            weight::MEDIUM,
            theme.colors.text,
        );
        let remove = Rect::from_xywh(row.right() - 28.0, row.center().y - 13.0, 26.0, 26.0);
        paint_small_button(ctx, app, remove, "×");
        hit(app, Action::RemoveZone(index), remove);
    }

    if app.zone_times.len() > max_rows && max_rows > 0 {
        caption(
            ctx,
            card.x() + pad,
            list_bottom,
            &format!(
                "{} +{}",
                libluna_i18n::tr("More time zones"),
                app.zone_times.len() - max_rows
            ),
            theme.colors.dim_text,
        );
    }

    let add = Rect::from_xywh(
        card.x() + pad,
        card.bottom() - pad - add_h,
        card.width() - pad * 2.0,
        add_h,
    );
    paint_button(
        ctx,
        app,
        add,
        &libluna_i18n::tr("Add time zone"),
        ButtonVariant::Default,
    );
    hit(app, Action::AddZone, add);
}

// ── Minutnik ──

fn paint_timer(app: &mut App, ctx: &mut PaintCtx, area: Rect) {
    let theme = *ctx.theme;
    let card = Rect::from_xywh(
        area.center().x - area.width().min(640.0) / 2.0,
        area.y() + (area.height() - area.height().min(540.0)).max(0.0) / 2.0,
        area.width().min(640.0),
        area.height().min(500.0),
    );
    paint_card(ctx, card);
    heading(
        ctx,
        card.x() + 24.0,
        card.y() + 28.0,
        &libluna_i18n::tr("Timer"),
    );

    let remaining = app.timer.remaining();
    let time = format_timer(remaining);
    let big = if app.timer.finished {
        theme.colors.accent
    } else {
        theme.colors.text
    };
    ctx.label_center(
        card.center().x,
        card.y() + 116.0,
        &time,
        72.0,
        weight::BOLD,
        big,
    );
    ctx.label_center(
        card.center().x,
        card.y() + 164.0,
        &status_label(app),
        theme.typography.body.size,
        weight::MEDIUM,
        theme.colors.dim_text,
    );

    // Pasek postępu.
    let bar = Rect::from_xywh(card.x() + 40.0, card.y() + 192.0, card.width() - 80.0, 8.0);
    ctx.scene.fill_rounded_rect(bar, 4.0, theme.colors.shade);
    let progress = app.timer.progress().clamp(0.0, 1.0);
    if progress > 0.0 {
        ctx.scene.fill_rounded_rect(
            Rect::from_xywh(bar.x(), bar.y(), bar.width() * progress, bar.height()),
            4.0,
            theme.colors.accent,
        );
    }

    caption(
        ctx,
        card.x() + 24.0,
        card.y() + 226.0,
        &libluna_i18n::tr("Quick presets (minutes)"),
        theme.colors.dim_text,
    );
    let presets = [1u32, 3, 5, 10, 15, 25, 30, 60];
    let gap = 6.0;
    let preset_w =
        (card.width() - 48.0 - gap * (presets.len() as f64 - 1.0)) / presets.len() as f64;
    for (index, minutes) in presets.iter().enumerate() {
        let rect = Rect::from_xywh(
            card.x() + 24.0 + index as f64 * (preset_w + gap),
            card.y() + 242.0,
            preset_w,
            30.0,
        );
        paint_button(
            ctx,
            app,
            rect,
            &minutes.to_string(),
            if app.timer.duration == Duration::from_secs(*minutes as u64 * 60) {
                ButtonVariant::Suggested
            } else {
                ButtonVariant::Default
            },
        );
        hit(app, Action::TimerPreset(*minutes), rect);
    }

    let units = [
        (TimerUnit::Hours, "Hours", remaining.as_secs() / 3600),
        (
            TimerUnit::Minutes,
            "Minutes",
            (remaining.as_secs() % 3600) / 60,
        ),
        (TimerUnit::Seconds, "Seconds", remaining.as_secs() % 60),
    ];
    let group_w = (card.width() - 48.0 - 24.0) / 3.0;
    for (index, (unit, label, value)) in units.iter().enumerate() {
        let gx = card.x() + 24.0 + index as f64 * (group_w + 12.0);
        let gy = card.y() + 300.0;
        let minus = Rect::from_xywh(gx, gy, 32.0, 32.0);
        let plus = Rect::from_xywh(gx + group_w - 32.0, gy, 32.0, 32.0);
        paint_small_button(ctx, app, minus, "−");
        paint_small_button(ctx, app, plus, "+");
        hit(app, Action::TimerUnit(*unit, -1), minus);
        hit(app, Action::TimerUnit(*unit, 1), plus);
        ctx.label_center(
            gx + group_w / 2.0,
            gy + 16.0,
            &format!("{value:02}"),
            26.0,
            weight::BOLD,
            theme.colors.text,
        );
        ctx.label_center(
            gx + group_w / 2.0,
            gy + 48.0,
            &libluna_i18n::tr(label),
            theme.typography.caption.size,
            weight::NORMAL,
            theme.colors.dim_text,
        );
    }

    let button_h = 40.0;
    let start_w = 160.0;
    let reset_w = 110.0;
    let total = start_w + 10.0 + reset_w;
    let start = Rect::from_xywh(
        card.center().x - total / 2.0,
        card.bottom() - 24.0 - button_h,
        start_w,
        button_h,
    );
    let reset = Rect::from_xywh(start.right() + 10.0, start.y(), reset_w, button_h);
    let start_label = if app.timer.running {
        libluna_i18n::tr("Pause")
    } else if app.timer.finished {
        libluna_i18n::tr("Restart")
    } else {
        libluna_i18n::tr("Start")
    };
    paint_button(ctx, app, start, &start_label, ButtonVariant::Suggested);
    paint_button(
        ctx,
        app,
        reset,
        &libluna_i18n::tr("Reset"),
        ButtonVariant::Flat,
    );
    hit(app, Action::TimerToggle, start);
    hit(app, Action::TimerReset, reset);
}

// ── Stoper ──

fn paint_stopwatch(app: &mut App, ctx: &mut PaintCtx, area: Rect) {
    let theme = *ctx.theme;
    let card = Rect::from_xywh(
        area.center().x - area.width().min(640.0) / 2.0,
        area.y() + (area.height() - area.height().min(560.0)).max(0.0) / 2.0,
        area.width().min(640.0),
        area.height().min(430.0),
    );
    paint_card(ctx, card);
    heading(
        ctx,
        card.x() + 24.0,
        card.y() + 28.0,
        &libluna_i18n::tr("Stopwatch"),
    );

    let elapsed = app.stopwatch.elapsed();
    let time = format_stopwatch(elapsed);
    ctx.label_center(
        card.center().x,
        card.y() + 118.0,
        &time,
        66.0,
        weight::BOLD,
        theme.colors.text,
    );
    let watch_status = if app.stopwatch.running {
        libluna_i18n::tr("Running")
    } else if app.stopwatch.is_empty() {
        libluna_i18n::tr("Ready")
    } else {
        libluna_i18n::tr("Paused")
    };
    ctx.label_center(
        card.center().x,
        card.y() + 164.0,
        &watch_status,
        theme.typography.body.size,
        weight::MEDIUM,
        theme.colors.dim_text,
    );

    let gap = 10.0;
    let button_h = 38.0;
    let button_w = (card.width() - 48.0 - gap * 2.0) / 3.0;
    let y = card.y() + 200.0;
    let start = Rect::from_xywh(card.x() + 24.0, y, button_w, button_h);
    let lap = Rect::from_xywh(start.right() + gap, y, button_w, button_h);
    let reset = Rect::from_xywh(lap.right() + gap, y, button_w, button_h);
    let start_label = if app.stopwatch.running {
        libluna_i18n::tr("Pause")
    } else {
        libluna_i18n::tr("Start")
    };
    paint_button(ctx, app, start, &start_label, ButtonVariant::Suggested);
    paint_button(ctx, app, lap, &libluna_i18n::tr("Lap"), ButtonVariant::Flat);
    paint_button(
        ctx,
        app,
        reset,
        &libluna_i18n::tr("Reset"),
        ButtonVariant::Flat,
    );
    hit(app, Action::WatchToggle, start);
    hit(app, Action::WatchLap, lap);
    hit(app, Action::WatchReset, reset);

    let list_top = y + button_h + 20.0;
    let row_h = 34.0;
    let max_rows = (((card.bottom() - 20.0 - list_top) / row_h).floor() as usize).max(0);
    if app.stopwatch.laps.is_empty() {
        caption(
            ctx,
            card.x() + 24.0,
            list_top + 12.0,
            &libluna_i18n::tr("Press Lap to record a split."),
            theme.colors.dim_text,
        );
        return;
    }
    let total_laps = app.stopwatch.laps.len();
    for (index, lap) in app.stopwatch.laps.iter().rev().take(max_rows).enumerate() {
        let row = Rect::from_xywh(
            card.x() + 24.0,
            list_top + index as f64 * row_h,
            card.width() - 48.0,
            row_h - 4.0,
        );
        if index % 2 == 0 {
            ctx.scene.fill_rounded_rect(
                row,
                theme.radius.small,
                theme.colors.shade.scale_alpha(0.6),
            );
        }
        ctx.label_left(
            row.x() + 8.0,
            row.center().y,
            &format!("{} {}", libluna_i18n::tr("Lap"), total_laps - index),
            theme.typography.caption.size,
            weight::MEDIUM,
            theme.colors.dim_text,
        );
        ctx.label_right(
            row.right() - 8.0,
            row.center().y,
            &format!(
                "{}   +{}",
                format_stopwatch(lap.total),
                format_stopwatch(lap.delta)
            ),
            theme.typography.caption.size,
            weight::NORMAL,
            theme.colors.text,
        );
    }
}

// ── Kalendarz ──

fn paint_calendar(app: &mut App, ctx: &mut PaintCtx, area: Rect) {
    let gap = 16.0;
    let wide = area.width() >= 780.0;
    let (month_card, details) = if wide {
        let left_w = (area.width() * 0.58).floor();
        (
            Rect::from_xywh(area.x(), area.y(), left_w, area.height()),
            Rect::from_xywh(
                area.x() + left_w + gap,
                area.y(),
                area.width() - left_w - gap,
                area.height(),
            ),
        )
    } else {
        let top_h = (area.height() * 0.56).floor();
        (
            Rect::from_xywh(area.x(), area.y(), area.width(), top_h),
            Rect::from_xywh(
                area.x(),
                area.y() + top_h + gap,
                area.width(),
                area.height() - top_h - gap,
            ),
        )
    };

    paint_card(ctx, month_card);
    paint_card(ctx, details);
    let theme = *ctx.theme;

    // Nagłówek miesiąca: ‹ Miesiąc Rok [Dziś] ›
    let header_y = month_card.y() + 20.0;
    let nav = 34.0;
    let prev = Rect::from_xywh(month_card.x() + 16.0, header_y, nav, nav);
    let next = Rect::from_xywh(month_card.right() - 16.0 - nav, header_y, nav, nav);
    let today_w = 66.0;
    let today = Rect::from_xywh(next.x() - 8.0 - today_w, header_y, today_w, nav);
    paint_small_button(ctx, app, prev, "‹");
    paint_small_button(ctx, app, next, "›");
    paint_button(
        ctx,
        app,
        today,
        &libluna_i18n::tr("Today"),
        ButtonVariant::Flat,
    );
    hit(app, Action::MonthPrev, prev);
    hit(app, Action::MonthNext, next);
    hit(app, Action::Today, today);

    ctx.label_left(
        prev.right() + 14.0,
        header_y + nav / 2.0,
        &format_month(app.calendar.year(), app.calendar.month()),
        theme.typography.heading.size,
        weight::BOLD,
        theme.colors.text,
    );

    let cal_rect = Rect::from_xywh(
        month_card.x() + 16.0,
        header_y + nav + 12.0,
        month_card.width() - 32.0,
        month_card.height() - (header_y - month_card.y()) - nav - 12.0 - 14.0,
    );
    let weekdays: Vec<String> = (0..7)
        .map(|day| libluna_i18n::tr(weekday_short_key(day)))
        .collect();
    app.calendar.paint(ctx, cal_rect, "", &weekdays);

    // Kropki wydarzeń i świąt + dzisiejszy pierścień.
    let holiday_dates = if app.prefs.show_holidays {
        app.holiday_dates(app.calendar.year(), app.calendar.month())
    } else {
        Vec::new()
    };
    for day in 1..=app.calendar.days_in_month() {
        let date = CivilDate::from_ymd(app.calendar.year(), app.calendar.month(), day);
        let cell = app.calendar.day_rect(cal_rect, &theme, day);
        if date == app.now.date && app.calendar.selected() != day {
            ctx.scene.stroke_rounded_rect(
                cell.deflate(Insets::splat(5.0)),
                theme.radius.small,
                1.5,
                theme.colors.accent.scale_alpha(0.55),
            );
        }
        let selected = app.calendar.selected() == day;
        let mut dots = Vec::new();
        if app.events.count_on(date) > 0 {
            dots.push(if selected {
                theme.colors.accent_fg
            } else {
                theme.colors.accent
            });
        }
        if holiday_dates.contains(&date) {
            dots.push(theme.colors.warning);
        }
        let start_x = cell.center().x - (dots.len() as f64 - 1.0) * 3.5;
        for (index, color) in dots.into_iter().enumerate() {
            ctx.scene.fill_circle(
                Point::new(start_x + index as f64 * 7.0, cell.bottom() - 6.0),
                2.2,
                color,
            );
        }
        hit(app, Action::Day(day), cell);
    }

    paint_day_details(app, ctx, details);
}

fn paint_day_details(app: &mut App, ctx: &mut PaintCtx, card: Rect) {
    let theme = *ctx.theme;
    let pad = 20.0;
    let date = app.selected_date;

    ctx.label_left(
        card.x() + pad,
        card.y() + 30.0,
        &format_date(app.prefs.date_format, date, true),
        theme.typography.heading.size,
        weight::BOLD,
        theme.colors.text,
    );
    let mut y = card.y() + 60.0;

    let holidays = app.holidays_for(date);
    if !holidays.is_empty() {
        caption(
            ctx,
            card.x() + pad,
            y,
            &libluna_i18n::tr("Holidays"),
            theme.colors.warning,
        );
        y += 20.0;
        for name in holidays {
            ctx.scene.fill_circle(
                Point::new(card.x() + pad + 4.0, y),
                3.0,
                theme.colors.warning,
            );
            ctx.label_left(
                card.x() + pad + 14.0,
                y,
                &libluna_i18n::tr(name),
                theme.typography.body.size,
                weight::NORMAL,
                theme.colors.text,
            );
            y += 22.0;
        }
        y += 8.0;
    }

    let add_h = 38.0;
    let add = Rect::from_xywh(
        card.x() + pad,
        card.bottom() - pad - add_h,
        card.width() - pad * 2.0,
        add_h,
    );
    let list_bottom = add.y() - 12.0;

    caption(
        ctx,
        card.x() + pad,
        y,
        &libluna_i18n::tr("Events"),
        theme.colors.dim_text,
    );
    y += 18.0;

    let events: Vec<crate::store::Event> = app.events_on(date).into_iter().cloned().collect();
    if events.is_empty() {
        ctx.label_left(
            card.x() + pad,
            y + 8.0,
            &libluna_i18n::tr("No events for this day."),
            theme.typography.body.size,
            weight::NORMAL,
            theme.colors.dim_text,
        );
    } else {
        let row_h = 46.0;
        let max_rows = (((list_bottom - y) / row_h).floor() as usize).max(0);
        for (index, event) in events.iter().take(max_rows).enumerate() {
            let row = Rect::from_xywh(
                card.x() + pad,
                y + index as f64 * row_h,
                card.width() - pad * 2.0,
                row_h - 4.0,
            );
            if is_hovered(app, row) {
                ctx.scene.fill_rounded_rect(
                    row,
                    theme.radius.small,
                    theme.colors.shade.scale_alpha(0.8),
                );
            }
            let time_text = match event.start_minute {
                Some(_) => event.start_label(app.prefs.use_24h),
                None => libluna_i18n::tr("All day"),
            };
            let time_width = ctx.measure(&time_text, theme.typography.caption.size, weight::MEDIUM);
            let title_x = row.x() + 8.0 + time_width.max(28.0) + 12.0;
            ctx.label_left(
                row.x() + 8.0,
                row.y() + row.height() * 0.34,
                &time_text,
                theme.typography.caption.size,
                weight::MEDIUM,
                theme.colors.accent,
            );
            ctx.label_left(
                title_x,
                row.y() + row.height() * 0.34,
                &event.title,
                theme.typography.body.size,
                weight::MEDIUM,
                theme.colors.text,
            );
            if !event.note.is_empty() {
                caption(
                    ctx,
                    title_x,
                    row.y() + row.height() * 0.74,
                    &event.note,
                    theme.colors.dim_text,
                );
            }
            let remove = Rect::from_xywh(row.right() - 28.0, row.center().y - 13.0, 26.0, 26.0);
            paint_small_button(ctx, app, remove, "×");
            hit(app, Action::DeleteEvent(event.id), remove);
            hit(app, Action::EditEvent(event.id), row);
        }
        if events.len() > max_rows && max_rows > 0 {
            caption(
                ctx,
                card.x() + pad,
                list_bottom - 8.0,
                &format!(
                    "{} +{}",
                    libluna_i18n::tr("More events"),
                    events.len() - max_rows
                ),
                theme.colors.dim_text,
            );
        }
    }

    paint_button(
        ctx,
        app,
        add,
        &libluna_i18n::tr("Add event"),
        ButtonVariant::Suggested,
    );
    hit(app, Action::NewEvent, add);
}

// ── Ustawienia ──

fn paint_settings(app: &mut App, ctx: &mut PaintCtx, area: Rect) {
    let gap = 16.0;
    let col_w = (area.width() - gap) / 2.0;
    let left = Rect::from_xywh(area.x(), area.y(), col_w, area.height());
    let right = Rect::from_xywh(
        area.x() + col_w + gap,
        area.y(),
        area.width() - col_w - gap,
        area.height(),
    );

    let appearance_h = (left.height() * 0.48).min(290.0);
    let clock_h = 148.0;
    let appearance = Rect::from_xywh(left.x(), left.y(), left.width(), appearance_h);
    let clock = Rect::from_xywh(left.x(), appearance.bottom() + 12.0, left.width(), clock_h);
    let privacy = Rect::from_xywh(
        left.x(),
        clock.bottom() + 12.0,
        left.width(),
        (left.bottom() - clock.bottom() - 12.0).max(0.0),
    );
    let calendar = right;

    paint_card(ctx, appearance);
    paint_card(ctx, clock);
    paint_card(ctx, privacy);
    paint_card(ctx, calendar);

    let theme = *ctx.theme;
    let pad = 20.0;

    // Wygląd.
    heading(
        ctx,
        appearance.x() + pad,
        appearance.y() + 30.0,
        &libluna_i18n::tr("Appearance"),
    );
    let auto_row = Rect::from_xywh(
        appearance.x() + pad,
        appearance.y() + 50.0,
        appearance.width() - pad * 2.0,
        34.0,
    );
    ctx.label_left(
        auto_row.x(),
        auto_row.center().y,
        &libluna_i18n::tr("Automatic theme"),
        theme.typography.body.size,
        weight::MEDIUM,
        theme.colors.text,
    );
    let switch = Rect::from_xywh(
        auto_row.right() - theme.metrics.switch.width,
        auto_row.center().y - theme.metrics.switch.height / 2.0,
        theme.metrics.switch.width,
        theme.metrics.switch.height,
    );
    paint_switch(ctx, app, switch, app.prefs.auto_theme);
    hit(app, Action::AutoTheme, switch);

    paint_wrapped(
        ctx,
        appearance.x() + pad,
        appearance.y() + 82.0,
        appearance.width() - pad * 2.0,
        &libluna_i18n::tr(
            "Switches the system appearance at fixed hours. Works when Luna is set to Auto.",
        ),
        theme.typography.caption.size,
        theme.colors.dim_text,
        15.0,
    );

    paint_hour_row(
        app,
        ctx,
        appearance.x() + pad,
        appearance.y() + 132.0,
        appearance.width() - pad * 2.0,
        &libluna_i18n::tr("Dark theme from"),
        app.prefs.dark_hour,
        Action::DarkHour(-1),
        Action::DarkHour(1),
        theme,
    );
    paint_hour_row(
        app,
        ctx,
        appearance.x() + pad,
        appearance.y() + 174.0,
        appearance.width() - pad * 2.0,
        &libluna_i18n::tr("Light theme from"),
        app.prefs.light_hour,
        Action::LightHour(-1),
        Action::LightHour(1),
        theme,
    );

    let apply = Rect::from_xywh(
        appearance.x() + pad,
        appearance.bottom() - 20.0 - 32.0,
        appearance.width() - pad * 2.0,
        32.0,
    );
    if apply.y() > appearance.y() + 200.0 {
        paint_button(
            ctx,
            app,
            apply,
            &libluna_i18n::tr("Apply now"),
            ButtonVariant::Default,
        );
        hit(app, Action::ApplyTheme, apply);
    }

    // Zegar.
    heading(
        ctx,
        clock.x() + pad,
        clock.y() + 28.0,
        &libluna_i18n::tr("Clock"),
    );
    paint_switch_row(
        app,
        ctx,
        clock.x() + pad,
        clock.y() + 58.0,
        clock.width() - pad * 2.0,
        &libluna_i18n::tr("Show seconds"),
        app.prefs.show_seconds,
        Action::ShowSeconds,
        theme,
    );
    paint_switch_row(
        app,
        ctx,
        clock.x() + pad,
        clock.y() + 98.0,
        clock.width() - pad * 2.0,
        &libluna_i18n::tr("24-hour clock"),
        app.prefs.use_24h,
        Action::Use24h,
        theme,
    );

    // Prywatność.
    heading(
        ctx,
        privacy.x() + pad,
        privacy.y() + 28.0,
        &libluna_i18n::tr("Privacy"),
    );
    let mut y = paint_wrapped(
        ctx,
        privacy.x() + pad,
        privacy.y() + 52.0,
        privacy.width() - pad * 2.0,
        &libluna_i18n::tr(
            "All events are stored only on this device. Holidays are calculated offline from built-in rules — Luna Clock never sends your data anywhere.",
        ),
        theme.typography.caption.size,
        theme.colors.dim_text,
        16.0,
    );
    y += 8.0;
    caption(
        ctx,
        privacy.x() + pad,
        y,
        &format!(
            "{}: {}",
            libluna_i18n::tr("Events stored"),
            app.events.len()
        ),
        theme.colors.dim_text,
    );
    y += 20.0;
    if let Some(path) = app.events.path() {
        caption(
            ctx,
            privacy.x() + pad,
            y,
            &path.display().to_string(),
            theme.colors.dim_text,
        );
    }

    // Kalendarz: kraj, święta, import/eksport.
    heading(
        ctx,
        calendar.x() + pad,
        calendar.y() + 30.0,
        &libluna_i18n::tr("Calendar"),
    );
    caption(
        ctx,
        calendar.x() + pad,
        calendar.y() + 62.0,
        &libluna_i18n::tr("Country"),
        theme.colors.dim_text,
    );
    ctx.label_left(
        calendar.x() + pad,
        calendar.y() + 86.0,
        &app.country_label(),
        theme.typography.body.size,
        weight::MEDIUM,
        theme.colors.text,
    );
    let change = Rect::from_xywh(
        calendar.right() - pad - 96.0,
        calendar.y() + 64.0,
        96.0,
        32.0,
    );
    paint_button(
        ctx,
        app,
        change,
        &libluna_i18n::tr("Change"),
        ButtonVariant::Flat,
    );
    hit(app, Action::PickCountry, change);

    let detect = Rect::from_xywh(
        calendar.x() + pad,
        calendar.y() + 108.0,
        calendar.width() - pad * 2.0,
        28.0,
    );
    if app.detected_country.is_some() {
        let text = format!(
            "{}: {} ({})",
            libluna_i18n::tr("Detected"),
            libluna_i18n::tr(holidays::country_name(app.detected_country.unwrap_or(""))),
            app.detected_country.unwrap_or("")
        );
        ctx.label_left(
            detect.x(),
            detect.center().y,
            &text,
            theme.typography.caption.size,
            weight::NORMAL,
            theme.colors.dim_text,
        );
        let use_detected = Rect::from_xywh(calendar.right() - pad - 116.0, detect.y(), 116.0, 28.0);
        paint_button(
            ctx,
            app,
            use_detected,
            &libluna_i18n::tr("Use detected"),
            ButtonVariant::Flat,
        );
        hit(app, Action::DetectCountry, use_detected);
    } else {
        caption(
            ctx,
            detect.x(),
            detect.center().y,
            &libluna_i18n::tr("Country is detected from your language and time zone."),
            theme.colors.dim_text,
        );
    }

    paint_switch_row(
        app,
        ctx,
        calendar.x() + pad,
        calendar.y() + 148.0,
        calendar.width() - pad * 2.0,
        &libluna_i18n::tr("Show holidays"),
        app.prefs.show_holidays,
        Action::ShowHolidays,
        theme,
    );

    // Format daty — cztery niezależne wybory, każdy cyklicznie klikany.
    caption(
        ctx,
        calendar.x() + pad,
        calendar.y() + 200.0,
        &libluna_i18n::tr("Date format"),
        theme.colors.dim_text,
    );
    let date_button = (calendar.width() - pad * 2.0).min(190.0);
    paint_choice_row(
        app,
        ctx,
        calendar.x() + pad,
        calendar.y() + 218.0,
        calendar.width() - pad * 2.0,
        &libluna_i18n::tr("Fields"),
        &app.prefs.date_format.fields.label(),
        Action::CycleDateFields,
        date_button,
        theme,
    );
    paint_choice_row(
        app,
        ctx,
        calendar.x() + pad,
        calendar.y() + 254.0,
        calendar.width() - pad * 2.0,
        &libluna_i18n::tr("Style"),
        &app.prefs.date_format.style.label(),
        Action::CycleDateStyle,
        date_button,
        theme,
    );
    paint_choice_row(
        app,
        ctx,
        calendar.x() + pad,
        calendar.y() + 290.0,
        calendar.width() - pad * 2.0,
        &libluna_i18n::tr("Order"),
        &app.prefs.date_format.order.label(),
        Action::CycleDateOrder,
        date_button,
        theme,
    );
    paint_choice_row(
        app,
        ctx,
        calendar.x() + pad,
        calendar.y() + 326.0,
        calendar.width() - pad * 2.0,
        &libluna_i18n::tr("Separator"),
        app.prefs.date_format.separator.label(),
        Action::CycleDateSeparator,
        date_button,
        theme,
    );

    ctx.scene.fill_rect(
        Rect::from_xywh(
            calendar.x() + pad,
            calendar.y() + 368.0,
            calendar.width() - pad * 2.0,
            1.0,
        ),
        theme.colors.border,
    );

    let mut upcoming_y = calendar.y() + 390.0;
    caption(
        ctx,
        calendar.x() + pad,
        upcoming_y,
        &libluna_i18n::tr("Upcoming holidays"),
        theme.colors.dim_text,
    );
    upcoming_y += 22.0;
    let country = app.country_code();
    let mut upcoming = Vec::new();
    if !country.is_empty() {
        for year in [app.now.date.year, app.now.date.year + 1] {
            for holiday in holidays::year_holidays(country, year) {
                if holiday.date >= app.now.date {
                    upcoming.push(holiday);
                }
            }
            if upcoming.len() >= 5 {
                break;
            }
        }
    }
    if upcoming.is_empty() {
        caption(
            ctx,
            calendar.x() + pad,
            upcoming_y,
            &libluna_i18n::tr("Choose a country to see its public holidays."),
            theme.colors.dim_text,
        );
    } else {
        for holiday in upcoming.iter().take(2) {
            ctx.scene.fill_circle(
                Point::new(calendar.x() + pad + 4.0, upcoming_y),
                3.0,
                theme.colors.warning,
            );
            ctx.label_left(
                calendar.x() + pad + 14.0,
                upcoming_y,
                &format!(
                    "{} — {}",
                    format_date(app.prefs.date_format, holiday.date, false),
                    libluna_i18n::tr(holiday.name)
                ),
                theme.typography.caption.size,
                weight::NORMAL,
                theme.colors.text,
            );
            upcoming_y += 22.0;
        }
    }

    let button_y = calendar.bottom() - 20.0 - 36.0;
    let button_w = (calendar.width() - pad * 2.0 - 10.0) / 2.0;
    let import = Rect::from_xywh(calendar.x() + pad, button_y, button_w, 36.0);
    let export = Rect::from_xywh(import.right() + 10.0, button_y, button_w, 36.0);
    paint_button(
        ctx,
        app,
        import,
        &libluna_i18n::tr("Import .ics"),
        ButtonVariant::Default,
    );
    paint_button(
        ctx,
        app,
        export,
        &libluna_i18n::tr("Export .ics"),
        ButtonVariant::Flat,
    );
    hit(app, Action::ImportOpen, import);
    hit(app, Action::ExportIcs, export);
}

fn paint_hour_row(
    app: &mut App,
    ctx: &mut PaintCtx,
    x: f64,
    y: f64,
    width: f64,
    label: &str,
    hour: u32,
    minus_action: Action,
    plus_action: Action,
    theme: Theme,
) {
    ctx.label_left(
        x,
        y + 16.0,
        label,
        theme.typography.body.size,
        weight::NORMAL,
        theme.colors.text,
    );
    let plus = Rect::from_xywh(x + width - 30.0, y, 30.0, 32.0);
    let minus = Rect::from_xywh(plus.x() - 6.0 - 30.0, y, 30.0, 32.0);
    let value = Rect::from_xywh(minus.x() - 6.0 - 56.0, y, 56.0, 32.0);
    paint_small_button(ctx, app, minus, "−");
    paint_small_button(ctx, app, plus, "+");
    ctx.label_center(
        value.center().x,
        value.center().y,
        &format!("{hour:02}:00"),
        theme.typography.body.size,
        weight::MEDIUM,
        theme.colors.text,
    );
    hit(app, minus_action, minus);
    hit(app, plus_action, plus);
}

fn paint_switch_row(
    app: &mut App,
    ctx: &mut PaintCtx,
    x: f64,
    y: f64,
    width: f64,
    label: &str,
    on: bool,
    action: Action,
    theme: Theme,
) {
    ctx.label_left(
        x,
        y + theme.metrics.switch.height / 2.0,
        label,
        theme.typography.body.size,
        weight::NORMAL,
        theme.colors.text,
    );
    let switch = Rect::from_xywh(
        x + width - theme.metrics.switch.width,
        y,
        theme.metrics.switch.width,
        theme.metrics.switch.height,
    );
    paint_switch(ctx, app, switch, on);
    hit(app, action, switch);
}

/// Wiersz wyboru cyklicznego: etykieta po lewej, wartość na płaskim przycisku.
fn paint_choice_row(
    app: &mut App,
    ctx: &mut PaintCtx,
    x: f64,
    y: f64,
    width: f64,
    label: &str,
    value: &str,
    action: Action,
    button_w: f64,
    theme: Theme,
) {
    ctx.label_left(
        x,
        y + 15.0,
        label,
        theme.typography.body.size,
        weight::NORMAL,
        theme.colors.text,
    );
    let button = Rect::from_xywh(x + width - button_w, y, button_w, 30.0);
    paint_button(ctx, app, button, value, ButtonVariant::Flat);
    hit(app, action, button);
}

// ── Modal: wydarzenie ──

fn paint_modal_overlay(ctx: &mut PaintCtx, size: Size) {
    ctx.scene
        .fill_rect(Rect::from_size(size), Color::rgba(0.0, 0.0, 0.0, 0.38));
}

fn modal_card(ctx: &mut PaintCtx, size: Size, width: f64, height: f64) -> Rect {
    let width = width.min(size.width - 60.0).max(280.0);
    let height = height.min(size.height - 60.0).max(200.0);
    let card = Rect::from_xywh(
        (size.width - width) / 2.0,
        (size.height - height) / 2.0,
        width,
        height,
    );
    let theme = *ctx.theme;
    ctx.scene.fill_rounded_rect(
        Rect::from_xywh(card.x() + 2.0, card.y() + 6.0, card.width(), card.height()),
        theme.radius.large,
        Color::rgba(0.0, 0.0, 0.0, 0.22),
    );
    paint_card(ctx, card);
    card
}

fn paint_form(app: &mut App, ctx: &mut PaintCtx, size: Size) {
    paint_modal_overlay(ctx, size);
    let theme = *ctx.theme;
    let card = modal_card(ctx, size, 540.0, 460.0);
    app.modal_rect = Some(card);
    let pointer = app.pointer;
    let pad = 22.0;
    let inner_w = card.width() - pad * 2.0;

    let Some(form) = app.form.as_mut() else {
        return;
    };
    let hits = &mut app.hits;

    heading(
        ctx,
        card.x() + pad,
        card.y() + 28.0,
        &if form.editing.is_some() {
            libluna_i18n::tr("Edit event")
        } else {
            libluna_i18n::tr("Add event")
        },
    );
    caption(
        ctx,
        card.x() + pad,
        card.y() + 54.0,
        &format_date(app.prefs.date_format, form.date, true),
        theme.colors.dim_text,
    );

    form.title.set_focused(form.focus == FormField::Title);
    form.time.set_focused(form.focus == FormField::Time);
    form.note.set_focused(form.focus == FormField::Note);

    let mut y = card.y() + 78.0;
    let field_h = theme.metrics.entry.height.max(32.0);

    caption(
        ctx,
        card.x() + pad,
        y,
        &libluna_i18n::tr("Title"),
        theme.colors.dim_text,
    );
    let title = Rect::from_xywh(card.x() + pad, y + 12.0, inner_w, field_h);
    form.title.paint(ctx, title);
    push_hit(hits, Action::FormFocus(FormField::Title), title);
    y += 12.0 + field_h + 16.0;

    caption(
        ctx,
        card.x() + pad,
        y,
        &libluna_i18n::tr("Time (HH:MM, blank for all day)"),
        theme.colors.dim_text,
    );
    let time = Rect::from_xywh(card.x() + pad, y + 12.0, inner_w, field_h);
    form.time.paint(ctx, time);
    push_hit(hits, Action::FormFocus(FormField::Time), time);
    y += 12.0 + field_h + 16.0;

    caption(
        ctx,
        card.x() + pad,
        y,
        &libluna_i18n::tr("Duration"),
        theme.colors.dim_text,
    );
    let dur_y = y + 10.0;
    let minus = Rect::from_xywh(card.x() + pad, dur_y, 34.0, field_h);
    let plus = Rect::from_xywh(card.x() + pad + inner_w - 34.0, dur_y, 34.0, field_h);
    paint_small_button_state(
        ctx,
        minus,
        "−",
        pointer.is_some_and(|point| minus.contains(point)),
    );
    paint_small_button_state(
        ctx,
        plus,
        "+",
        pointer.is_some_and(|point| plus.contains(point)),
    );
    push_hit(hits, Action::FormDuration(-1), minus);
    push_hit(hits, Action::FormDuration(1), plus);
    let duration_text = if form.time.is_empty() {
        libluna_i18n::tr("All day")
    } else {
        duration_label(form.duration())
    };
    ctx.label_center(
        card.center().x,
        dur_y + field_h / 2.0,
        &duration_text,
        theme.typography.body.size,
        weight::MEDIUM,
        theme.colors.text,
    );
    y += 10.0 + field_h + 16.0;

    caption(
        ctx,
        card.x() + pad,
        y,
        &libluna_i18n::tr("Note (optional)"),
        theme.colors.dim_text,
    );
    let note = Rect::from_xywh(card.x() + pad, y + 12.0, inner_w, field_h);
    form.note.paint(ctx, note);
    push_hit(hits, Action::FormFocus(FormField::Note), note);
    y += 12.0 + field_h + 10.0;

    if let Some(error) = form.error {
        ctx.label_left(
            card.x() + pad,
            y,
            &libluna_i18n::tr(error),
            theme.typography.caption.size,
            weight::MEDIUM,
            theme.colors.error,
        );
    }

    let button_h = 38.0;
    let save = Rect::from_xywh(
        card.right() - pad - 110.0,
        card.bottom() - 20.0 - button_h,
        110.0,
        button_h,
    );
    let cancel = Rect::from_xywh(save.x() - 10.0 - 100.0, save.y(), 100.0, button_h);
    paint_button_state(
        ctx,
        cancel,
        &libluna_i18n::tr("Cancel"),
        ButtonVariant::Flat,
        pointer.is_some_and(|point| cancel.contains(point)),
    );
    paint_button_state(
        ctx,
        save,
        &libluna_i18n::tr("Save"),
        ButtonVariant::Suggested,
        pointer.is_some_and(|point| save.contains(point)),
    );
    push_hit(hits, Action::FormCancel, cancel);
    push_hit(hits, Action::FormSave, save);
}

// ── Modal: lista stref/krajów ──

fn paint_picker(app: &mut App, ctx: &mut PaintCtx, size: Size) {
    paint_modal_overlay(ctx, size);
    let theme = *ctx.theme;
    let card = modal_card(ctx, size, 540.0, 520.0);
    app.modal_rect = Some(card);
    let pad = 22.0;
    let Some(kind) = app.picker.as_ref().map(|picker| picker.kind) else {
        return;
    };
    let values = app.picker_values();
    let selected = app
        .picker
        .as_ref()
        .map(|picker| picker.selected)
        .unwrap_or(0)
        .min(values.len().saturating_sub(1));
    let pointer = app.pointer;
    let title = match kind {
        PickerKind::Zone => libluna_i18n::tr("Add time zone"),
        PickerKind::Country => libluna_i18n::tr("Choose country"),
    };
    heading(ctx, card.x() + pad, card.y() + 28.0, &title);

    let search = Rect::from_xywh(
        card.x() + pad,
        card.y() + 54.0,
        card.width() - pad * 2.0,
        theme.metrics.entry.height.max(32.0),
    );
    let (picker, hits) = match app.picker.as_mut() {
        Some(picker) => (picker, &mut app.hits),
        None => return,
    };
    picker.query.paint(ctx, search);

    let list_top = search.bottom() + 12.0;
    let list_bottom = card.bottom() - 20.0 - 38.0 - 12.0;
    let row_h = 34.0;
    let visible = (((list_bottom - list_top) / row_h).floor() as usize).max(1);
    let start = if selected >= visible {
        selected + 1 - visible
    } else {
        0
    };
    if values.is_empty() {
        caption(
            ctx,
            card.x() + pad,
            list_top + 14.0,
            &libluna_i18n::tr("No results"),
            theme.colors.dim_text,
        );
    }
    for (offset, index) in (start..values.len().min(start + visible)).enumerate() {
        let row = Rect::from_xywh(
            card.x() + pad,
            list_top + offset as f64 * row_h,
            card.width() - pad * 2.0,
            row_h - 2.0,
        );
        if index == selected {
            ctx.scene.fill_rounded_rect(
                row,
                theme.radius.small,
                theme.colors.accent.scale_alpha(0.18),
            );
            ctx.scene
                .stroke_rounded_rect(row, theme.radius.small, 1.5, theme.colors.accent);
        } else if pointer.is_some_and(|point| row.contains(point)) {
            ctx.scene.fill_rounded_rect(
                row,
                theme.radius.small,
                theme.colors.shade.scale_alpha(1.6),
            );
        }
        let label = match kind {
            PickerKind::Zone => values[index].clone(),
            PickerKind::Country => format!(
                "{} ({})",
                libluna_i18n::tr(holidays::country_name(&values[index])),
                values[index]
            ),
        };
        ctx.label_left(
            row.x() + 10.0,
            row.center().y,
            &label,
            theme.typography.body.size,
            weight::NORMAL,
            theme.colors.text,
        );
        push_hit(hits, Action::PickerApply(index), row);
    }

    let cancel = Rect::from_xywh(
        card.right() - pad - 100.0,
        card.bottom() - 20.0 - 38.0,
        100.0,
        38.0,
    );
    paint_button_state(
        ctx,
        cancel,
        &libluna_i18n::tr("Cancel"),
        ButtonVariant::Flat,
        pointer.is_some_and(|point| cancel.contains(point)),
    );
    push_hit(hits, Action::PickerCancel, cancel);
}

// ── Modal: import .ics ──

fn paint_importer(app: &mut App, ctx: &mut PaintCtx, size: Size) {
    paint_modal_overlay(ctx, size);
    let theme = *ctx.theme;
    let card = modal_card(ctx, size, 500.0, 270.0);
    app.modal_rect = Some(card);
    let pad = 22.0;
    let pointer = app.pointer;
    let Some(importer) = app.importer.as_mut() else {
        return;
    };
    let hits = &mut app.hits;
    heading(
        ctx,
        card.x() + pad,
        card.y() + 28.0,
        &libluna_i18n::tr("Import .ics"),
    );
    paint_wrapped(
        ctx,
        card.x() + pad,
        card.y() + 58.0,
        card.width() - pad * 2.0,
        &libluna_i18n::tr(
            "Choose a local iCalendar file. The file is read on this device and is never uploaded.",
        ),
        theme.typography.caption.size,
        theme.colors.dim_text,
        16.0,
    );
    let entry = Rect::from_xywh(
        card.x() + pad,
        card.y() + 110.0,
        card.width() - pad * 2.0,
        theme.metrics.entry.height.max(32.0),
    );
    importer.path.paint(ctx, entry);
    if let Some(error) = importer.error {
        ctx.label_left(
            card.x() + pad,
            entry.bottom() + 16.0,
            &libluna_i18n::tr(error),
            theme.typography.caption.size,
            weight::MEDIUM,
            theme.colors.error,
        );
    }
    let button_h = 38.0;
    let import = Rect::from_xywh(
        card.right() - pad - 110.0,
        card.bottom() - 20.0 - button_h,
        110.0,
        button_h,
    );
    let cancel = Rect::from_xywh(import.x() - 10.0 - 100.0, import.y(), 100.0, button_h);
    paint_button_state(
        ctx,
        cancel,
        &libluna_i18n::tr("Cancel"),
        ButtonVariant::Flat,
        pointer.is_some_and(|point| cancel.contains(point)),
    );
    paint_button_state(
        ctx,
        import,
        &libluna_i18n::tr("Import"),
        ButtonVariant::Suggested,
        pointer.is_some_and(|point| import.contains(point)),
    );
    push_hit(hits, Action::ImportCancel, cancel);
    push_hit(hits, Action::ImportSubmit, import);
}

// ── Zdarzenia ──

pub fn handle_events(
    app: &mut App,
    events: &[Event],
    size: Size,
    theme: &Theme,
    text: &TextEngine,
) {
    let layout = chrome(size, theme, text);
    let header = HeaderBar::new().controls(ControlsSide::Right);
    let modal_open = app.form.is_some() || app.picker.is_some() || app.importer.is_some();

    for event in events {
        match event {
            Event::PointerButton(button)
                if button.button == PointerButton::Left && button.state == ButtonState::Pressed =>
            {
                if let Some(control) = header.control_at(layout.header, theme, button.position) {
                    match control {
                        WindowControl::Minimize => app.pending_minimize = true,
                        WindowControl::Maximize => app.pending_maximize = true,
                        WindowControl::Close => app.pending_close = true,
                    }
                    continue;
                }
                let action = app
                    .hits
                    .iter()
                    .rev()
                    .find(|(_, rect)| rect.contains(button.position))
                    .map(|(action, _)| *action);
                if let Some(action) = action {
                    apply_action(app, action);
                } else if !modal_open && layout.header.contains(button.position) {
                    // Przeciąganie okna za wolny obszar paska tytułu.
                    app.pending_move = true;
                } else if modal_open
                    && app
                        .modal_rect
                        .is_none_or(|rect| !rect.contains(button.position))
                {
                    app.form = None;
                    app.picker = None;
                    app.importer = None;
                }
            }
            Event::PointerMotion { position, .. } => {
                app.pointer = Some(*position);
                app.controls_hover = header.control_at(layout.header, theme, *position);
            }
            Event::PointerLeave => {
                app.pointer = None;
                app.controls_hover = None;
            }
            Event::Scroll(scroll) => {
                if app.picker.is_some() {
                    app.move_picker(if scroll.dy < 0.0 { 1 } else { -1 });
                }
            }
            Event::Key(key) if key.state == ButtonState::Pressed => {
                handle_key(app, key);
            }
            Event::Text(input) => insert_text(app, input),
            Event::Paste(input) => insert_text(app, input),
            Event::Window(WindowEvent::CloseRequested) => app.pending_close = true,
            _ => {}
        }
    }
}

fn handle_key(app: &mut App, key: &KeyEvent) {
    // Schowek działa tylko wtedy, gdy otwarte jest pole modalne.
    if focused_entry_mut(app).is_some() {
        if let Some(command) = {
            let entry = focused_entry_mut(app).expect("entry");
            clipboard_command(entry, key)
        } {
            match command {
                TextClipboardCommand::Copy(text) | TextClipboardCommand::Cut(text) => {
                    if let Some(text) = text {
                        app.clipboard_text = Some(text);
                    }
                }
                TextClipboardCommand::Paste => app.pending_paste = true,
            }
            return;
        }
    }

    if app.form.is_some() {
        match key.keysym {
            KEY_ESCAPE => app.form = None,
            KEY_ENTER | KEY_KP_ENTER => {
                app.save_form();
            }
            KEY_TAB => {
                if let Some(form) = app.form.as_mut() {
                    form.focus_next(key.modifiers.shift);
                }
            }
            KEY_BACKSPACE => {
                if let Some(form) = app.form.as_mut() {
                    form.focused_mut().backspace();
                }
            }
            KEY_DELETE => {
                if let Some(form) = app.form.as_mut() {
                    form.focused_mut().delete_forward();
                }
            }
            KEY_LEFT => {
                if let Some(form) = app.form.as_mut() {
                    form.focused_mut().move_left_selecting(key.modifiers.shift);
                }
            }
            KEY_RIGHT => {
                if let Some(form) = app.form.as_mut() {
                    form.focused_mut().move_right_selecting(key.modifiers.shift);
                }
            }
            KEY_HOME => {
                if let Some(form) = app.form.as_mut() {
                    form.focused_mut().home_selecting(key.modifiers.shift);
                }
            }
            KEY_END => {
                if let Some(form) = app.form.as_mut() {
                    form.focused_mut().end_selecting(key.modifiers.shift);
                }
            }
            0x61 if key.modifiers.ctrl => {
                if let Some(form) = app.form.as_mut() {
                    form.focused_mut().select_all();
                }
            }
            _ => {
                if !key.modifiers.ctrl {
                    if let Some(text) = &key.text {
                        insert_plain(app, text);
                    }
                }
            }
        }
        return;
    }

    if app.picker.is_some() {
        match key.keysym {
            KEY_ESCAPE => app.picker = None,
            KEY_ENTER | KEY_KP_ENTER => app.apply_picker(),
            KEY_UP => app.move_picker(-1),
            KEY_DOWN | KEY_TAB => app.move_picker(1),
            KEY_BACKSPACE => {
                if let Some(picker) = app.picker.as_mut() {
                    picker.query.backspace();
                    picker.selected = 0;
                }
            }
            KEY_DELETE => {
                if let Some(picker) = app.picker.as_mut() {
                    picker.query.delete_forward();
                    picker.selected = 0;
                }
            }
            KEY_LEFT => {
                if let Some(picker) = app.picker.as_mut() {
                    picker.query.move_left();
                }
            }
            KEY_RIGHT => {
                if let Some(picker) = app.picker.as_mut() {
                    picker.query.move_right();
                }
            }
            KEY_HOME => {
                if let Some(picker) = app.picker.as_mut() {
                    picker.query.home();
                }
            }
            KEY_END => {
                if let Some(picker) = app.picker.as_mut() {
                    picker.query.end();
                }
            }
            0x61 if key.modifiers.ctrl => {
                if let Some(picker) = app.picker.as_mut() {
                    picker.query.select_all();
                }
            }
            _ => {
                if !key.modifiers.ctrl {
                    if let Some(text) = &key.text {
                        insert_plain(app, text);
                    }
                }
            }
        }
        return;
    }

    if app.importer.is_some() {
        match key.keysym {
            KEY_ESCAPE => app.importer = None,
            KEY_ENTER | KEY_KP_ENTER => {
                app.import_ics();
            }
            KEY_BACKSPACE => {
                if let Some(importer) = app.importer.as_mut() {
                    importer.path.backspace();
                }
            }
            KEY_DELETE => {
                if let Some(importer) = app.importer.as_mut() {
                    importer.path.delete_forward();
                }
            }
            KEY_LEFT => {
                if let Some(importer) = app.importer.as_mut() {
                    importer.path.move_left();
                }
            }
            KEY_RIGHT => {
                if let Some(importer) = app.importer.as_mut() {
                    importer.path.move_right();
                }
            }
            KEY_HOME => {
                if let Some(importer) = app.importer.as_mut() {
                    importer.path.home();
                }
            }
            KEY_END => {
                if let Some(importer) = app.importer.as_mut() {
                    importer.path.end();
                }
            }
            0x61 if key.modifiers.ctrl => {
                if let Some(importer) = app.importer.as_mut() {
                    importer.path.select_all();
                }
            }
            _ => {
                if !key.modifiers.ctrl {
                    if let Some(text) = &key.text {
                        insert_plain(app, text);
                    }
                }
            }
        }
        return;
    }

    // Skróty globalne.
    match key.keysym {
        0x31..=0x35 => app.set_tab(Tab::from_index((key.keysym - 0x31) as usize)),
        KEY_TAB => {
            let index = app.tab.index();
            let next = (index + 1) % Tab::ALL.len();
            app.set_tab(Tab::ALL[next]);
        }
        KEY_SPACE => match app.tab {
            Tab::Timer => app.timer.start_pause(),
            Tab::Stopwatch => app.stopwatch.start_pause(),
            Tab::Calendar => app.go_today(),
            _ => {}
        },
        0x74 if app.tab == Tab::Calendar => app.go_today(), // t
        0x6e if app.tab == Tab::Calendar => app.open_new_event(), // n
        0x71 if key.modifiers.ctrl => app.pending_close = true, // Ctrl+Q
        KEY_LEFT if app.tab == Tab::Calendar => app.month_prev(),
        KEY_RIGHT if app.tab == Tab::Calendar => app.month_next(),
        _ => {}
    }
}

fn focused_entry_mut(app: &mut App) -> Option<&mut Entry> {
    if let Some(form) = app.form.as_mut() {
        let field = form.focus;
        return Some(form.entry_mut(field));
    }
    if let Some(picker) = app.picker.as_mut() {
        return Some(&mut picker.query);
    }
    app.importer.as_mut().map(|importer| &mut importer.path)
}

fn insert_text(app: &mut App, text: &str) {
    if app.form.is_none() && app.picker.is_none() && app.importer.is_none() {
        return;
    }
    insert_plain(app, text);
}

fn insert_plain(app: &mut App, text: &str) {
    let plain: String = text.chars().filter(|ch| !ch.is_control()).collect();
    if plain.is_empty() {
        return;
    }
    if let Some(entry) = focused_entry_mut(app) {
        entry.insert_str(&plain);
    }
}

fn apply_action(app: &mut App, action: Action) {
    match action {
        Action::Window(control) => match control {
            WindowControl::Minimize => app.pending_minimize = true,
            WindowControl::Maximize => app.pending_maximize = true,
            WindowControl::Close => app.pending_close = true,
        },
        Action::Tab(tab) => app.set_tab(tab),
        Action::AddZone => app.open_picker(PickerKind::Zone),
        Action::RemoveZone(index) => app.remove_zone(index),
        Action::TimerUnit(unit, delta) => {
            app.timer.adjust(unit, delta);
            app.persist_activity(true);
        }
        Action::TimerPreset(minutes) => {
            app.timer
                .set_duration(Duration::from_secs(minutes as u64 * 60));
            app.prefs.timer_minutes = minutes;
            app.prefs.save();
            app.persist_activity(true);
        }
        Action::TimerToggle => {
            app.timer.start_pause();
            app.persist_activity(true);
        }
        Action::TimerReset => {
            app.timer.reset();
            app.persist_activity(true);
        }
        Action::WatchToggle => {
            app.stopwatch.start_pause();
            app.persist_activity(true);
        }
        Action::WatchLap => {
            app.stopwatch.lap();
            app.persist_activity(true);
        }
        Action::WatchReset => {
            app.stopwatch.reset();
            app.persist_activity(true);
        }
        Action::MonthPrev => app.month_prev(),
        Action::MonthNext => app.month_next(),
        Action::Today => app.go_today(),
        Action::Day(day) => app.select_day(day),
        Action::NewEvent => app.open_new_event(),
        Action::EditEvent(id) => app.open_edit_event(id),
        Action::DeleteEvent(id) => app.delete_event(id),
        Action::FormSave => {
            app.save_form();
        }
        Action::FormCancel => app.form = None,
        Action::FormFocus(field) => {
            if let Some(form) = app.form.as_mut() {
                form.focus = field;
                form.title.set_focused(field == FormField::Title);
                form.time.set_focused(field == FormField::Time);
                form.note.set_focused(field == FormField::Note);
            }
        }
        Action::FormDuration(delta) => app.cycle_duration(delta),
        Action::PickerApply(index) => {
            if let Some(picker) = app.picker.as_mut() {
                picker.selected = index;
            }
            app.apply_picker();
        }
        Action::PickerCancel => app.picker = None,
        Action::ImportOpen => app.open_importer(),
        Action::ImportSubmit => {
            app.import_ics();
        }
        Action::ImportCancel => app.importer = None,
        Action::AutoTheme => {
            app.prefs.auto_theme = !app.prefs.auto_theme;
            app.prefs.save();
            if app.prefs.auto_theme {
                let dark = app.apply_theme_now();
                app.show_toast(if dark {
                    libluna_i18n::tr("Automatic dark theme")
                } else {
                    libluna_i18n::tr("Automatic light theme")
                });
            } else {
                app.show_toast(libluna_i18n::tr("Automatic theme off"));
            }
        }
        Action::ApplyTheme => {
            let dark = app.apply_theme_now();
            app.show_toast(if dark {
                libluna_i18n::tr("Dark theme")
            } else {
                libluna_i18n::tr("Light theme")
            });
        }
        Action::ShowHolidays => {
            app.prefs.show_holidays = !app.prefs.show_holidays;
            app.prefs.save();
        }
        Action::ShowSeconds => {
            app.prefs.show_seconds = !app.prefs.show_seconds;
            app.prefs.save();
        }
        Action::Use24h => {
            app.prefs.use_24h = !app.prefs.use_24h;
            app.prefs.save();
        }
        Action::CycleDateFields => {
            app.prefs.date_format.fields = app.prefs.date_format.fields.next();
            app.prefs.save();
        }
        Action::CycleDateStyle => {
            app.prefs.date_format.style = app.prefs.date_format.style.next();
            app.prefs.save();
        }
        Action::CycleDateOrder => {
            app.prefs.date_format.order = app.prefs.date_format.order.next();
            app.prefs.save();
        }
        Action::CycleDateSeparator => {
            app.prefs.date_format.separator = app.prefs.date_format.separator.next();
            app.prefs.save();
        }
        Action::DarkHour(delta) => {
            app.prefs.dark_hour = (app.prefs.dark_hour as i64 + delta).rem_euclid(24) as u32;
            app.prefs.save();
            if app.prefs.auto_theme {
                app.apply_theme_now();
            }
        }
        Action::LightHour(delta) => {
            app.prefs.light_hour = (app.prefs.light_hour as i64 + delta).rem_euclid(24) as u32;
            app.prefs.save();
            if app.prefs.auto_theme {
                app.apply_theme_now();
            }
        }
        Action::PickCountry => app.open_picker(PickerKind::Country),
        Action::DetectCountry => app.use_detected_country(),
        Action::ExportIcs => app.export_ics(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::Prefs;
    use crate::store::EventStore;

    fn app() -> App {
        let mut store = EventStore::from_text("", None);
        store.set_path(None);
        App::new(Prefs::default(), store, None, None)
    }

    #[test]
    fn paints_every_tab_without_a_window() {
        let mut app = app();
        let theme = Theme::dark();
        let text = TextEngine::none();
        let size = Size::new(1120.0, 760.0);
        for tab in Tab::ALL {
            app.set_tab(tab);
            let mut scene = Scene::new();
            paint(&mut app, &mut scene, size, &theme, &text);
            assert!(scene.len() > 10, "strona {tab:?} powinna coś narysować");
        }
    }

    #[test]
    fn tab_switch_clicks_change_page() {
        let mut app = app();
        let theme = Theme::dark();
        let text = TextEngine::none();
        let size = Size::new(1120.0, 760.0);
        let mut scene = Scene::new();
        paint(&mut app, &mut scene, size, &theme, &text);
        let layout = chrome(size, &theme, &text);
        let segments = ViewSwitcher::new(["a", "b", "c", "d", "e"]).segments(layout.tabs);
        let click = Event::PointerButton(libluna_core::PointerEvent {
            position: segments[3].center(),
            button: PointerButton::Left,
            state: ButtonState::Pressed,
            modifiers: libluna_core::Modifiers::NONE,
        });
        handle_events(&mut app, &[click], size, &theme, &text);
        assert_eq!(app.tab, Tab::Calendar);
    }

    #[test]
    fn calendar_day_click_selects_date() {
        let mut app = app();
        app.set_tab(Tab::Calendar);
        let theme = Theme::dark();
        let text = TextEngine::none();
        let size = Size::new(1120.0, 760.0);
        let mut scene = Scene::new();
        paint(&mut app, &mut scene, size, &theme, &text);
        let day_hit = app
            .hits
            .iter()
            .find(|(action, _)| matches!(action, Action::Day(15)))
            .map(|(_, rect)| rect.center())
            .expect("dzień 15 powinien mieć region kliknięcia");
        let click = Event::PointerButton(libluna_core::PointerEvent {
            position: day_hit,
            button: PointerButton::Left,
            state: ButtonState::Pressed,
            modifiers: libluna_core::Modifiers::NONE,
        });
        handle_events(&mut app, &[click], size, &theme, &text);
        assert_eq!(app.selected_date.day, 15);
    }

    #[test]
    fn form_modal_blocks_background_and_escape_closes() {
        let mut app = app();
        app.open_new_event();
        let theme = Theme::dark();
        let text = TextEngine::none();
        let size = Size::new(1120.0, 760.0);
        let mut scene = Scene::new();
        paint(&mut app, &mut scene, size, &theme, &text);
        assert!(app
            .hits
            .iter()
            .all(|(action, _)| !matches!(action, Action::Tab(_))));
        let escape = Event::Key(KeyEvent {
            keysym: KEY_ESCAPE,
            keycode: 0,
            text: None,
            state: ButtonState::Pressed,
            modifiers: libluna_core::Modifiers::NONE,
            repeat: false,
        });
        handle_events(&mut app, &[escape], size, &theme, &text);
        assert!(app.form.is_none());
    }

    #[test]
    fn timer_click_starts_and_pauses() {
        let mut app = app();
        app.set_tab(Tab::Timer);
        let theme = Theme::dark();
        let text = TextEngine::none();
        let size = Size::new(1120.0, 760.0);
        let mut scene = Scene::new();
        paint(&mut app, &mut scene, size, &theme, &text);
        let start = app
            .hits
            .iter()
            .find(|(action, _)| *action == Action::TimerToggle)
            .map(|(_, rect)| rect.center())
            .expect("przycisk Start");
        let event = Event::PointerButton(libluna_core::PointerEvent {
            position: start,
            button: PointerButton::Left,
            state: ButtonState::Pressed,
            modifiers: libluna_core::Modifiers::NONE,
        });
        handle_events(&mut app, &[event.clone()], size, &theme, &text);
        assert!(app.timer.running);
        handle_events(&mut app, &[event], size, &theme, &text);
        assert!(!app.timer.running);
    }

    #[test]
    fn world_clock_add_and_remove() {
        let mut app = app();
        app.prefs.world_clocks.clear();
        app.refresh_clock();
        let theme = Theme::dark();
        let text = TextEngine::none();
        let size = Size::new(1120.0, 760.0);
        let mut scene = Scene::new();
        paint(&mut app, &mut scene, size, &theme, &text);
        let add = app
            .hits
            .iter()
            .find(|(action, _)| *action == Action::AddZone)
            .map(|(_, rect)| rect.center())
            .expect("przycisk dodania strefy");
        let event = Event::PointerButton(libluna_core::PointerEvent {
            position: add,
            button: PointerButton::Left,
            state: ButtonState::Pressed,
            modifiers: libluna_core::Modifiers::NONE,
        });
        handle_events(&mut app, &[event], size, &theme, &text);
        assert!(app.picker.is_some());
    }
}
