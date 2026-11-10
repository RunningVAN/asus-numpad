//! 主界面。
//!
//! 界面按平台分两套，共用一个窗口骨架：
//!
//! * **Windows**：只做灯光（选接口、调档位、NumLock 联动、看报文）。键位与手势交给
//!   ASUS 官方驱动，界面一个字节都不碰 FEATURE 0x0d 之外的东西。
//! * **Linux**：全套。除灯光之外，驱动配置的每一项（机型、布局、键码风格、混合模式、
//!   手势阈值、背光通道、跟灯参数、时序、语言）都在 [`full`] 里有对应控件，另加系统
//!   状态、服务启停与命令行兜底。
//!
//! 所有档位映射都走 [`asus_numpad::brightness`] 与 [`asus_numpad::led_proto`]——和
//! Linux 侧同一份函数，避免两边灯效暗中不一致。

mod driver;
mod full;
mod windows;

use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;

use asus_numpad::led_policy::{RANK_WARN_ASUS_FALLBACK, RANK_WARN_NO_FEATURE};
use asus_numpad::led_proto;

use crate::backend::Backend;
use crate::cli_bridge::{self, Completion};
use crate::cmdform::{BrightAction, CommandForm, LedAction, LevelsAction, SubCmd};
use crate::settings::{self, Settings};
use crate::sysinfo::{self, ServiceVerb, Snapshot};
use crate::{autostart, backend, ui_font};

const MAX_LOG: usize = 200;

/// 平台状态里 systemctl 要真起进程，别每帧都刷。
const STATE_REFRESH: Duration = Duration::from_secs(10);

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
    ServiceRun(ServiceVerb),
    ServiceAutostart(bool),
    RefreshState,
    RunCommand,
    SaveDriver,
    SaveDriverAndRestart,
    ReloadDriver,
    ResetDriver,
}

