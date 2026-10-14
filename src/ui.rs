
use crate::common::*;
use crate::winutil;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

const BG: Color32 = Color32::from_rgb(13, 15, 21);
const SIDEBAR: Color32 = Color32::from_rgb(17, 20, 28);
const SURFACE: Color32 = Color32::from_rgb(22, 26, 36);
const SURFACE_HI: Color32 = Color32::from_rgb(30, 35, 48);
const BORDER: Color32 = Color32::from_rgb(38, 44, 58);
const TEXT: Color32 = Color32::from_rgb(231, 233, 238);
const MUTED: Color32 = Color32::from_rgb(138, 145, 163);
const ACCENT: Color32 = Color32::from_rgb(139, 92, 246);
const ACCENT2: Color32 = Color32::from_rgb(34, 211, 238);
const GOOD: Color32 = Color32::from_rgb(52, 211, 153);
const WARN: Color32 = Color32::from_rgb(251, 191, 36);

const PALETTE: [Color32; 8] = [
    Color32::from_rgb(139, 92, 246),
    Color32::from_rgb(34, 211, 238),
    Color32::from_rgb(244, 114, 182),
    Color32::from_rgb(52, 211, 153),
    Color32::from_rgb(251, 191, 36),
    Color32::from_rgb(96, 165, 250),
    Color32::from_rgb(248, 113, 113),
    Color32::from_rgb(163, 230, 53),
];

const MONTHS: [&str; 12] = ["янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек"];
const MONTHS_FULL: [&str; 12] = ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"];
const WEEKDAYS_ON: [&str; 7] = ["В понедельник", "Во вторник", "В среду", "В четверг", "В пятницу", "В субботу", "В воскресенье"];

fn plural<'a>(n: u64, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let (m10, m100) = (n % 10, n % 100);
    if m10 == 1 && m100 != 11 {
        one
    } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
        few
    } else {
        many
    }
}

fn date_full(d: Date) -> String {
    format!("{} {}", d.d, MONTHS_FULL[(d.m - 1) as usize])
}

const WEEKDAYS: [&str; 7] = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];

#[derive(PartialEq, Clone, Copy)]
enum View {
    Stats,
    Settings,
}

#[derive(PartialEq, Clone, Copy)]
enum Period {
    Day,
    Week,
    Month,
    All,
}

struct Loaded {
    title: String,
    apps: Vec<(String, u64)>,
    total: u64,
    hours: [u64; 24],
    days: Vec<(Date, u64)>,
    active_days: usize,
}

pub struct App {
    view: View,
    period: Period,
    offset: i64,
    data: Loaded,
    paths: HashMap<String, String>,
    names: HashMap<String, String>,
    icons: HashMap<String, Option<egui::TextureHandle>>,
    cfg: Config,
    autostart: bool,
    tracker_alive: bool,
    last_reload: Instant,
    new_exclude: String,
}

pub fn run() {
    if winutil::focus_existing_ui() {
        return;
    }
    let icon = egui::IconData { rgba: crate::icon::icon_rgba(64), width: 64, height: 64 };
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(UI_TITLE)
            .with_inner_size([1000.0, 680.0])
            .with_min_inner_size([760.0, 520.0])
            .with_icon(Arc::new(icon)),
        ..Default::default()
    };
    let _ = eframe::run_native(UI_TITLE, opts, Box::new(|cc| {
        setup_style(&cc.egui_ctx);
        Ok(Box::new(App::new()))
    }));
}

fn setup_style(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(reg) = winutil::windows_font("segoeui.ttf") {
        fonts.font_data.insert("segoe".into(), Arc::new(egui::FontData::from_owned(reg)));
        fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "segoe".into());
    }
    let mut bold = vec![];
    if let Some(b) = winutil::windows_font("seguisb.ttf").or_else(|| winutil::windows_font("segoeuib.ttf")) {
        fonts.font_data.insert("segoe-bold".into(), Arc::new(egui::FontData::from_owned(b)));
        bold.push("segoe-bold".to_string());
    }
    bold.extend(fonts.families[&FontFamily::Proportional].iter().cloned());
    fonts.families.insert(FontFamily::Name("bold".into()), bold);
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = Color32::from_rgb(10, 12, 17);
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.inactive.bg_fill = SURFACE_HI;
    v.widgets.inactive.weak_bg_fill = SURFACE_HI;
    v.widgets.hovered.bg_fill = Color32::from_rgb(40, 46, 62);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(40, 46, 62);
    v.widgets.active.bg_fill = ACCENT;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(8);
    }
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    ctx.options_mut(|o| o.theme_preference = egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 8.0);
        s.spacing.button_padding = Vec2::new(12.0, 6.0);
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(12.0));
    });
}

fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .corner_radius(CornerRadius::same(14))
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(egui::Margin::same(16))
}

fn date_label(d: Date) -> String {
    format!("{} {}", d.d, MONTHS[(d.m - 1) as usize])
}

impl App {
    fn new() -> Self {
        let mut a = App {
            view: View::Stats,
            period: Period::Day,
            offset: 0,
            data: Loaded { title: String::new(), apps: vec![], total: 0, hours: [0; 24], days: vec![], active_days: 0 },
            paths: HashMap::new(),
            names: HashMap::new(),
            icons: HashMap::new(),
            cfg: Config::load(),
            autostart: autostart::is_enabled(),
            tracker_alive: winutil::tracker_running(),
            last_reload: Instant::now(),
            new_exclude: String::new(),
        };
        a.reload();
        a
    }

    fn reload(&mut self) {
        winutil::flush_tracker();
        self.tracker_alive = winutil::tracker_running();
        self.paths = load_app_paths();
        let today = Date::today();
        let dates: Vec<Date> = match self.period {
            Period::Day => vec![today.add(self.offset)],
            Period::Week => {
                let end = today.add(self.offset * 7);
                (0..7).rev().map(|i| end.add(-i)).collect()
            }
            Period::Month => {
                let end = today.add(self.offset * 30);
                (0..30).rev().map(|i| end.add(-i)).collect()
            }
            Period::All => {
                let mut v: Vec<Date> = std::fs::read_dir(data_dir().join("days"))
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|e| Date::parse(e.file_name().to_string_lossy().strip_suffix(".tsv")?))
                    .collect();
                v.sort();
                v
            }
        };
        let mut apps: HashMap<String, u64> = HashMap::new();
        let mut hours = [0u64; 24];
        let mut days = Vec::with_capacity(dates.len());
        for &d in &dates {
            let dd = DayData::load(d);
            let mut day_total = 0;
            for (exe, s) in dd.apps {
                if self.cfg.is_excluded(&exe) {
                    continue;
                }
                day_total += s;
                *apps.entry(exe).or_default() += s;
            }
            for (h, s) in dd.hours.iter().enumerate() {
                hours[h] += s;
            }
            days.push((d, day_total));
        }
        let mut apps: Vec<(String, u64)> = apps.into_iter().filter(|(_, s)| *s > 0).collect();
        apps.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let total = apps.iter().map(|a| a.1).sum();
        let title = match self.period {
            Period::Day => match self.offset {
                0 => "Сегодня".to_string(),
                -1 => "Вчера".to_string(),
                _ => {
                    let d = dates[0];
                    format!("{}, {}", WEEKDAYS[d.weekday() as usize], date_label(d))
                }
            },
            Period::Week | Period::Month => format!("{} — {}", date_label(dates[0]), date_label(*dates.last().unwrap())),
            Period::All => match (dates.first(), dates.last()) {
                (Some(a), Some(b)) => format!("С {} {} года по {}", date_full(*a), a.y, date_full(*b)),
                _ => "Всё время".into(),
            },
        };
        if self.period == Period::All && days.len() > 90 {
            days.drain(..days.len() - 90);
        }
        self.data = Loaded {
            active_days: days.iter().filter(|d| d.1 > 0).count(),
            title,
            apps,
            total,
            hours,
            days,
        };
        self.last_reload = Instant::now();
    }

    fn name_of(&mut self, exe: &str) -> String {
        if let Some(n) = self.names.get(exe) {
            return n.clone();
        }
        let n = self
            .paths
            .get(exe)
            .and_then(|p| winutil::app_description(p))
            .unwrap_or_else(|| {
                let stem = exe.trim_end_matches(".exe");
                let mut c = stem.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            });
        self.names.insert(exe.to_string(), n.clone());
        n
    }

    fn icon_of(&mut self, ctx: &egui::Context, exe: &str) -> Option<egui::TextureHandle> {
        if let Some(t) = self.icons.get(exe) {
            return t.clone();
        }
        let tex = self.paths.get(exe).and_then(|p| winutil::app_icon_rgba(p)).map(|(w, h, px)| {
            ctx.load_texture(format!("icon-{exe}"), egui::ColorImage::from_rgba_unmultiplied([w, h], &px), egui::TextureOptions::LINEAR)
        });
        self.icons.insert(exe.to_string(), tex.clone());
        tex
    }

    fn save_cfg(&mut self) {
        self.cfg.save();
        winutil::notify_reload();
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let (r, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
            if let Some(t) = self.logo(ui.ctx()) {
                egui::Image::new(&t).paint_at(ui, r);
            }
            ui.label(egui::RichText::new("Tempo").font(bold(20.0)).color(TEXT));
        });
        ui.add_space(22.0);
        for (v, label, glyph) in [(View::Stats, "Статистика", 0u8), (View::Settings, "Настройки", 1u8)] {
            if nav_button(ui, glyph, label, self.view == v) {
                self.view = v;
            }
        }

        let bottom = ui.max_rect().bottom();
        let h = 92.0;
        let rect = Rect::from_min_max(Pos2::new(ui.max_rect().left() + 12.0, bottom - h - 14.0), Pos2::new(ui.max_rect().right() - 12.0, bottom - 14.0));
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(12), SURFACE);
        p.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
        let (color, text) = if !self.tracker_alive {
            (Color32::from_rgb(248, 113, 113), "Трекер выключен")
        } else if self.cfg.paused {
            (WARN, "Трекер на паузе")
        } else {
            (GOOD, "Трекер работает")
        };
        p.circle_filled(rect.left_top() + Vec2::new(18.0, 22.0), 4.5, color);
        p.text(rect.left_top() + Vec2::new(30.0, 22.0), Align2::LEFT_CENTER, text, bold(13.0), TEXT);
        let btn_rect = Rect::from_min_size(rect.left_top() + Vec2::new(10.0, 44.0), Vec2::new(rect.width() - 20.0, 34.0));
        let label = if !self.tracker_alive {
            "Запустить трекер"
        } else if self.cfg.paused {
            "Продолжить отслеживание"
        } else {
            "Поставить на паузу"
        };
        if ui.put(btn_rect, egui::Button::new(label)).clicked() {
            if !self.tracker_alive {
                winutil::start_tracker();
                std::thread::sleep(Duration::from_millis(300));
                self.tracker_alive = winutil::tracker_running();
            } else {
                self.cfg.paused = !self.cfg.paused;
                self.save_cfg();
            }
        }
    }

    fn logo(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if let Some(t) = self.icons.get("__logo") {
            return t.clone();
        }
        let t = ctx.load_texture("logo", egui::ColorImage::from_rgba_unmultiplied([64, 64], &crate::icon::icon_rgba(64)), egui::TextureOptions::LINEAR);
        self.icons.insert("__logo".into(), Some(t.clone()));
        Some(t)
    }

    fn stats(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&self.data.title).font(bold(24.0)).color(TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut changed = false;
                if self.period != Period::All {
                    let can_fwd = self.offset < 0;
                    if ui.add_enabled(can_fwd, egui::Button::new("›").min_size(Vec2::new(32.0, 30.0))).clicked() {
                        self.offset += 1;
                        changed = true;
                    }
                    if ui.add(egui::Button::new("‹").min_size(Vec2::new(32.0, 30.0))).clicked() {
                        self.offset -= 1;
                        changed = true;
                    }
                    ui.add_space(8.0);
                }
                for (p, l) in [(Period::All, "Всё время"), (Period::Month, "30 дней"), (Period::Week, "Неделя"), (Period::Day, "День")] {
                    if segment(ui, l, self.period == p) && self.period != p {
                        self.period = p;
                        self.offset = 0;
                        changed = true;
                    }
                }
                if changed {
                    self.reload();
                }
            });
        });
        ui.add_space(10.0);

        let d = &self.data;
        let top = d.apps.first().cloned();
        let top_name = top.as_ref().map(|t| t.0.clone());
        let top_name = top_name.map(|e| self.name_of(&e));
        let d = &self.data;
        let mut cards: Vec<(&str, String, String)> = vec![(
            "Общее время за период",
            fmt_dur(d.total),
            format!("Вы использовали {} {}.", d.apps.len(), plural(d.apps.len() as u64, "приложение", "приложения", "приложений")),
        )];
        cards.push((
            "Самое используемое приложение",
            top_name.clone().unwrap_or_else(|| "—".into()),
            top.as_ref().map(|t| format!("Вы провели в нём {}.", fmt_dur(t.1))).unwrap_or_else(|| "Данных пока нет.".into()),
        ));
        if self.period == Period::Day {
            let (ph, ps) = d.hours.iter().enumerate().max_by_key(|x| *x.1).map(|(h, s)| (h, *s)).unwrap_or((0, 0));
            cards.push((
                "Самый активный час",
                if ps > 0 { format!("{ph:02}:00 – {:02}:00", (ph + 1) % 24) } else { "—".into() },
                if ps > 0 { format!("Вы были активны {}.", fmt_dur(ps)) } else { "Данных пока нет.".into() },
            ));
        } else {
            let avg = if d.active_days > 0 { d.total / d.active_days as u64 } else { 0 };
            let total_days = d.days.len() as u64;
            cards.push((
                "Среднее время за активный день",
                fmt_dur(avg),
                format!("Вы были активны {} {} из {}.", d.active_days, plural(d.active_days as u64, "день", "дня", "дней"), total_days),
            ));
        }
        let avail = ui.available_width();
        let gap = 12.0;
        let cw = (avail - gap * (cards.len() as f32 - 1.0)) / cards.len() as f32;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (label, value, sub) in cards.iter() {
                card().show(ui, |ui| ui.vertical(|ui| {
                    ui.set_width(cw - 34.0);
                    ui.set_height(64.0);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.label(egui::RichText::new(*label).color(MUTED).size(12.5));
                    let mut val = value.clone();
                    if val.chars().count() > 22 {
                        val = val.chars().take(21).collect::<String>() + "…";
                    }
                    ui.label(egui::RichText::new(val).font(bold(21.0)).color(TEXT));
                    ui.label(egui::RichText::new(sub).color(MUTED).size(12.0));
                }));
            }
        });
        ui.add_space(12.0);

        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let caption = if self.period == Period::Day { "Активность по часам" } else { "Активность по дням" };
            ui.label(egui::RichText::new(caption).font(bold(14.0)).color(TEXT));
            ui.add_space(6.0);
            if self.period == Period::Day {
                let bars: Vec<(String, u64, String)> = self
                    .data
                    .hours
                    .iter()
                    .enumerate()
                    .map(|(h, s)| (if h % 3 == 0 { format!("{h:02}") } else { String::new() }, *s, if *s > 0 {
                            format!("С {h:02}:00 до {:02}:00 вы были активны {}.", (h + 1) % 24, fmt_dur(*s))
                        } else {
                            format!("С {h:02}:00 до {:02}:00 активности не было.", (h + 1) % 24)
                        }))
                    .collect();
                bar_chart(ui, &bars, 3600);
            } else {
                let n = self.data.days.len();
                let step = (n / 10).max(1);
                let bars: Vec<(String, u64, String)> = self
                    .data
                    .days
                    .iter()
                    .enumerate()
                    .map(|(i, (d, s))| {
                        let lbl = if n <= 7 { WEEKDAYS[d.weekday() as usize].to_string() } else if (n - 1 - i).is_multiple_of(step) { format!("{}.{:02}", d.d, d.m) } else { String::new() };
                        (lbl, *s, if *s > 0 {
                            format!("{}, {}, вы были активны {}.", WEEKDAYS_ON[d.weekday() as usize], date_full(*d), fmt_dur(*s))
                        } else {
                            format!("{}, {}, активности не было.", WEEKDAYS_ON[d.weekday() as usize], date_full(*d))
                        })
                    })
                    .collect();
                bar_chart(ui, &bars, 0);
            }
        });
        ui.add_space(12.0);

        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Приложения").font(bold(14.0)).color(TEXT));
            ui.add_space(4.0);
            if self.data.apps.is_empty() {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("За этот период трекер ещё не записал ни одной минуты активности.").color(MUTED));
                    if !self.tracker_alive {
                        ui.label(egui::RichText::new("Фоновый трекер сейчас не запущен, поэтому время не записывается. Нажмите «Запустить трекер» слева, чтобы начать отслеживание.").color(MUTED).size(12.0));
                    }
                });
                ui.add_space(30.0);
                return;
            }
            let apps = self.data.apps.clone();
            let total = self.data.total.max(1);
            let max = apps.first().map(|a| a.1).unwrap_or(1).max(1);
            let mut exclude: Option<String> = None;
            for (i, (exe, secs)) in apps.iter().enumerate() {
                let name = self.name_of(exe);
                let icon = self.icon_of(ui.ctx(), exe);
                let color = PALETTE[i % PALETTE.len()];
                let resp = app_row(ui, &name, exe, *secs, *secs as f32 / max as f32, *secs as f32 / total as f32, color, icon.as_ref());
                resp.context_menu(|ui| {
                    if ui.button("Не учитывать это приложение").clicked() {
                        exclude = Some(exe.clone());
                        ui.close();
                    }
                    if let Some(p) = self.paths.get(exe)
                        && ui.button("Открыть папку приложения").clicked() {
                            let dir = p.rsplit_once('\\').map(|x| x.0).unwrap_or(p).to_string();
                            winutil::open_path(&dir);
                            ui.close();
                        }
                });
            }
            if let Some(e) = exclude
                && !self.cfg.is_excluded(&e) {
                    self.cfg.excluded.push(e);
                    self.save_cfg();
                    self.reload();
                }
        });
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Настройки").font(bold(24.0)).color(TEXT));
        ui.add_space(10.0);

        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Общие").font(bold(14.0)).color(TEXT));
            ui.add_space(6.0);
            if toggle_row(ui, "Запускать вместе с Windows", "Когда вы входите в Windows, трекер автоматически запускается в фоне.", &mut self.autostart) {
                autostart::set(self.autostart);
            }
            ui.separator();
            let mut paused = self.cfg.paused;
            if toggle_row(ui, "Поставить отслеживание на паузу", "Пока пауза включена, трекер не записывает время в приложениях.", &mut paused) {
                self.cfg.paused = paused;
                self.save_cfg();
            }
            ui.separator();
            ui.label("Через сколько минут без действий вас нужно считать неактивным");
            ui.label(
                egui::RichText::new(
                    "Если вы не пользуетесь мышью и клавиатурой дольше указанного времени, трекер перестаёт считать время. \
                     Когда на экране открыто полноэкранное окно, например игра или видео, время считается даже без действий.",
                )
                .color(MUTED)
                .size(12.0),
            );
            ui.horizontal(|ui| {
                {
                    let mut mins = (self.cfg.idle_secs / 60) as u32;
                    let r = ui.add(egui::Slider::new(&mut mins, 1..=30).suffix(" мин"));
                    if r.changed() {
                        self.cfg.idle_secs = mins as u64 * 60;
                    }
                    if r.drag_stopped() || (r.changed() && !r.dragged()) {
                        self.save_cfg();
                    }
                }
            });
        });
        ui.add_space(12.0);

        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Исключённые приложения").font(bold(14.0)).color(TEXT));
            ui.label(egui::RichText::new("Системные процессы Windows не учитываются по умолчанию, потому что они постоянно работают в фоне. \
                 Чтобы исключить другое приложение, щёлкните по нему правой кнопкой мыши в статистике или введите имя его файла ниже.").color(MUTED).size(12.0));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut self.new_exclude).hint_text("Введите имя файла, например steam.exe").desired_width(330.0));
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Добавить в исключения").clicked() || enter {
                    let mut e = self.new_exclude.trim().to_lowercase();
                    if !e.is_empty() {
                        if !e.ends_with(".exe") {
                            e.push_str(".exe");
                        }
                        if !self.cfg.is_excluded(&e) {
                            self.cfg.excluded.push(e);
                            self.save_cfg();
                            self.reload();
                        }
                    }
                    self.new_exclude.clear();
                }
                if ui.button("Вернуть стандартный список").clicked() {
                    self.cfg.excluded = DEFAULT_EXCLUDED.iter().map(|s| s.to_string()).collect();
                    self.save_cfg();
                    self.reload();
                }
            });
            ui.add_space(6.0);
            let mut remove = None;
            egui::ScrollArea::vertical().max_height(220.0).id_salt("excl").show(ui, |ui| {
                let user: Vec<String> = self.cfg.excluded.iter().filter(|e| !DEFAULT_EXCLUDED.contains(&e.as_str())).cloned().collect();
                let sys: Vec<String> = self.cfg.excluded.iter().filter(|e| DEFAULT_EXCLUDED.contains(&e.as_str())).cloned().collect();
                for e in user.iter().chain(sys.iter()) {
                    ui.horizontal(|ui| {
                        let is_sys = DEFAULT_EXCLUDED.contains(&e.as_str());
                        ui.label(egui::RichText::new(e).color(if is_sys { MUTED } else { TEXT }));
                        if is_sys {
                            ui.label(egui::RichText::new("системный процесс").color(MUTED).size(11.0));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Снова учитывать").on_hover_text("Нажмите эту кнопку, чтобы трекер снова считал время в этом приложении.").clicked() {
                                remove = Some(e.clone());
                            }
                        });
                    });
                }
            });
            if let Some(e) = remove {
                self.cfg.excluded.retain(|x| *x != e);
                self.save_cfg();
                self.reload();
            }
        });
        ui.add_space(12.0);

        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Данные").font(bold(14.0)).color(TEXT));
            ui.label(egui::RichText::new("Трекер хранит статистику в этой папке в виде текстовых файлов, по одному файлу на каждый день.").color(MUTED).size(12.0));
            ui.label(egui::RichText::new(data_dir().display().to_string()).color(MUTED).size(12.0));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Открыть папку с данными").clicked() {
                    winutil::open_path(&data_dir().display().to_string());
                }
                if self.tracker_alive {
                    if ui.button("Остановить фоновый трекер").clicked() {
                        winutil::flush_tracker();
                        winutil::stop_tracker();
                        std::thread::sleep(Duration::from_millis(200));
                        self.tracker_alive = winutil::tracker_running();
                    }
                } else if ui.button("Запустить фоновый трекер").clicked() {
                    winutil::start_tracker();
                    std::thread::sleep(Duration::from_millis(300));
                    self.tracker_alive = winutil::tracker_running();
                }
            });
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.last_reload.elapsed() >= Duration::from_secs(10) {
            self.reload();
        }
        ui.ctx().request_repaint_after(Duration::from_secs(10));

        egui::Panel::left("side")
            .exact_size(210.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(SIDEBAR))
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin { left: 24, right: 24, top: 20, bottom: 12 }))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.view {
                    View::Stats => self.stats(ui),
                    View::Settings => self.settings(ui),
                });
            });
    }

    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }
}

