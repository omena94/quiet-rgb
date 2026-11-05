#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;
mod effect;
mod msi;
mod openrgb;
mod worker;

use effect::{Effect, Settings};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use openrgb::Device;
use std::time::{Duration, Instant};
use worker::{Event, Worker};

const ACCENT: Color32 = Color32::from_rgb(112, 221, 206);
const MUTED: Color32 = Color32::from_rgb(150, 162, 181);

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 850.0])
            .with_min_inner_size([940.0, 760.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Quiet RGB · 主機燈光",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

struct App {
    settings: Settings,
    worker: Worker,
    devices: Vec<Device>,
    selected: Vec<u32>,
    connected: bool,
    connecting: bool,
    active: bool,
    status: String,
    error: bool,
    port: u16,
    started: Instant,
    hex: String,
    help: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_style(&cc.egui_ctx);
        let mut settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, "lighting"))
            .unwrap_or_default();
        settings.sanitize();
        let hex = hex_color(settings.color);
        let mut app = Self {
            settings,
            worker: Worker::new(),
            devices: vec![],
            selected: vec![],
            connected: false,
            connecting: false,
            active: false,
            status: "預覽模式 · 尚未控制硬體".into(),
            error: false,
            port: 6742,
            started: Instant::now(),
            hex,
            help: false,
        };
        app.connect(true);
        #[cfg(feature = "screenshot")]
        if std::env::var_os("EFRAME_SCREENSHOT_TO").is_some() {
            let deadline = Instant::now() + Duration::from_secs(4);
            while app.connecting && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(25));
                app.events();
            }
        }
        app
    }

    fn connect(&mut self, yes: bool) {
        self.active = false;
        self.connected = false;
        self.connecting = yes;
        self.selected.clear();
        self.devices.clear();
        self.error = false;
        let mut d = self.worker.desired.lock().unwrap();
        d.connection += 1;
        d.connected = yes;
        d.port = self.port;
        d.active = false;
    }

    fn sync(&self) {
        let mut d = self.worker.desired.lock().unwrap();
        d.settings = self.settings.clone();
        d.selected = self.selected.clone();
        d.active = self.active && self.connected;
    }

    fn events(&mut self) {
        while let Ok(event) = self.worker.events.try_recv() {
            match event {
                Event::Connected(devices) => {
                    // Default to case components, leaving keyboards/mice untouched.
                    self.selected = devices
                        .iter()
                        .filter(|d| d.controllable && matches!(d.kind, 0..=3))
                        .map(|d| d.id)
                        .collect();
                    self.devices = devices;
                    self.connected = true;
                    self.connecting = false;
                    self.error = false;
                    self.status = format!(
                        "已連線 · 找到 {} 個裝置 · 按「套用到主機」開始",
                        self.devices.len()
                    );
                }
                Event::Status(s) => {
                    self.status = s;
                }
                Event::Error(s) => {
                    self.status = s;
                    self.error = true;
                    self.active = false;
                    self.connected = false;
                    self.connecting = false;
                }
            }
        }
    }

    fn device_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.label(RichText::new("裝置").size(20.0).strong());
        ui.label(RichText::new("選取要一起調整的燈光").color(MUTED));
        ui.add_space(16.0);
        if self.devices.is_empty() {
            card().show(ui, |ui| {
                ui.set_min_width(190.0);
                ui.label(
                    RichText::new(if self.connecting {
                        "正在尋找裝置…"
                    } else {
                        "等待燈控服務"
                    })
                    .strong(),
                );
                ui.add_space(8.0);
                ui.label("自動偵測 MSI 主機板 USB。其他品牌裝置可透過 OpenRGB 擴充。");
                ui.add_space(8.0);
                if ui.button("查看連線方式").clicked() {
                    self.help = true;
                }
            });
        }
        for device in &self.devices {
            card().show(ui, |ui| {
                ui.set_min_width(190.0);
                let mut checked = self.selected.contains(&device.id);
                if ui
                    .add_enabled(
                        device.controllable,
                        egui::Checkbox::new(&mut checked, RichText::new(&device.name).strong()),
                    )
                    .changed()
                {
                    if checked {
                        self.selected.push(device.id);
                    } else {
                        self.selected.retain(|id| *id != device.id);
                    }
                }
                let detail = if device.id == msi::DEVICE_ID {
                    "4 個接頭 · 每接頭最多 240 LEDs".into()
                } else {
                    format!("{} · {} LEDs", device.vendor, device.leds)
                };
                ui.label(RichText::new(detail).small().color(MUTED));
                if !device.controllable {
                    ui.label(
                        RichText::new("此裝置未提供可用的直接色彩模式")
                            .small()
                            .color(Color32::LIGHT_YELLOW),
                    );
                }
                ui.label(
                    RichText::new(format!("模式：{}", device.modes.join(" / ")))
                        .small()
                        .color(MUTED),
                );
            });
            ui.add_space(8.0);
        }
        ui.add_space(18.0);
        ui.label(
            RichText::new("裝置連線 / OPENRGB 擴充")
                .small()
                .color(MUTED),
        );
        ui.horizontal(|ui| {
            ui.label("127.0.0.1 :");
            ui.add_enabled(
                !self.connected && !self.connecting,
                egui::DragValue::new(&mut self.port).range(1024..=65535),
            );
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.connecting, egui::Button::new("重新連線"))
                .clicked()
            {
                self.connect(true);
            }
            if self.connected && ui.button("中斷").clicked() {
                self.connect(false);
            }
        });
        ui.add_space(12.0);
        ui.label(
            RichText::new("這張 MSI 主機板可直接控制，\n不需要 OpenRGB 或原廠軟體。")
                .small()
                .color(MUTED),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("讓燈光，剛剛好。").size(28.0).strong());
                ui.label(RichText::new("一點色彩，少一點打擾。").color(MUTED));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let text = if self.active {
                    "●  正在控制主機"
                } else {
                    "○  燈效預覽"
                };
                ui.label(RichText::new(text).color(ACCENT));
            });
        });
        ui.add_space(10.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            preview(ui, &self.settings, self.started.elapsed().as_secs_f32());
        });
        ui.add_space(8.0);
        ui.label(RichText::new("燈光效果").size(17.0).strong());
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for effect in Effect::ALL {
                if ui
                    .add_sized(
                        [105.0, 36.0],
                        egui::Button::new(effect.label()).selected(self.settings.effect == effect),
                    )
                    .clicked()
                {
                    self.settings.effect = effect;
                }
            }
        });
        ui.add_space(14.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("亮度").strong());
                ui.label(
                    RichText::new(format!("{:.0}%", self.settings.brightness * 100.0))
                        .color(ACCENT)
                        .size(20.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("夜間 5%").clicked() {
                        self.settings.brightness = 0.05;
                    }
                    if ui.small_button("柔和 15%").clicked() {
                        self.settings.brightness = 0.15;
                    }
                });
            });
            ui.style_mut().spacing.slider_width = (ui.available_width() - 25.0).max(100.0);
            ui.add(egui::Slider::new(&mut self.settings.brightness, 0.0..=1.0).show_value(false));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("色彩");
                ui.add_enabled_ui(
                    matches!(self.settings.effect, Effect::Solid | Effect::Breathe),
                    |ui| {
                        if ui
                            .color_edit_button_srgb(&mut self.settings.color)
                            .changed()
                        {
                            self.hex = hex_color(self.settings.color);
                        }
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.hex)
                                .desired_width(85.0)
                                .char_limit(7),
                        );
                        if response.changed()
                            && let Some(color) = parse_hex(&self.hex)
                        {
                            self.settings.color = color;
                        }
                        if parse_hex(&self.hex).is_none() {
                            ui.colored_label(Color32::LIGHT_YELLOW, "6 碼色碼");
                        }
                        for color in [
                            [110, 175, 255],
                            [119, 230, 199],
                            [202, 148, 255],
                            [255, 149, 112],
                            [255, 218, 155],
                            [255, 255, 255],
                        ] {
                            let c = Color32::from_rgb(color[0], color[1], color[2]);
                            if ui
                                .add(egui::Button::new(" ").fill(c).min_size(Vec2::splat(23.0)))
                                .clicked()
                            {
                                self.settings.color = color;
                                self.hex = hex_color(color);
                            }
                        }
                    },
                );
            });
            ui.add_space(8.0);
            ui.add_enabled_ui(self.settings.effect.animated(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("週期");
                    ui.style_mut().spacing.slider_width = 230.0;
                    ui.add(
                        egui::Slider::new(&mut self.settings.period, 0.5..=15.0)
                            .suffix(" 秒")
                            .fixed_decimals(1),
                    );
                    ui.label(RichText::new("數字越大，變化越慢").small().color(MUTED));
                });
            });
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            let ready = self.connected && !self.selected.is_empty();
            if ui
                .add_enabled(
                    ready,
                    egui::Button::new(
                        RichText::new(if self.active {
                            "✓ 即時套用中"
                        } else {
                            "套用到主機"
                        })
                        .color(Color32::from_rgb(15, 35, 34)),
                    )
                    .fill(ACCENT)
                    .min_size([155.0, 40.0].into()),
                )
                .clicked()
            {
                self.active = true;
            }
            if ui
                .add_enabled(
                    ready,
                    egui::Button::new("一鍵熄燈").min_size([105.0, 40.0].into()),
                )
                .clicked()
            {
                self.settings.effect = Effect::Off;
                self.active = true;
            }
            if ui
                .add_enabled(
                    self.active,
                    egui::Button::new("停止並熄燈").min_size([115.0, 40.0].into()),
                )
                .clicked()
            {
                self.active = false;
            }
        });
        ui.add_space(10.0);
        ui.checkbox(
            &mut self.settings.off_on_exit,
            "關閉程式時，熄滅正在控制的燈光",
        );
        ui.label(
            RichText::new("取消勾選則保留最後顏色。呼吸與彩虹需要保持程式執行。")
                .small()
                .color(MUTED),
        );
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.events();
        let ctx = ui.ctx().clone();
        egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(17, 22, 31))
                    .inner_margin(18.0),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (icon, _) = ui.allocate_exact_size(Vec2::splat(25.0), egui::Sense::hover());
                    ui.painter()
                        .circle_stroke(icon.center(), 10.0, Stroke::new(2.0, ACCENT));
                    ui.painter().circle_filled(
                        icon.center(),
                        4.0,
                        Color32::from_rgb(154, 155, 255),
                    );
                    ui.label(RichText::new("QUIET RGB").size(21.0).strong());
                    ui.label(RichText::new("  /  主機燈光控制").color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("使用說明").clicked() {
                            self.help = true;
                        }
                    });
                });
            });
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .inner_margin(12.0)
                    .fill(Color32::from_rgb(17, 22, 31)),
            )
            .show(ui, |ui| {
                ui.label(RichText::new(&self.status).color(if self.error {
                    Color32::from_rgb(246, 185, 119)
                } else {
                    MUTED
                }));
            });
        egui::Panel::left("devices")
            .exact_size(265.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .inner_margin(18.0)
                    .fill(Color32::from_rgb(20, 25, 35)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.device_panel(ui));
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .inner_margin(24.0)
                    .fill(Color32::from_rgb(13, 17, 25)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.controls(ui));
            });
        egui::Window::new("開始使用 Quiet RGB").open(&mut self.help).resizable(false).default_width(530.0).show(&ctx, |ui| {
            ui.label("1. 執行 Quiet RGB，即可直接偵測這台 MSI MS-7E51 主機板。");
            ui.label("2. 選取裝置，調整顏色／亮度／效果，再按「套用到主機」。");
            ui.label("3. 其他裝置可選擇啟動 OpenRGB SDK Server（127.0.0.1:6742）後重新連線。");
            ui.add_space(10.0);
            ui.label("若只有部分燈具出現：確認風扇／燈條接到主機板 ARGB 接頭，或控制器已切到主機板同步。記憶體等裝置可能需要 OpenRGB 的 PawnIO 驅動與管理員權限。");
            ui.label("原生模式控制 JAF 與三個 JARGB 接頭。風扇轉速不受影響；請避免同時用其他軟體控制同一張主機板。");
            ui.hyperlink_to("OpenRGB 官方下載與支援", "https://openrgb.org/releases.html");
        });
        self.sync();
        ctx.request_repaint_after(Duration::from_millis(if self.settings.effect.animated() {
            33
        } else {
            150
        }));
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "lighting", &self.settings);
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.sync();
        self.worker.shutdown();
    }
}

fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgb(25, 32, 44))
        .stroke(Stroke::new(1.0, Color32::from_rgb(43, 52, 68)))
        .corner_radius(12)
        .inner_margin(14.0)
}

fn setup_style(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/msjh.ttc") {
        fonts
            .font_data
            .insert("chinese".into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("chinese".into());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("chinese".into());
    }
    ctx.set_fonts(fonts);
    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = [10.0, 9.0].into();
        style.spacing.button_padding = [12.0, 8.0].into();
        style.visuals.selection.bg_fill = Color32::from_rgb(39, 93, 87);
        style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    });
}

fn preview(ui: &mut egui::Ui, settings: &Settings, seconds: f32) {
    let (rect, _) =
        ui.allocate_exact_size([ui.available_width(), 100.0].into(), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let colors = settings.colors(36, seconds);
    let center = rect.center();
    for fan in 0..3 {
        let x = center.x + (fan as f32 - 1.0) * 122.0;
        let origin = egui::pos2(x, center.y - 8.0);
        painter.circle_filled(origin, 35.0, Color32::from_rgb(13, 17, 25));
        painter.circle_stroke(
            origin,
            35.0,
            Stroke::new(1.0, Color32::from_rgb(48, 57, 72)),
        );
        for led in 0..12 {
            let angle = led as f32 / 12.0 * std::f32::consts::TAU;
            let point = origin + Vec2::angled(angle) * 27.0;
            let c = colors[fan * 12 + led];
            let color = Color32::from_rgb(c[0], c[1], c[2]);
            painter.circle_filled(point, 7.0, color.gamma_multiply(0.25));
            painter.circle_filled(point, 4.0, color);
        }
        painter.circle_filled(origin, 12.0, Color32::from_rgb(34, 43, 57));
        painter.circle_stroke(
            origin,
            15.0,
            Stroke::new(1.0, Color32::from_rgb(57, 67, 84)),
        );
    }
    painter.text(
        egui::pos2(center.x, rect.bottom() - 5.0),
        egui::Align2::CENTER_BOTTOM,
        "燈效示意 · 實際效果依裝置與接線而定",
        egui::FontId::proportional(12.0),
        MUTED,
    );
}

fn hex_color(color: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}
fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 || !s.is_ascii() {
        return None;
    }
    Some([
        u8::from_str_radix(&s[0..2], 16).ok()?,
        u8::from_str_radix(&s[2..4], 16).ok()?,
        u8::from_str_radix(&s[4..6], 16).ok()?,
    ])
}