/// 右侧页签。Windows 侧只有灯光，Linux 侧五页齐全。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Page {
    Light,
    Driver,
    System,
    Command,
    Settings,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Light => "灯光",
            Page::Driver => "驱动配置",
            Page::System => "系统状态",
            Page::Command => "命令行",
            Page::Settings => "设置",
        }
    }

    /// 启动时落在哪一页。默认灯光；`ASUS_NUMPAD_GUI_PAGE=driver|system|command|settings`
    /// 可以直接进对应页（文档截图与排障都省一次点击）。
    fn from_env() -> Self {
        let Ok(value) = std::env::var("ASUS_NUMPAD_GUI_PAGE") else {
            return Page::Light;
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "driver" | "cfg" | "config" => Page::Driver,
            "system" | "sys" => Page::System,
            "command" | "cmd" | "cli" => Page::Command,
            "settings" | "prefs" => Page::Settings,
            _ => Page::Light,
        }
    }
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
    /// 平台状态快照（机型、布局、背光通道、后台服务）
    state: Snapshot,
    state_at: Instant,
    /// 是否画 Linux 全套面板；Windows 只画灯光
    ui_full: bool,
    /// 右侧当前页（只有全套模式才显示页签）
    tab: Page,
    /// Linux 侧驱动配置（`asus-numpad.json`）的编辑草稿
    driver: driver::Draft,
    /// 后台服务是否开机自启；Windows 用注册表那份，Linux 用这一份
    service_enabled: bool,
    /// 命令面板：与 `asus-numpad` CLI 同一套实现，界面只负责拼参数
    cmd: CommandForm,
    cmd_bin: Option<std::path::PathBuf>,
    cmd_rx: Option<std::sync::mpsc::Receiver<Completion>>,
    cmd_running: bool,
    cmd_out: Option<String>,
    cmd_ok: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let font_note = ui_font::install(&cc.egui_ctx);
        let settings = settings::load();
        let state = sysinfo::snapshot();
        let service_enabled = state
            .service
            .as_ref()
            .map(|s| s.enabled.trim() == "enabled")
            .unwrap_or(false);
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
            state,
            state_at: Instant::now(),
            ui_full: ui_full(),
            tab: Page::from_env(),
            driver: driver::Draft::load(),
            service_enabled,
            cmd: CommandForm::default(),
            cmd_bin: cli_bridge::locate(),
            cmd_rx: None,
            cmd_running: false,
            cmd_out: None,
            cmd_ok: true,
        };
        app.rescan();
        if app.settings.apply_on_start && app.settings.on {
            app.apply_current();
        }
        app
    }

    /// 重读平台状态。服务启停之后必须调用，否则界面还显示旧状态。
    fn refresh_state(&mut self) {
        self.state = sysinfo::snapshot();
        self.state_at = Instant::now();
        self.service_enabled = self
            .state
            .service
            .as_ref()
            .map(|s| s.enabled.trim() == "enabled")
            .unwrap_or(false);
    }

    fn set_status(&mut self, text: String, bad: bool) {
        self.status = text;
        self.status_bad = bad;
    }

    /// 把表单拼成 `asus-numpad <args>` 交给后台线程跑。
    ///
    /// 不在这里直接 `Command::output()`：读 DMI、等 systemd 重启服务都可能要几百毫秒，
    /// 同步执行会把界面卡住。
    fn run_command(&mut self) {
        if self.cmd_running {
            return;
        }
        let Some(bin) = self.cmd_bin.clone() else {
            self.set_status(
                "找不到 asus-numpad 可执行文件：装好驱动包后点「重新查找」，或设置 ASUS_NUMPAD_BIN"
                    .to_string(),
                true,
            );
            return;
        };
        let args = match self.cmd.argv() {
            Ok(args) => args,
            Err(err) => {
                self.set_status(format!("参数有误：{err}"), true);
                return;
            }
        };
        let label = args.join(" ");
        let timeout = self.cmd.timeout();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let done = cli_bridge::run(&bin, args, timeout);
            let _ = tx.send(done);
        });
        self.cmd_rx = Some(rx);
        self.cmd_running = true;
        let note = match timeout {
            Some(limit) => format!(
                "正在执行 asus-numpad {label} ……（{} 秒后自动收工）",
                limit.as_secs()
            ),
            None => format!("正在执行 asus-numpad {label} ……"),
        };
        self.set_status(note, false);
    }

    fn poll_command(&mut self) {
        use std::sync::mpsc::TryRecvError;

        let Some(rx) = self.cmd_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(done) => {
                self.cmd_rx = None;
                self.cmd_running = false;
                self.finish_command(done);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.cmd_rx = None;
                self.cmd_running = false;
                self.set_status("命令行进程异常中断".to_string(), true);
            }
        }
    }

    fn finish_command(&mut self, done: Completion) {
        let ok = done.ok();
        let wrote = self.cmd.writes_config();
        let summary = done.summary();

        // 日志里放子命令本身与首行结果，完整输出留在输出框
        let label = if done.argv.len() > 1 {
            done.argv[1..].join(" ")
        } else {
            "asus-numpad".to_string()
        };
        let mut note = done
            .error
            .clone()
            .into_iter()
            .chain(done.stderr.lines().map(str::to_string))
            .chain(done.stdout.lines().map(str::to_string))
            .map(|line| line.trim().to_string())
            .find(|line| !line.is_empty())
            .unwrap_or_else(|| match done.code {
                Some(code) => format!("退出码 {code}"),
                None => "被信号终止".to_string(),
            });
        if note.chars().count() > 60 {
            note = note.chars().take(60).collect::<String>() + "…";
        }
        self.push_log(format!("cli {label}"), note.clone(), ok);

        self.cmd_ok = ok;
        self.cmd_out = Some(summary);
        let verdict = if ok {
            "完成".to_string()
        } else if done.timed_out {
            "已按时间上限终止，输出见下方".to_string()
        } else {
            note.clone()
        };
        self.set_status(format!("asus-numpad {label}：{verdict}"), !ok);

        // 写配置的子命令会改机型 / 布局 / 档位 / 语言，界面上的状态跟着重读一次
        if ok && wrote {
            self.refresh_state();
        }
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
                ui.heading(self.window_title());
                ui.label(small(self.state.subtitle.clone()));
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
                            // 两个平台没枚举到接口的原因不同，提示跟着后端走
                            let hint = if cfg!(windows) {
                                "确认 ASUS 官方驱动已安装、设备未被其它程序独占。"
                            } else {
                                "确认触摸板带 FEATURE 0x0d、hidraw 已加载，且 udev 规则已生效。"
                            };
                            ui.label(small(hint));
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
                                ui.label(egui::RichText::new("无可用 FEATURE 0x0d").small().weak());
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
                let label = if n == 4 {
                    "4 档（含熄灭）"
                } else {
                    "8 档（纯亮度）"
                };
                if ui
                    .selectable_label(self.settings.steps == n, label)
                    .clicked()
                {
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
                egui::Grid::new("log")
                    .striped(true)
                    .num_columns(3)
                    .show(ui, |ui| {
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

    /// 只读的平台状态 + 后台服务开关。
    ///
    /// 灯不亮时这一区先给答案：机型认没认出来、布局取的是哪套、背光通道在不在、
    /// 后台服务是否在跑。省掉「回去敲命令行再看一遍」这一步。
    fn draw_system(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("系统状态");
            if ui.button("刷新").clicked() {
                *action = Some(Action::RefreshState);
            }
        });

        egui::Grid::new("system_state")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                for (name, value) in &self.state.rows {
                    ui.label(name);
                    ui.label(small(value.clone()));
                    ui.end_row();
                }
            });

        for hint in &self.state.hints {
            ui.label(
                egui::RichText::new(hint.clone())
                    .small()
                    .color(egui::Color32::from_rgb(200, 150, 40)),
            );
        }

        if let Some(service) = &self.state.service {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let color = if service.is_active() {
                    egui::Color32::from_rgb(90, 170, 90)
                } else {
                    egui::Color32::from_rgb(200, 150, 40)
                };
                ui.label(
                    egui::RichText::new(format!(
                        "后台服务：{} / {}",
                        service.active, service.enabled
                    ))
                    .color(color),
                );
                if ui.button("启动").clicked() {
                    *action = Some(Action::ServiceRun(ServiceVerb::Start));
                }
                if ui.button("停止").clicked() {
                    *action = Some(Action::ServiceRun(ServiceVerb::Stop));
                }
                if ui.button("重启").clicked() {
                    *action = Some(Action::ServiceRun(ServiceVerb::Restart));
                }
            });
            ui.label(small(
                "用户单元不需要 root；停止后数字键与手势失效，触摸板恢复成普通触摸板。",
            ));
        }

        // 诊断出口放最后：它是「要发出去问人」时才点的，不该挤掉上面的状态与服务操作
        ui.add_space(6.0);
        self.draw_diagnostics(ui);
    }

    /// 诊断信息出口：内容与 `asus-numpad-gui --selftest` 同一份，省得为了看一眼接口和
    /// 报文去开终端。写文件只落在界面自己的配置目录旁边，不碰系统路径。
    fn draw_diagnostics(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("复制诊断信息").clicked() {
                let text = self.diagnostics();
                ui.ctx().copy_text(text);
                self.set_status("诊断信息已复制到剪贴板".to_string(), false);
            }
            if ui.button("保存诊断报告").clicked() {
                let text = self.diagnostics();
                let path = settings::config_path()
                    .parent()
                    .map(|dir| dir.join(format!("diag-{}.txt", stamp())));
                match path.map(|path| std::fs::write(&path, &text).map(|_| path)) {
                    Some(Ok(path)) => {
                        self.set_status(format!("诊断报告已写入 {}", path.display()), false)
                    }
                    Some(Err(err)) => {
                        self.set_status(format!("写诊断报告失败：{err}"), true);
                    }
                    None => self.set_status("取不到设置文件目录".to_string(), true),
                }
            }
        });
        ui.label(small(
            "内容：平台状态、接口列表、8 档与熄灭报文——和 --selftest 打印的一致。",
        ));
    }

    /// 与 `--selftest` 同构的诊断文本。
    fn diagnostics(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "后端：{}（真实硬件：{}）\n",
            self.backend.name(),
            self.backend.is_real()
        ));
        out.push_str(&format!("{}\n", self.state.subtitle));
        for (name, value) in &self.state.rows {
            out.push_str(&format!("  {name}：{value}\n"));
        }
        for hint in &self.state.hints {
            out.push_str(&format!("  提示：{hint}\n"));
        }
        if let Some(service) = &self.state.service {
            out.push_str(&format!(
                "  后台服务：{} / {}\n",
                service.active, service.enabled
            ));
        }

        if self.devices.is_empty() {
            out.push_str("未枚举到任何 HID 接口。\n");
        } else {
            out.push_str(&format!("共 {} 个接口：\n", self.devices.len()));
            for device in &self.devices {
                out.push_str(&format!(
                    "  {:<10} {:<8} feature={:<5} len={:<3} rank={:?} selected={}\n",
                    device.vid_pid(),
                    device.vendor(),
                    device.has_feature,
                    device.feature_len,
                    device.rank,
                    device.selected
                ));
                out.push_str(&format!("             path={}\n", device.path));
            }
        }

        out.push_str("\n档位报文：\n");
        for (index, level) in led_proto::brightness_levels().iter().enumerate() {
            if let Some(frame) = led_proto::encode(*level) {
                out.push_str(&format!(
                    "  {} 档 0x{:02x} -> {}\n",
                    index + 1,
                    level,
                    led_proto::hex(&frame)
                ));
            }
        }
        for (label, level) in [("熄灭", led_proto::LED_OFF), ("普通点亮", led_proto::LED_ON)] {
            if let Some(frame) = led_proto::encode(level) {
                out.push_str(&format!("{label}报文：{}\n", led_proto::hex(&frame)));
            }
        }
        out
    }

    /// 命令面板：把 CLI 的全部子命令接到界面上。
    ///
    /// 做法是拼参数、调 `asus-numpad` 本身，而不是在界面里重写一遍：`check` 要读 DMI，
    /// `levels` / `sku` / `lang` 要改配置文件，输出文案又跟着语言文件走，重写必然与
    /// CLI 漂移。界面上显示的命令行与终端里敲下的完全一致，可直接复制过去复现。
    fn draw_commands(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.separator();
        // 默认展开：这张面板就是给人「不去终端也能把所有子命令跑一遍」用的，收起来
        // 等于把功能藏起来；真嫌长的人点一下标题就收。
        egui::CollapsingHeader::new("命令行（asus-numpad 全部子命令）")
            .default_open(true)
            .show(ui, |ui| {
                self.draw_command_target(ui);
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("子命令");
                    egui::ComboBox::from_id_salt("cmd_sub")
                        .width(130.0)
                        .selected_text(self.cmd.sub.name())
                        .show_ui(ui, |ui| {
                            for sub in SubCmd::ALL {
                                ui.selectable_value(&mut self.cmd.sub, sub, sub.name());
                            }
                        });
                    ui.label(small(self.cmd.sub.about()));
                });

                ui.add_space(2.0);
                self.draw_command_args(ui);
                self.draw_command_advanced(ui);
                ui.add_space(4.0);
                self.draw_command_launcher(ui, action);
                self.draw_command_output(ui);
            });
    }

    fn draw_command_target(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match &self.cmd_bin {
                Some(path) => {
                    ui.label(small(format!("可执行文件：{}", path.display())));
                }
                None => {
                    ui.label(
                        egui::RichText::new("没找到 asus-numpad 可执行文件，命令面板暂不可用")
                            .small()
                            .color(WARN),
                    );
                }
            }
            if ui.button("重新查找").clicked() {
                self.cmd_bin = cli_bridge::locate();
            }
        });
        if self.cmd_bin.is_none() {
            ui.label(small(
                "装好驱动包后再点「重新查找」；或用环境变量 ASUS_NUMPAD_BIN 指向可执行文件。",
            ));
        }
        ui.label(small(
            "这里调的就是同一个 asus-numpad 二进制，输出与终端一致。`run` 是前台常驻引擎，界面上对应的入口在「系统状态」区的服务启停。",
        ));
    }

    fn draw_command_args(&mut self, ui: &mut egui::Ui) {
        match self.cmd.sub {
            SubCmd::Check => {
                ui.label(small("这条子命令不需要参数。"));
            }
            SubCmd::Dump => {
                ui.label(small(
                    "打印原始触摸板事件，本该一直跑到 Ctrl+C。这里只跑 5 秒就收工，把这段时间里的事件贴到下面。",
                ));
            }
            SubCmd::Bright => {
                let items: Vec<_> = BrightAction::ALL.iter().map(|a| (*a, a.label())).collect();
                ui.horizontal(|ui| {
                    ui.label("动作");
                    combo(ui, "cmd_bright_action", &mut self.cmd.bright_action, &items);
                });
                match self.cmd.bright_action {
                    BrightAction::Step => {
                        field(ui, "档位", &mut self.cmd.bright_step, "1 起，取决于档位数");
                    }
                    BrightAction::Follow => {
                        ui.checkbox(&mut self.cmd.bright_follow_on, "开启跟随（不勾＝关闭跟随）");
                        ui.label(small("会写入配置并重启后台服务。"));
                    }
                    BrightAction::Status => {
                        ui.label(small("只打印当前档位与跟随状态，不改配置。"));
                    }
                    BrightAction::Cycle | BrightAction::Up | BrightAction::Down => {
                        ui.label(small("按当前档位表挪一档，并重启后台服务。"));
                    }
                }
            }
            SubCmd::Levels => {
                let items: Vec<_> = LevelsAction::ALL.iter().map(|a| (*a, a.label())).collect();
                ui.horizontal(|ui| {
                    ui.label("动作");
                    combo(ui, "cmd_levels_action", &mut self.cmd.levels_action, &items);
                });
                match self.cmd.levels_action {
                    LevelsAction::Table => {
                        ui.label(small("打印每档对应的命令字。"));
                    }
                    LevelsAction::Reset => {
                        ui.label(small("把档位表恢复成默认，并重启后台服务。"));
                    }
                    LevelsAction::Set => {
                        field(ui, "档位", &mut self.cmd.levels_step, "从 1 开始");
                        field(
                            ui,
                            "命令字",
                            &mut self.cmd.levels_value,
                            "0x41 到 0x48，或十进制",
                        );
                        ui.label(small("相邻档必须单调，写反了 CLI 会拒绝并保持原配置。"));
                    }
                }
            }
            SubCmd::Led => {
                let items: Vec<_> = LedAction::ALL.iter().map(|a| (*a, a.label())).collect();
                ui.horizontal(|ui| {
                    ui.label("动作");
                    combo(ui, "cmd_led_action", &mut self.cmd.led_action, &items);
                });
                match self.cmd.led_action {
                    LedAction::Status => {
                        ui.label(small(
                            "CLI 的 led 没有状态查询，不带值只会打印用法。当前命令字看「灯光」区或 check。",
                        ));
                    }
                    LedAction::Raw => {
                        field(ui, "命令字", &mut self.cmd.led_raw, "如 0x41");
                        ui.label(small(
                            "绕开状态机直接下发：led on 亮而 led 0x41 不亮，就说明这台机器只吃两态。",
                        ));
                    }
                    LedAction::On | LedAction::Off => {
                        ui.label(small("直接从当前选中的接口下发一帧，不改配置。"));
                    }
                }
            }
            SubCmd::Layout => {
                field(
                    ui,
                    "布局名",
                    &mut self.cmd.layout_name,
                    "留空＝只查看，如 g614",
                );
                ui.checkbox(
                    &mut self.cmd.layout_ids,
                    "同时打印 HID 号 → 布局对照表（--ids）",
                );
                if !self.cmd.layout_name.trim().is_empty() {
                    ui.label(small("填了布局名就会写配置并重启后台服务。"));
                }
            }
            SubCmd::Sku => {
                field(
                    ui,
                    "机型",
                    &mut self.cmd.sku_id,
                    "留空＝查看；list＝列全部；auto＝恢复自动识别",
                );
                ui.label(small("指定具体机型 ID 会写配置，覆盖自动识别结果。"));
            }
            SubCmd::Lang => {
                field(
                    ui,
                    "语言",
                    &mut self.cmd.lang_code,
                    "留空＝查看；auto / en / zh_CN / zh_TW",
                );
            }
        }
    }

    fn draw_command_advanced(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("高级选项（全局参数）")
            .default_open(false)
            .show(ui, |ui| {
                let adv = &mut self.cmd.advanced;
                egui::Grid::new("cmd_adv")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        let rows: [(&str, &mut String, &str); 12] = [
                            ("--config", &mut adv.config, "配置文件路径"),
                            ("--device", &mut adv.device, "指定触摸板设备"),
                            ("--keyboard", &mut adv.keyboard, "键盘设备（读背光状态用）"),
                            ("--led", &mut adv.led, "LED 设备"),
                            ("--hidraw", &mut adv.hidraw, "hidraw 节点名，如 hidraw3"),
                            ("--i2c-bus", &mut adv.i2c_bus, "整数"),
                            ("--i2c-addr", &mut adv.i2c_addr, "如 0x15"),
                            ("--i2c-transfer", &mut adv.i2c_transfer, "i2c 传输方式"),
                            ("--key-mode", &mut adv.key_mode, "键盘扫描码模式"),
                            ("--layout", &mut adv.layout, "覆盖布局名"),
                            ("--sku", &mut adv.sku, "覆盖机型 ID"),
                            ("--numlock", &mut adv.numlock, "覆盖 NumLock 状态来源"),
                        ];
                        for (flag, value, hint) in rows {
                            ui.monospace(flag);
                            ui.add(
                                egui::TextEdit::singleline(value)
                                    .hint_text(hint)
                                    .desired_width(240.0),
                            );
                            ui.end_row();
                        }
                        ui.monospace("--mixed-mode");
                        ui.add(
                            egui::TextEdit::singleline(&mut adv.mixed_mode)
                                .hint_text("混排模式")
                                .desired_width(240.0),
                        );
                        ui.end_row();
                        ui.monospace("--pressure-min");
                        ui.add(
                            egui::TextEdit::singleline(&mut adv.pressure_min)
                                .hint_text("整数")
                                .desired_width(240.0),
                        );
                        ui.end_row();
                    });

                ui.add_space(2.0);
                ui.checkbox(&mut adv.verbose, "--verbose  打印识别与通道细节");
                ui.checkbox(&mut adv.dump_config, "--dump-config  打印解析前的原始配置");
                ui.checkbox(
                    &mut adv.no_follow_brightness,
                    "--no-follow-brightness  不跟随键盘背光",
                );
                ui.checkbox(
                    &mut adv.allow_unverified,
                    "--allow-unverified  允许未验证机型 / 布局",
                );
                ui.horizontal(|ui| {
                    ui.checkbox(&mut adv.grab, "--grab  独占抓取输入设备");
                    ui.checkbox(&mut adv.no_grab, "--no-grab");
                });
                ui.label(small(
                    "留空的项不会出现在命令行里；--grab 与 --no-grab 只能勾一个。",
                ));
            });
    }

    fn draw_command_launcher(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let bin_name = self
            .cmd_bin
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "asus-numpad".to_string());
        let preview = self.cmd.preview(&bin_name);
        let writes = self.cmd.writes_config();

        ui.label(egui::RichText::new(preview.clone()).monospace().small());
        ui.horizontal(|ui| {
            let can_run = self.cmd_bin.is_some() && !self.cmd_running;
            let text = if self.cmd_running {
                "执行中……"
            } else {
                "执行"
            };
            if ui.add_enabled(can_run, egui::Button::new(text)).clicked() {
                *action = Some(Action::RunCommand);
            }
            if ui.button("复制命令").clicked() {
                ui.ctx().copy_text(preview.clone());
            }
            if writes {
                ui.label(egui::RichText::new("会写入配置文件").small().color(WARN));
            }
        });
    }

    fn draw_command_output(&mut self, ui: &mut egui::Ui) {
        if self.cmd_running {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(small("等待进程返回……"));
            });
        }

        let Some(output) = self.cmd_out.clone() else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label("输出");
            if ui.button("复制").clicked() {
                ui.ctx().copy_text(output.clone());
            }
            if ui.button("清空").clicked() {
                self.cmd_out = None;
            }
        });
        if self.cmd_out.is_none() {
            return;
        }

        let color = if self.cmd_ok {
            ui.visuals().text_color()
        } else {
            egui::Color32::from_rgb(210, 90, 90)
        };
        egui::ScrollArea::vertical()
            .id_salt("cmd_output")
            .max_height(240.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(output).monospace().small().color(color))
                        .wrap(),
                );
            });
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        // 设置页是页签里的独立一页，再套一层默认折叠的标题就等于把内容藏起来
        egui::CollapsingHeader::new("设置")
            .default_open(true)
            .show(ui, |ui| {
                ui.checkbox(
                    &mut self.settings.apply_on_start,
                    "启动时自动下发一次当前档位",
                );
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
                } else if sysinfo::has_service() {
                    // Linux 的「自启」就是 systemd 用户单元的 enable，和后台服务共用一份状态
                    let mut on = self.service_enabled;
                    if ui
                        .checkbox(&mut on, "开机自启（systemctl --user enable asus-numpad）")
                        .changed()
                    {
                        *action = Some(Action::ServiceAutostart(on));
                    }
                } else {
                    ui.label(small("当前平台不支持开机自启"));
                }

                ui.horizontal(|ui| {
                    if ui.button("保存设置").clicked() {
                        *action = Some(Action::SaveSettings);
                    }
                    if ui.button("恢复默认").clicked() {
                        *action = Some(Action::ResetSettings);
                    }
                });
                match &self.font_note {
                    Some(path) => ui.label(small(format!("界面字体：{path}"))),
                    None => ui.label(small("未找到可用中文字体，界面可能显示为方块")),
                };
            });

        // 路径与版本：出问题时第一个要看的就是这几行
        ui.separator();
        ui.label(egui::RichText::new("路径与版本").strong());
        egui::Grid::new("settings_paths")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("界面设置");
                ui.label(small(settings::config_path().display().to_string()));
                ui.end_row();
                ui.label("驱动配置");
                ui.label(small(self.driver.write_path.display().to_string()));
                ui.end_row();
                ui.label("当前接口");
                ui.label(small(
                    self.devices
                        .get(self.selected.unwrap_or(usize::MAX))
                        .map(|device| device.path.clone())
                        .unwrap_or_else(|| "未选择".to_string()),
                ));
                ui.end_row();
                ui.label("版本");
                ui.label(small(format!(
                    "界面 {} / 库 {}",
                    env!("CARGO_PKG_VERSION"),
                    asus_numpad::VERSION
                )));
                ui.end_row();
            });

        ui.horizontal(|ui| {
            if ui.button("打开设置目录").clicked() {
                let dir = settings::config_path();
                let dir = dir.parent().unwrap_or(&dir).to_path_buf();
                self.report_open(open_dir(&dir), &dir);
            }
            if ui.button("打开驱动配置目录").clicked() {
                let file = self.driver.write_path.clone();
                let dir = file.parent().unwrap_or(&file).to_path_buf();
                self.report_open(open_dir(&dir), &dir);
            }
        });
    }

    fn report_open(&mut self, result: std::io::Result<()>, dir: &std::path::Path) {
        match result {
            Ok(()) => self.set_status(format!("已打开 {}", dir.display()), false),
            Err(err) => self.set_status(format!("打不开 {}：{err}", dir.display()), true),
        }
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
                        let msg = if on {
                            "已登记开机自启"
                        } else {
                            "已取消开机自启"
                        };
                        self.set_status(msg.to_string(), false);
                    }
                    Err(err) => self.set_status(format!("更新自启项失败：{err:#}"), true),
                }
            }
            Action::ServiceRun(verb) => {
                match sysinfo::service_run(verb) {
                    Ok(msg) => self.set_status(msg, false),
                    Err(err) => self.set_status(format!("{err:#}"), true),
                }
                // 服务状态刚变过，立刻重读，否则界面还显示旧值
                self.refresh_state();
            }
            Action::ServiceAutostart(on) => {
                match sysinfo::service_autostart(on) {
                    Ok(msg) => {
                        self.service_enabled = on;
                        self.set_status(msg, false);
                    }
                    Err(err) => self.set_status(format!("{err:#}"), true),
                }
                self.refresh_state();
            }
            Action::RefreshState => self.refresh_state(),
            Action::RunCommand => self.run_command(),
            Action::SaveDriver => match self.driver.save() {
                Ok(msg) => self.set_status(msg, false),
                Err(err) => self.set_status(format!("保存驱动配置失败：{err:#}"), true),
            },
            Action::SaveDriverAndRestart => {
                match self.driver.save() {
                    Ok(msg) => match sysinfo::service_run(ServiceVerb::Restart) {
                        Ok(done) => self.set_status(format!("{msg}；{done}"), false),
                        Err(err) => self.set_status(
                            format!("{msg}，但后台服务重启失败：{err:#}"),
                            true,
                        ),
                    },
                    Err(err) => self.set_status(format!("保存驱动配置失败：{err:#}"), true),
                }
                self.refresh_state();
            }
            Action::ReloadDriver => {
                self.driver.reload();
                let path = self.driver.write_path.display().to_string();
                self.set_status(format!("已重新读取 {path}"), false);
            }
            Action::ResetDriver => {
                self.driver.reset();
                self.set_status("驱动配置已恢复官方默认值（还需点保存）".to_string(), false);
            }
        }
    }

    fn window_title(&self) -> &'static str {
        if self.ui_full {
            "ASUS NumberPad 控制中心"
        } else {
            "ASUS NumberPad 灯光控制"
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 文档截图模式（设置了 ASUS_NUMPAD_GUI_SHOT 才生效）
        crate::shot::tick(root.ctx());

        let before = self.settings.clone();
        let mut action: Option<Action> = None;

        self.poll_numlock();
        self.poll_command();

        // 机型 / 服务状态在会话里会变（插拔、systemctl 被别处调用），定期重读一次
        if self.state_at.elapsed() > STATE_REFRESH {
            self.refresh_state();
        }

        self.draw_header(root, &mut action);
        self.draw_status(root);
        self.draw_devices(root);

        egui::CentralPanel::default_margins().show(root, |ui| {
            if self.ui_full {
                ui.horizontal_wrapped(|ui| {
                    for page in [
                        Page::Light,
                        Page::Driver,
                        Page::System,
                        Page::Command,
                        Page::Settings,
                    ] {
                        if ui
                            .selectable_label(self.tab == page, page.title())
                            .clicked()
                        {
                            self.tab = page;
                        }
                    }
                });
                ui.separator();
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .id_salt(("page", self.tab))
                .show(ui, |ui| match (self.ui_full, self.tab) {
                    (false, _) => {
                        self.draw_lighting(ui, &mut action);
                        self.draw_follow(ui);
                        self.draw_log(ui);
                        self.draw_lighting_note(ui, &mut action);
                        self.draw_settings(ui, &mut action);
                    }
                    (true, Page::Light) => {
                        self.draw_lighting(ui, &mut action);
                        self.draw_follow(ui);
                        self.draw_log(ui);
                    }
                    (true, Page::Driver) => self.draw_full_controls(ui, &mut action),
                    (true, Page::System) => self.draw_system(ui, &mut action),
                    (true, Page::Command) => self.draw_commands(ui, &mut action),
                    (true, Page::Settings) => self.draw_settings(ui, &mut action),
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

/// 用系统文件管理器打开一个目录。只读操作，失败就如实报出来。
fn open_dir(dir: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(windows)]
    let opener = "explorer";
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    let opener = "xdg-open";
    std::process::Command::new(opener)
        .arg(dir)
        .spawn()
        .map(|_| ())
}

/// 诊断报告文件名用的时间戳（Unix 秒，够排序了）。
fn stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 是否画 Linux 全套面板。
///
/// 默认按平台走：Linux 全套、Windows 只灯光。环境变量 `ASUS_NUMPAD_GUI_FULL=1/0`
/// 可以强制覆盖，用于在另一平台上预览对面那套界面（真机默认行为不受影响）。
pub fn ui_full() -> bool {
    let Ok(value) = std::env::var("ASUS_NUMPAD_GUI_FULL") else {
        return cfg!(target_os = "linux");
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "on" | "true" | "full" | "yes" => true,
        "0" | "off" | "false" | "lite" | "no" => false,
        _ => cfg!(target_os = "linux"),
    }
}

pub fn window_title() -> &'static str {
    if ui_full() {
        "ASUS NumberPad 控制中心"
    } else {
        "ASUS NumberPad 灯光控制"
    }
}

fn small(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into()).small().weak()
}

/// 「会写配置」「没找到可执行文件」这类提示色，和系统状态区的告警保持一致。
const WARN: egui::Color32 = egui::Color32::from_rgb(200, 150, 40);

/// 下拉框：`items` 里第一项通常就是默认值，找不到匹配时显示空串而不是 panic。
fn combo<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut T,
    items: &[(T, &'static str)],
) {
    let current = items
        .iter()
        .find(|(candidate, _)| candidate == value)
        .map(|(_, label)| *label)
        .unwrap_or("");
    egui::ComboBox::from_id_salt(id)
        .width(230.0)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (candidate, label) in items {
                ui.selectable_value(value, *candidate, *label);
            }
        });
}

/// 一行「标签 + 输入框」。空串一律视为「不传这个参数」，由 cmdform 判空。
fn field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .desired_width(220.0),
        )
    })
    .inner
}

/// 日志时间戳（UTC，`HH:MM:SS`）。只用于看报文先后，不做展示级时区换算。
fn now_hms() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        % 86_400;
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}