fn nav_button(ui: &mut egui::Ui, glyph: u8, label: &str, selected: bool) -> bool {
    let w = ui.available_width() - 24.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
    let r = Rect::from_min_size(rect.min + Vec2::new(12.0, 2.0), Vec2::new(w, 36.0));
    let p = ui.painter();
    if selected {
        p.rect_filled(r, CornerRadius::same(10), SURFACE_HI);
        p.rect_filled(Rect::from_min_size(r.min + Vec2::new(0.0, 9.0), Vec2::new(3.0, 18.0)), CornerRadius::same(2), ACCENT);
    } else if resp.hovered() {
        p.rect_filled(r, CornerRadius::same(10), SURFACE);
    }
    let c = if selected { TEXT } else { MUTED };
    let ic = if selected { ACCENT2 } else { c };
    let o = r.left_center() + Vec2::new(22.0, 0.0);
    if glyph == 0 {
        for (i, hgt) in [7.0f32, 12.0, 9.0].iter().enumerate() {
            let x = o.x - 6.0 + i as f32 * 5.0;
            p.rect_filled(Rect::from_min_max(Pos2::new(x, o.y + 6.0 - hgt), Pos2::new(x + 3.0, o.y + 6.0)), CornerRadius::same(1), ic);
        }
    } else {
        p.circle_stroke(o, 5.5, Stroke::new(2.0, ic));
        p.circle_filled(o, 1.8, ic);
    }
    p.text(r.left_center() + Vec2::new(40.0, 0.0), Align2::LEFT_CENTER, label, if selected { bold(14.0) } else { FontId::proportional(14.0) }, c);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn segment(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let galley = ui.painter().layout_no_wrap(label.to_string(), if selected { bold(13.0) } else { FontId::proportional(13.0) }, TEXT);
    let size = Vec2::new(galley.size().x + 24.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(8), ACCENT);
    } else {
        p.rect_filled(rect, CornerRadius::same(8), if resp.hovered() { SURFACE_HI } else { SURFACE });
        p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
    }
    p.text(rect.center(), Align2::CENTER_CENTER, label, if selected { bold(13.0) } else { FontId::proportional(13.0) }, if selected { Color32::WHITE } else { MUTED });
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn toggle_row(ui: &mut egui::Ui, title: &str, sub: &str, on: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(title);
            ui.label(egui::RichText::new(sub).color(MUTED).size(12.0));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(44.0, 24.0), Sense::click());
            if resp.clicked() {
                *on = !*on;
                changed = true;
            }
            let t = ui.ctx().animate_bool(resp.id, *on);
            let p = ui.painter();
            let bg = lerp_color(Color32::from_rgb(55, 62, 80), ACCENT, t);
            p.rect_filled(rect, CornerRadius::same(12), bg);
            let x = egui::lerp(rect.left() + 12.0..=rect.right() - 12.0, t);
            p.circle_filled(Pos2::new(x, rect.center().y), 9.0, Color32::WHITE);
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });
    });
    changed
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn bar_chart(ui: &mut egui::Ui, bars: &[(String, u64, String)], fixed_max: u64) {
    let h = 150.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h + 20.0), Sense::hover());
    let plot = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + h));
    let max = if fixed_max > 0 { fixed_max } else { bars.iter().map(|b| b.1).max().unwrap_or(0).max(60) } as f32;
    let p = ui.painter().clone();
    for f in [0.25, 0.5, 0.75, 1.0] {
        let y = plot.bottom() - plot.height() * f;
        p.line_segment([Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)], Stroke::new(1.0, Color32::from_rgb(30, 35, 47)));
    }
    let n = bars.len().max(1) as f32;
    let slot = plot.width() / n;
    let bw = (slot * 0.62).clamp(2.0, 46.0);
    for (i, (label, v, tip)) in bars.iter().enumerate() {
        let cx = plot.left() + slot * (i as f32 + 0.5);
        let col = Rect::from_center_size(Pos2::new(cx, plot.center().y), Vec2::new(slot, plot.height()));
        let resp = ui.interact(col, ui.id().with(("bar", i)), Sense::hover());
        let hovered = resp.hovered();
        let bh = ((*v as f32 / max).min(1.0) * plot.height()).max(if *v > 0 { 3.0 } else { 0.0 });
        let r = Rect::from_min_max(Pos2::new(cx - bw / 2.0, plot.bottom() - bh), Pos2::new(cx + bw / 2.0, plot.bottom()));
        if hovered {
            p.rect_filled(Rect::from_min_max(Pos2::new(cx - slot / 2.0 + 1.0, plot.top()), Pos2::new(cx + slot / 2.0 - 1.0, plot.bottom())), CornerRadius::same(6), Color32::from_rgba_unmultiplied(255, 255, 255, 6));
        }
        if bh > 0.0 {
            let rad = (bw / 2.0).min(6.0) as u8;
            let top_c = if hovered { ACCENT2 } else { ACCENT };
            let bot_c = lerp_color(ACCENT, ACCENT2, 0.55);
            gradient_rect(&p, r, top_c, bot_c, rad);
        } else {
            p.rect_filled(Rect::from_min_max(Pos2::new(cx - bw / 2.0, plot.bottom() - 2.0), Pos2::new(cx + bw / 2.0, plot.bottom())), CornerRadius::same(1), Color32::from_rgb(34, 39, 52));
        }
        if !label.is_empty() {
            p.text(Pos2::new(cx, plot.bottom() + 11.0), Align2::CENTER_CENTER, label, FontId::proportional(11.0), MUTED);
        }
        resp.on_hover_text_at_pointer(tip);
    }
}

