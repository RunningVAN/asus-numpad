//! 主界面。
//!
//! 界面只做三件事：选接口、调档位、看报文。所有档位映射都走
//! [`asus_numpad::brightness`] 与 [`asus_numpad::led_proto`]——和 Linux 侧同一份函数，
//! 避免两边灯效暗中不一致。

use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;

use asus_numpad::led_policy::{RANK_WARN_ASUS_FALLBACK, RANK_WARN_NO_FEATURE};
use asus_numpad::led_proto;

use crate::backend::Backend;
use crate::settings::{self, Settings};
use crate::{autostart, backend, ui_font};

const MAX_LOG: usize = 200;

struct LogEntry {
    at: String,
    frame: String,
    note: String,
    ok: bool,
}

/// 界面里点出来的动作，先收集、等界面画完再执行，避免面板闭包和状态互相借。
enum Action {
    Rescan,
    SendOff,
    SendCurrent,
    SaveSettings,
    ResetSettings,
    SetAutostart(bool),
}

pub struct App {
    backend: Box<dyn Backend>,
    devices: Vec<backend::DeviceInfo>,
    selected: Option<usize>,
    settings: Settings,
    log: VecDeque<LogEntry>,
    status: String,
    status_bad: bool,
    last_poll: Instant,
    last_numlock: Option<bool>,
    sent_numlock: Option<bool>,
    autostart_on: bool,
    font_note: Option<String>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let font_note = ui_font::install(&cc.egui_ctx);
        let settings = settings::load();
        let mut app = Self {
            backend: backend::create(),
            devices: Vec::new(),
            selected: None,
            settings,
            log: VecDeque::new(),
            status: String::new(),
            status_bad: false,
            last_poll: Instant::now(),
            last_numlock: None,
            sent_numlock: None,
            autostart_on: autostart::enabled(),
            font_note,
        };
        app.rescan();
        if app.settings.apply_on_start && app.settings.on {
            app.apply_current();
        }
        app
    }

    fn set_status(&mut self, text: String, bad: bool) {
        self.status = text;
        self.status_bad = bad;
    }

    fn rescan(&mut self) {
        match self.backend.scan() {
            Ok(devices) => {
                self.devices = devices;
                let wanted = self.settings.device_path.clone();
                let by_path = wanted
                    .as_deref()
                    .and_then(|p| self.devices.iter().position(|d| d.path == p));
                // 记住的接口必须还能点灯，否则退回自动挑选
                let by_path = by_path.filter(|&i| self.devices[i].usable());
                self.selected = by_path.or_else(|| self.devices.iter().position(|d| d.selected));
                let msg = match self.selected {
                    Some(i) => {
                        let d = &self.devices[i];
                        self.settings.device_path = Some(d.path.clone());
                        format!("已选中 {}（{}）", d.vid_pid(), d.vendor())
                    }
                    None => "未找到可点灯的接口".to_string(),
                };
                let bad = self.selected.is_none();
                self.set_status(msg, bad);
            }
            Err(err) => {
                self.devices.clear();
                self.selected = None;
                self.set_status(format!("枚举 HID 接口失败：{err:#}"), true);
            }
        }
    }

    /// 当前档位组：`0x00`/`0x41..=0x48` 上等分取 `steps` 个点，与 Linux 侧同一函数。
    fn level_table(&self) -> Vec<u8> {
        backend::level_table(self.settings.steps)
    }

    fn level_for_step(&self, step: u8) -> Option<u8> {
        backend::level_for_step(self.settings.steps, step)
    }

    fn push_log(&mut self, frame: String, note: String, ok: bool) {
        self.log.push_front(LogEntry {
            at: now_hms(),
            frame,
            note,
            ok,
        });
        self.log.truncate(MAX_LOG);
    }

    fn send(&mut self, value: u8, note: &str) {
        let Some(idx) = self.selected else {
            self.set_status("没有可用的灯光接口".to_string(), true);
            return;
        };
        let device = self.devices[idx].clone();
        if !device.usable() {
            self.set_status(
                format!("{} 不带 FEATURE 0x0d，无法点灯", device.vid_pid()),
                true,
            );
            return;
        }
        let Some(frame) = led_proto::encode(value) else {
            return;
        };
        let result = self.backend.write(&device, frame);
        let hex = led_proto::hex(&frame);
        match result {
            Ok(()) => {
                self.push_log(hex.clone(), note.to_string(), true);
                self.set_status(format!("已写入 {hex}（{note}）"), false);
            }
            Err(err) => {
                self.push_log(hex.clone(), format!("{note}·失败"), false);
                self.set_status(format!("写入 {hex} 失败：{err:#}"), true);
            }
        }
    }

    fn apply_current(&mut self) {
        if !self.settings.on {
            self.send(led_proto::LED_OFF, "熄灭");
            return;
        }
        match self.level_for_step(self.settings.step) {
            Some(0) => self.send(led_proto::LED_OFF, "档位 1＝熄灭"),
            Some(level) => {
                let note = format!("亮度 {} 档", self.settings.step);
                self.send(level, &note)
            }
            None => self.set_status("档位越界，未下发".to_string(), true),
        }
    }

    fn poll_numlock(&mut self) {
        if !self.settings.follow_numlock {
            return;
        }
        if self.last_poll.elapsed() < Duration::from_millis(self.settings.poll_ms) {
            return;
        }
        self.last_poll = Instant::now();
        let Some(on) = self.backend.numlock() else {
            return;
        };
        self.last_numlock = Some(on);
        if self.sent_numlock == Some(on) {
            return;
        }
        self.sent_numlock = Some(on);
        if on {
            if self.settings.on {
                self.apply_current();
            }
        } else {
            self.send(led_proto::LED_OFF, "NumLock 关闭，熄灯");
        }
    }

    fn numlock_text(&self) -> String {
        match self.last_numlock {
            Some(true) => "NumLock：开".to_string(),
            Some(false) => "NumLock：关".to_string(),
            None => "NumLock：未知".to_string(),
        }
    }

    fn draw_header(&mut self, root: &mut egui::Ui, action: &mut Option<Action>) {
        egui::Panel::top("header").show(root, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("ASUS NumberPad 背光控制");
                ui.label(small("数字键与手势由 ASUS 官方驱动处理，本程序只写背光 FEATURE 报文"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("重新扫描").clicked() {
                        *action = Some(Action::Rescan);
                    }
                });
            });
            ui.add_space(6.0);
        });
    }

    fn draw_devices(&mut self, root: &mut egui::Ui) {
        let mut clicked: Option<usize> = None;
        egui::Panel::left("devices")
            .resizable(true)
            .default_size(340.0)
            .show(root, |ui| {
                ui.add_space(6.0);
                ui.heading("HID 接口");
                ui.label(small(format!(
                    "{} · 共 {} 个接口",
                    self.backend.name(),
                    self.devices.len()
                )));
                ui.separator();

                if !self.backend.is_real() {
                    ui.label(
                        egui::RichText::new("模拟后端：报文只进日志，不会碰硬件")
                            .small()
                            .color(egui::Color32::from_rgb(200, 150, 40)),
                    );
                    ui.separator();
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.devices.is_empty() {
                            ui.label("没有枚举到任何 HID 接口。");
                            ui.label(small("确认 ASUS 官方驱动已安装、设备未被其它程序独占。"));
                            return;
                        }
                        for (i, d) in self.devices.iter().enumerate() {
                            let name = if d.product.is_empty() {
                                d.tail().to_string()
                            } else {
                                d.product.clone()
                            };
                            let title = format!("{}  {}\n{}", d.vid_pid(), d.vendor(), name);
                            if ui
                                .selectable_label(self.selected == Some(i), title)
                                .clicked()
                            {
                                clicked = Some(i);
                            }
                            if d.usable() {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "FEATURE 0x0d 可用，报告长度 {}",
                                        d.feature_len
                                    ))
                                    .small(),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("无可用 FEATURE 0x0d").small().weak(),
                                );
                            }
                            match d.rank {
                                Some(RANK_WARN_NO_FEATURE) => {
                                    ui.label(
                                        egui::RichText::new("优先级：兜底（缺 FEATURE 报告）")
                                            .small()
                                            .color(egui::Color32::from_rgb(200, 150, 40)),
                                    );
                                }
                                Some(RANK_WARN_ASUS_FALLBACK) => {
                                    ui.label(
                                        egui::RichText::new("优先级：ASUS 兜底（OMNI RECEIVER）")
                                            .small()
                                            .weak(),
                                    );
                                }
                                _ => {}
                            }
                            ui.label(small(d.path.clone()));
                            ui.separator();
                        }
                    });
            });
        if let Some(i) = clicked {
            self.selected = Some(i);
            self.settings.device_path = Some(self.devices[i].path.clone());
            let label = format!(
                "已选中 {}（{}）",
                self.devices[i].vid_pid(),
                self.devices[i].vendor()
            );
            self.set_status(label, !self.devices[i].usable());
        }
    }

    fn draw_lighting(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.heading("灯光");
        ui.horizontal(|ui| {
            if ui.button("点亮（保持当前档）").clicked() {
                *action = Some(Action::SendCurrent);
            }
            if ui.button("熄灭").clicked() {
                *action = Some(Action::SendOff);
            }
        });

        let steps = self.settings.steps;
        let table = self.level_table();
        let current = self.settings.step;

        ui.add_space(4.0);
        let slider = ui.add(
            egui::Slider::new(&mut self.settings.step, 1..=steps)
                .text("亮度档")
                .integer(),
        );
        if slider.changed() {
            *action = Some(Action::SendCurrent);
        }

        ui.add_space(2.0);
        let mut picked: Option<u8> = None;
        ui.horizontal_wrapped(|ui| {
            for step in 1..=steps {
                let level = table.get((step - 1) as usize).copied();
                let label = match level {
                    Some(0) => format!("{step} · 熄灭"),
                    Some(v) => format!("{step} · 0x{v:02x}"),
                    None => format!("{step}"),
                };
                if ui.selectable_label(current == step, label).clicked() {
                    picked = Some(step);
                }
            }
        });
        if let Some(step) = picked {
            self.settings.step = step;
            *action = Some(Action::SendCurrent);
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("档位组");
            for n in [4u8, 8u8] {
                let label = if n == 4 { "4 档（含熄灭）" } else { "8 档（纯亮度）" };
                if ui.selectable_label(self.settings.steps == n, label).clicked() {
                    self.settings.steps = n;
                    if self.settings.step > n {
                        self.settings.step = n;
                    }
                }
            }
        });
        let mapped = self
            .level_table()
            .iter()
            .map(|v| format!("0x{v:02x}"))
            .collect::<Vec<_>>()
            .join("  ");
        ui.label(small(format!("当前档位组映射：{mapped}")));
        ui.label(small(
            "4 档组的第 1 档即熄灭，与官方四档键盘一致；映射与 Linux 侧共用 linear_levels_from_off。",
        ));
    }

    fn draw_follow(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading("联动");
        ui.checkbox(&mut self.settings.follow_numlock, "NumLock 关闭时自动熄灯");
        if self.settings.follow_numlock {
            ui.horizontal(|ui| {
                ui.label("轮询间隔");
                ui.add(
                    egui::Slider::new(&mut self.settings.poll_ms, 100..=2000)
                        .step_by(50.0)
                        .suffix(" ms"),
                );
            });
            ui.label(small(self.numlock_text()));
            if self.sent_numlock.is_none() {
                ui.label(small("等待第一次状态读取……"));
            }
        }
        if !self.backend.is_real() {
            let mut mock_on = self.backend.numlock().unwrap_or(true);
            if ui.checkbox(&mut mock_on, "模拟 NumLock 状态").changed() {
                self.backend.set_numlock(mock_on);
                self.last_numlock = Some(mock_on);
            }
        }
    }

    fn draw_log(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("报文日志");
            if ui.button("清空").clicked() {
                self.log.clear();
            }
        });
        if self.log.is_empty() {
            ui.label(small("还没有下发过报文。"));
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("log").striped(true).num_columns(3).show(ui, |ui| {
                    for entry in &self.log {
                        ui.monospace(&entry.at);
                        let mark = if entry.ok { "ok" } else { "fail" };
                        ui.monospace(format!("{mark}  {}", entry.frame));
                        ui.label(small(entry.note.clone()));
                        ui.end_row();
                    }
                });
            });
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.separator();
        egui::CollapsingHeader::new("设置")
            .default_open(false)
            .show(ui, |ui| {
                ui.checkbox(&mut self.settings.apply_on_start, "启动时自动下发一次当前档位");
                ui.label(small(
                    "休眠、热插拔后 EC 会丢状态；勾选后每次启动都会重发一遍。",
                ));

                if autostart::supported() {
                    let mut on = self.autostart_on;
                    if ui
                        .checkbox(&mut on, "开机自动启动（HKCU\\...\\Run，无需管理员）")
                        .changed()
                    {
                        *action = Some(Action::SetAutostart(on));
                    }
                } else {
                    ui.label(small("当前平台不支持开机自启（仅 Windows）"));
                }

                ui.horizontal(|ui| {
                    if ui.button("保存设置").clicked() {
                        *action = Some(Action::SaveSettings);
                    }
                    if ui.button("恢复默认").clicked() {
                        *action = Some(Action::ResetSettings);
                    }
                });
                ui.label(small(format!(
                    "配置文件：{}",
                    settings::config_path().display()
                )));
                match &self.font_note {
                    Some(path) => ui.label(small(format!("界面字体：{path}"))),
                    None => ui.label(small("未找到可用中文字体，界面可能显示为方块")),
                };
            });
    }

    fn draw_status(&mut self, root: &mut egui::Ui) {
        egui::Panel::bottom("status").show(root, |ui| {
            ui.horizontal(|ui| {
                let color = if self.status_bad {
                    egui::Color32::from_rgb(200, 80, 80)
                } else {
                    ui.visuals().weak_text_color()
                };
                ui.label(egui::RichText::new(self.status.clone()).color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(small(format!(
                        "档位 {}/{}",
                        self.settings.step, self.settings.steps
                    )));
                });
            });
        });
    }

    fn run_action(&mut self, action: Action) {
        match action {
            Action::Rescan => self.rescan(),
            Action::SendOff => {
                self.settings.on = false;
                self.send(led_proto::LED_OFF, "熄灭");
            }
            Action::SendCurrent => {
                self.settings.on = true;
                self.apply_current();
            }
            Action::SaveSettings => {
                self.settings.sanitize();
                match settings::save(&self.settings) {
                    Ok(()) => self.set_status("设置已保存".to_string(), false),
                    Err(err) => self.set_status(format!("保存设置失败：{err:#}"), true),
                }
            }
            Action::ResetSettings => {
                self.settings = Settings::default();
                self.set_status("已恢复默认设置".to_string(), false);
                self.apply_current();
            }
            Action::SetAutostart(on) => {
                let cmd = autostart::command_line();
                match autostart::set(on, &cmd) {
                    Ok(()) => {
                        self.autostart_on = on;
                        let msg = if on { "已登记开机自启" } else { "已取消开机自启" };
                        self.set_status(msg.to_string(), false);
                    }
                    Err(err) => self.set_status(format!("更新自启项失败：{err:#}"), true),
                }
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let before = self.settings.clone();
        let mut action: Option<Action> = None;

        self.poll_numlock();

        self.draw_header(root, &mut action);
        self.draw_status(root);
        self.draw_devices(root);

        egui::CentralPanel::default_margins().show(root, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    self.draw_lighting(ui, &mut action);
                    self.draw_follow(ui);
                    self.draw_log(ui);
                    self.draw_settings(ui, &mut action);
                });
        });

        if let Some(action) = action {
            self.run_action(action);
        }

        // 设置改动即时落盘，避免「调好了没保存」这类丢失
        if self.settings != before {
            self.settings.sanitize();
            let _ = settings::save(&self.settings);
        }

        if self.settings.follow_numlock {
            root.ctx()
                .request_repaint_after(Duration::from_millis(self.settings.poll_ms));
        }
    }
}

fn small(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into()).small().weak()
}

/// 日志时间戳（UTC，`HH:MM:SS`）。只用于看报文先后，不做展示级时区换算。
fn now_hms() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        % 86_400;
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
}