fn gradient_rect(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32, radius: u8) {
    let cap = (radius as f32).min(r.height());
    let cap_rect = Rect::from_min_max(r.min, Pos2::new(r.right(), r.top() + cap * 2.0).min(r.max));
    p.rect_filled(cap_rect, CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 }, top);
    let body = Rect::from_min_max(Pos2::new(r.left(), r.top() + cap), r.max);
    if body.height() > 0.0 {
        let t = (cap / r.height()).clamp(0.0, 1.0);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(body.left_top(), lerp_color(top, bottom, t));
        mesh.colored_vertex(body.right_top(), lerp_color(top, bottom, t));
        mesh.colored_vertex(body.right_bottom(), bottom);
        mesh.colored_vertex(body.left_bottom(), bottom);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(egui::Shape::mesh(mesh));
    }
}

#[allow(clippy::too_many_arguments)]
fn app_row(ui: &mut egui::Ui, name: &str, exe: &str, secs: u64, rel: f32, share: f32, color: Color32, icon: Option<&egui::TextureHandle>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(10), SURFACE_HI);
    }
    let icon_rect = Rect::from_center_size(rect.left_center() + Vec2::new(24.0, 0.0), Vec2::splat(28.0));
    match icon {
        Some(t) => egui::Image::new(t).paint_at(ui, icon_rect),
        None => {
            p.rect_filled(icon_rect, CornerRadius::same(8), lerp_color(color, SURFACE, 0.6));
            let letter = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
            p.text(icon_rect.center(), Align2::CENTER_CENTER, letter, bold(14.0), Color32::WHITE);
        }
    }
    let right_w = 150.0;
    let x0 = rect.left() + 50.0;
    let bar_right = rect.right() - right_w - 12.0;
    let mut nm: String = name.to_string();
    if nm.chars().count() > 48 {
        nm = nm.chars().take(47).collect::<String>() + "…";
    }
    let g = p.layout_no_wrap(nm, bold(14.0), TEXT);
    let name_w = g.size().x;
    p.galley(Pos2::new(x0, rect.top() + 8.0), g, TEXT);
    if !name.eq_ignore_ascii_case(exe.trim_end_matches(".exe")) {
        p.text(Pos2::new(x0 + name_w + 8.0, rect.top() + 17.0), Align2::LEFT_CENTER, exe, FontId::proportional(11.5), MUTED);
    }
    let track = Rect::from_min_max(Pos2::new(x0, rect.top() + 33.0), Pos2::new(bar_right.max(x0 + 40.0), rect.top() + 39.0));
    p.rect_filled(track, CornerRadius::same(3), Color32::from_rgb(34, 39, 52));
    let fill = Rect::from_min_size(track.min, Vec2::new((track.width() * rel).max(4.0), track.height()));
    p.rect_filled(fill, CornerRadius::same(3), color);
    p.text(Pos2::new(rect.right() - 12.0, rect.top() + 17.0), Align2::RIGHT_CENTER, fmt_dur(secs), bold(14.0), TEXT);
    p.text(Pos2::new(rect.right() - 12.0, rect.top() + 36.0), Align2::RIGHT_CENTER, format!("{:.1}%", share * 100.0), FontId::proportional(12.0), MUTED);
    resp
}
