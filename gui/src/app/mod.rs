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

mod calib;
mod driver;
mod extend;
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
use crate::kbdmap;
use crate::settings::{self, Settings};
use crate::sysinfo::{self, ServiceVerb, Snapshot};
use crate::theme::{self, Palette};
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
    /// 按指定 level 下发（键盘四档映射表里逐档试亮用）
    SendLevel(u8, String),
    /// 把键盘四档映射整表写进驱动配置（`manual_brightness_levels`）
    SyncLevelsToDriver,
    /// 从驱动配置读回档位表，覆盖界面上的四档映射
    LoadLevelsFromDriver,
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
    /// 扩展：自定义布局 / 报文 / 压感 / 热键
    Extend,
    /// 校准：自定义分区与手动校准（非原生机型）
    Calib,
    System,
    Command,
    Settings,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Light => "灯光",
            Page::Driver => "驱动配置",
            Page::Extend => "扩展",
            Page::Calib => "校准",
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
            "extend" | "ext" | "extension" => Page::Extend,
            "calib" | "cal" | "calibration" => Page::Calib,
            "system" | "sys" => Page::System,
            "command" | "cmd" | "cli" => Page::Command,
            "settings" | "prefs" => Page::Settings,
            _ => Page::Light,
        }
    }
}

pub struct App {
    /// 当前生效的调色板（深浅色各一套，见 [`crate::theme`]）
    palette: Palette,
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
    /// 扩展页「校验扩展项」的结果（布局装没装上、HID 打没打架、报文合不合法）
    ext_notes: Vec<String>,
    /// 扩展页里自定义报文模板的十六进制草稿
    ext_proto: String,
    /// 草稿是否正被编辑：编辑期间不用配置里的值覆盖，免得打到一半跳一下
    ext_proto_dirty: bool,
    ext_proto_err: Option<String>,
    /// 扩展页「配置片段」文本框：四项打包成一段 JSON，方便同机型之间互传
    ext_snippet: String,
    /// 片段导入结果：`(文案, 是否出错)`
    ext_snippet_msg: Option<(String, bool)>,
    /// 校准页：样本文件路径（真机采到的落点）
    calib_samples_path: String,
    /// 校准页：解算时按几列几行（与键表一致）
    calib_cols: usize,
    calib_rows: usize,
    /// 校准页：回放结果 `(文案, 是否出错)`
    calib_msg: Option<(String, bool)>,
    /// 校准页「配置片段」：partition + calibration 打包
    calib_snippet: String,
    calib_snippet_msg: Option<(String, bool)>,
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
        // 主题要在建界面之前装好，首帧就是定稿的样子（截图也才稳定）
        let palette = theme::install(&cc.egui_ctx);
        let settings = settings::load();
        let state = sysinfo::snapshot();
        let service_enabled = state
            .service
            .as_ref()
            .map(|s| s.enabled.trim() == "enabled")
            .unwrap_or(false);
        let driver = driver::Draft::load();
        // 报文草稿从配置里的模板起步；之后的编辑由 draw_ext_proto 自己同步
        let ext_proto = extend::hex_of(&driver.cfg.led_proto.template);
        let mut app = Self {
            palette,
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
            driver,
            ext_notes: Vec::new(),
            ext_proto,
            ext_proto_dirty: false,
            ext_proto_err: None,
            ext_snippet: String::new(),
            ext_snippet_msg: None,
            calib_samples_path: String::new(),
            calib_cols: 5,
            calib_rows: 4,
            calib_msg: None,
            calib_snippet: String::new(),
            calib_snippet_msg: None,
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

    /// 当前档位组对应的原始命令字表。
    ///
    /// 4 档（对齐键盘背光）用界面上的映射表，逐档可自定义；8 档是纯亮度控制，
    /// 仍按 `0x00`/`0x41..=0x48` 线性等分，与 Linux 侧同一函数。
    fn level_table(&self) -> Vec<u8> {
        if self.settings.steps as usize == kbdmap::KBD_STEPS {
            return self.settings.kbd_map.clone();
        }
        backend::level_table(self.settings.steps)
    }

    fn level_for_step(&self, step: u8) -> Option<u8> {
        if self.settings.steps as usize == kbdmap::KBD_STEPS {
            return kbdmap::level_for(step, &self.settings.kbd_map);
        }
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

    /// 把灯光页的四档映射整表写进驱动配置。
    ///
    /// 界面设置与驱动配置本来是两份东西，但档位表是两边共用的语义：界面上调好的
    /// 四档若不落到 `manual_brightness_levels`，拔插或重启后服务仍按旧表点灯。
    /// 这里直接写驱动那份文件，写完再把结果读回界面，保证看到的就是磁盘上的值。
    fn sync_levels_to_driver(&mut self) {
        let map = self.settings.kbd_map.clone();
        if !kbdmap::monotonic(&map) {
            self.set_status(
                format!(
                    "未写入：{} 不是从暗到亮递增，驱动不接受这样的档位表",
                    hex_table(&map)
                ),
                true,
            );
            return;
        }
        self.driver.cfg.manual_brightness_levels = Some(map);
        self.driver.dirty = true;
        match self.driver.save() {
            Ok(msg) => {
                if let Some(table) = self.driver.cfg.manual_brightness_levels.clone() {
                    if table.len() == kbdmap::KBD_STEPS {
                        self.settings.kbd_map = table;
                    }
                }
                let now = hex_table(&self.settings.kbd_map);
                self.set_status(format!("{msg}；四档映射已同步为 {now}"), false);
            }
            Err(err) => self.set_status(format!("写入驱动配置失败：{err:#}"), true),
        }
    }

    /// 反过来：把驱动配置里的档位表读进灯光页，按服务当前实际在用的表点灯。
    fn load_levels_from_driver(&mut self) {
        match self.driver.cfg.manual_brightness_levels.clone() {
            Some(table) if table.len() == kbdmap::KBD_STEPS => {
                let mut table = table;
                kbdmap::sanitize(&mut table);
                self.settings.kbd_map = table;
                let now = hex_table(&self.settings.kbd_map);
                self.set_status(format!("已读入驱动配置的四档映射：{now}"), false);
            }
            Some(table) => self.set_status(
                format!(
                    "驱动配置的档位表是 {} 档，套不进键盘四档（先在驱动配置页改成 4 档）",
                    table.len()
                ),
                true,
            ),
            None => self.set_status(
                "驱动配置没有显式档位表（驱动自己线性等分），界面保持当前映射".to_string(),
                true,
            ),
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

    /// 顶栏：品牌块 + 两级标题 + 右侧状态胶囊与「重新扫描」。
    ///
    /// 原来是一行 `heading + 小字 + 按钮`，层级挤在一起；这里把「这是什么工具」
    /// 「当前连的是哪块硬件」拆成两行，右侧状态改用胶囊，一眼能扫到。
    fn draw_header(&mut self, root: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let real = self.backend.is_real();
        let backend_name = self.backend.name().to_string();
        let bar = egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(p.panel)
                    .inner_margin(egui::Margin::symmetric(20, 12)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    brand_mark(ui, &p);
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(self.window_title())
                                .size(18.0)
                                .strong()
                                .color(p.text),
                        );
                        ui.label(
                            egui::RichText::new(self.state.subtitle.clone())
                                .size(11.5)
                                .color(p.text_weak),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let rescan = theme::tip(
                            theme::secondary(ui, &p, "重新扫描"),
                            "重新枚举 HID 接口：热插拔设备、或刚装上官方驱动之后点它。",
                        );
                        if rescan.clicked() {
                            *action = Some(Action::Rescan);
                        }
                        let (dot, tint) = if real {
                            ("●  ", p.ok)
                        } else {
                            ("●  ", p.warn)
                        };
                        theme::pill(
                            ui,
                            &format!("{dot}{backend_name}"),
                            if real { p.ok } else { p.warn },
                        )
                        .on_hover_text(if real {
                            "已连到真实 HID 接口"
                        } else {
                            "模拟后端：报文只进日志，不会碰硬件"
                        });
                        let _ = tint;
                    });
                });
            });
        // 顶栏与内容区之间只放一条极淡的分隔线：比给面板加描边更轻，
        // 但足以让白色顶栏和灰色内容底分属两层。
        let r = bar.response.rect;
        root.painter()
            .hline(r.x_range(), r.bottom(), egui::Stroke::new(1.0, p.stroke));
    }

    /// 左侧接口列表：每个接口一张小卡，选中态用强调色淡底 + 描边。
    ///
    /// 原来是 `selectable_label` 加一堆紧跟的小字，条目之间只隔一条细线，多个接口时
    /// 很难看出「哪一张卡是这台设备」。改成卡片后边界清楚，路径这类长文本可以直接折行。
    fn draw_devices(&mut self, root: &mut egui::Ui) {
        let p = self.palette;
        let mut clicked: Option<usize> = None;
        egui::Panel::left("devices")
            .resizable(true)
            .default_size(340.0)
            .frame(
                egui::Frame::new()
                    .fill(p.panel)
                    .inner_margin(egui::Margin::symmetric(16, 14)),
            )
            .show(root, |ui| {
                theme::title_row_help(
                    ui,
                    &p,
                    "HID 接口",
                    Some("驱动通过 FEATURE 0x0d 控制的那一路 HID 接口；同一台机器上出现多个时，选带 NumberPad 小键盘的那个。"),
                );
                theme::note(
                    ui,
                    &p,
                    format!("{} · 共 {} 个接口", self.backend.name(), self.devices.len()),
                );
                ui.add_space(4.0);

                if !self.backend.is_real() {
                    theme::pill(ui, "模拟后端", p.warn).on_hover_text("报文只进日志，不会碰硬件");
                    ui.add_space(4.0);
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.devices.is_empty() {
                            ui.label(egui::RichText::new("没有枚举到任何 HID 接口").color(p.text));
                            // 两个平台没枚举到接口的原因不同，提示跟着后端走
                            let hint = if cfg!(windows) {
                                "确认 ASUS 官方驱动已安装、设备未被其它程序独占。"
                            } else {
                                "确认触摸板带 FEATURE 0x0d、hidraw 已加载，且 udev 规则已生效。"
                            };
                            theme::note(ui, &p, hint);
                            return;
                        }
                        for (i, d) in self.devices.iter().enumerate() {
                            let selected = self.selected == Some(i);
                            let name = if d.product.is_empty() {
                                d.tail().to_string()
                            } else {
                                d.product.clone()
                            };
                            let card = egui::Frame::new()
                                // 选中态不再画整圈描边：侧栏条目改用「淡底 + 左侧色条」表达选中，
                                // 像系统设置的侧边栏，条目之间也就不需要边框来分隔
                                .fill(if selected {
                                    p.accent_soft
                                } else {
                                    egui::Color32::TRANSPARENT
                                })
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::symmetric(12, 9))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    if selected {
                                        // 色条先画、文字后画，色条只占左侧 2.5px，不会压到内容
                                        let bar = egui::Rect::from_min_size(
                                            egui::pos2(
                                                ui.max_rect().left() - 9.0,
                                                ui.max_rect().top() + 2.0,
                                            ),
                                            egui::Vec2::new(
                                                2.5,
                                                (ui.available_height() - 4.0).max(18.0),
                                            ),
                                        );
                                        ui.painter().rect_filled(
                                            bar,
                                            egui::CornerRadius::same(2),
                                            p.accent,
                                        );
                                    }
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} · {}",
                                            d.vid_pid(),
                                            d.vendor()
                                        ))
                                        .size(13.0)
                                        .strong()
                                        .color(p.text),
                                    );
                                    ui.label(
                                        egui::RichText::new(name).size(12.0).color(p.text_weak),
                                    );
                                    let (tint, text) = if d.usable() {
                                        (
                                            p.ok,
                                            format!(
                                                "FEATURE 0x0d 可用 · 报告 {} 字节",
                                                d.feature_len
                                            ),
                                        )
                                    } else {
                                        (p.warn, "无可用 FEATURE 0x0d".to_string())
                                    };
                                    ui.label(egui::RichText::new(text).size(11.5).color(tint));
                                    match d.rank {
                                        Some(RANK_WARN_NO_FEATURE) => {
                                            ui.label(
                                                egui::RichText::new(
                                                    "优先级：兜底（缺 FEATURE 报告）",
                                                )
                                                .size(11.5)
                                                .color(p.warn),
                                            );
                                        }
                                        Some(RANK_WARN_ASUS_FALLBACK) => {
                                            ui.label(
                                                egui::RichText::new(
                                                    "优先级：ASUS 兜底（OMNI RECEIVER）",
                                                )
                                                .size(11.5)
                                                .color(p.text_weak),
                                            );
                                        }
                                        _ => {}
                                    }
                                    ui.label(
                                        egui::RichText::new(d.path.clone())
                                            .size(11.0)
                                            .color(p.text_weak),
                                    );
                                })
                                .response;
                            let resp =
                                ui.interact(card.rect, card.id.with("row"), egui::Sense::click());
                            // 卡片里的路径与产品名在窄侧栏里会被截断，悬浮一次给全。
                            let resp = theme::tip(
                                resp,
                                format!(
                                    "{} · {}\n{}\n{}",
                                    d.vid_pid(),
                                    d.vendor(),
                                    if d.product.is_empty() {
                                        d.tail().to_string()
                                    } else {
                                        d.product.clone()
                                    },
                                    d.path
                                ),
                            );
                            if resp.clicked() {
                                clicked = Some(i);
                            }
                            ui.add_space(8.0);
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

    /// 灯光页：档位组 / 亮度 / 动作分三块，每块一张卡片。
    ///
    /// 亮度选择从 `Slider` 换成自绘档位条（[`theme::level_meter`]）：滑块看不到一共有
    /// 几档、也不知下一档在哪个方向，档位条一格一档，点哪格就是哪档。
    fn draw_lighting(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let steps = self.settings.steps;
        let table = self.level_table();
        let current = self.settings.step;

        theme::section(ui, &p, "亮度档位", |ui| {
            theme::row(
                ui,
                &p,
                "档位组",
                Some("4 档＝和键盘背光的四个亮度档一一对齐；8 档＝只用触摸板自身的纯亮度分档，过渡更细。"),
                |ui| {
                let items = [
                    (
                        4u8,
                        "4 档 · 对齐键盘背光",
                        "把键盘背光的 4 个档映射到触摸板，观感与官方四档一致",
                    ),
                    (
                        8u8,
                        "8 档 · 纯亮度",
                        "只用触摸板自身的 8 档亮度，不跟键盘背光走",
                    ),
                ];
                if let Some(picked) = theme::segmented(ui, &p, "steps", steps, &items) {
                    self.settings.steps = picked;
                    if self.settings.step > picked {
                        self.settings.step = picked;
                    }
                }
            });
            ui.add_space(6.0);

            // 档位条要用到下面算好的 steps / current，所以先取一份当前值
            let value = self.settings.step;
            if let Some(picked) =
                theme::ctrl_column(ui, |ui| theme::level_meter(ui, &p, "level", value, steps))
            {
                self.settings.step = picked;
                *action = Some(Action::SendCurrent);
            }
            ui.add_space(4.0);

            let level = self.level_for_step(self.settings.step);
            theme::row(
                ui,
                &p,
                "当前档位",
                Some("界面上选中的档位，以及它真正写进 FEATURE 0x0d 的 level 字节。"),
                |ui| {
                    let text = match level {
                        Some(0) => "当前：熄灭".to_string(),
                        Some(v) => format!("当前：第 {} 档 · 0x{v:02x}", self.settings.step),
                        None => format!("当前：第 {} 档", self.settings.step),
                    };
                    ui.label(
                        egui::RichText::new(text)
                            .size(13.0)
                            .strong()
                            .color(p.accent),
                    );
                    ui.label(
                        egui::RichText::new(format!("共 {steps} 档 · 点格子直接切档"))
                            .size(12.0)
                            .color(p.text_weak),
                    );
                },
            );
            ui.add_space(2.0);
            let mapped = table
                .iter()
                .map(|v| format!("0x{v:02x}"))
                .collect::<Vec<_>>()
                .join("  ");
            theme::row(
                ui,
                &p,
                "档位 → 报文",
                Some("每档下发时用的 level 字节：0x41 最暗到 0x48 最亮，0x00 是熄灭。"),
                |ui| {
                    ui.label(
                        egui::RichText::new(mapped)
                            .size(11.5)
                            .monospace()
                            .color(p.text_weak),
                    );
                },
            );

            ui.add_space(8.0);
            theme::row(
                ui,
                &p,
                "下发",
                Some("两个动作都只碰背光，不改配置：点亮＝按当前档重发一帧，熄灭＝下发 0x00。"),
                |ui| {
                    if theme::tip(
                        theme::primary(ui, &p, "点亮"),
                        "把当前档位立刻写进背光。休眠、热插拔后 EC 丢状态时用它找回亮度。",
                    )
                    .clicked()
                    {
                        *action = Some(Action::SendCurrent);
                    }
                    if theme::tip(
                        theme::secondary(ui, &p, "熄灭"),
                        "下发 0x00 熄灯；数字键和手势不受影响。",
                    )
                    .clicked()
                    {
                        *action = Some(Action::SendOff);
                    }
                },
            );
        });

        ui.add_space(10.0);

        if steps as usize == kbdmap::KBD_STEPS {
            theme::section(ui, &p, "键盘 4 档 → 触摸板亮度", |ui| {
                self.draw_kbd_map(ui, action);
            });
        } else {
            theme::section(ui, &p, "8 档映射", |ui| {
                theme::note(
                    ui,
                    &p,
                    format!(
                        "线性等分：{mapped_note}",
                        mapped_note = table
                            .iter()
                            .map(|v| format!("0x{v:02x}"))
                            .collect::<Vec<_>>()
                            .join("  ")
                    ),
                );
                theme::note(
                    ui,
                    &p,
                    "8 档是纯亮度控制；切到 4 档即可逐档对齐键盘背光的四个亮度档。",
                );
            });
        }

        ui.add_space(10.0);
        let _ = current;
    }

    /// 键盘 4 档 → 触摸板亮度的逐档编辑器。
    ///
    /// 键盘背光只有 BIOS 给的 4 档，触摸板侧能写的是 FEATURE `0x0d` 的 level 字节
    /// （`0x00` 灭 / `0x41..=0x48` 八档，没有中间灰度）。这里把两条轴对齐：每一档
    /// 单独选触摸板档位、单独试亮，还可以整表写进驱动配置，让后台服务用同一份表。
    fn draw_kbd_map(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        theme::note(
            ui,
            &p,
            "键盘第 N 档对应哪个触摸板档位逐档可选；第 1 档选「熄灭」即与官方四档观感一致。",
        );

        let mut retouch_current = false;
        // 一行一档：标签在左列、下拉框与「试亮」在控件列、帧报文在说明列。四行严格对齐，
        // 不必再猜列宽——之前用 Grid 手写四列宽度，总宽比控件列宽出一截，整张表就歪在
        // 卡片左边，和上下表单对不上。
        for step in 1..=kbdmap::KBD_STEPS as u8 {
            let index = (step - 1) as usize;
            let mut level = self.settings.kbd_map[index];
            let frame = kbdmap::frame_preview(level);
            theme::row_hint(
                ui,
                &p,
                &format!("键盘第 {step} 档"),
                Some("键盘背光由 BIOS 给出的 4 个档位，这里逐档对齐触摸板档位。"),
                &frame,
                |ui| {
                    let combo_width = theme::ctrl_width() - theme::BTN_W - theme::ROW_GAP;
                    let combo = egui::ComboBox::from_id_salt(("kbd_map", step))
                        .width(combo_width)
                        .selected_text(kbdmap::level_label(level))
                        .show_ui(ui, |ui| {
                            for (value, text) in kbdmap::level_choices() {
                                ui.selectable_value(&mut level, value, text);
                            }
                        });
                    theme::tip(
                        combo.response,
                        "这一档对应的触摸板亮度；选「熄灭」即与官方四档观感一致。",
                    );
                    let send = theme::secondary(ui, &p, "试亮");
                    if theme::tip(send, format!("把第 {step} 档的亮度立刻下发一次")).clicked()
                    {
                        *action = Some(Action::SendLevel(
                            level,
                            format!("键盘第 {step} 档 → {}", kbdmap::level_label(level)),
                        ));
                    }
                },
            );

            if level != self.settings.kbd_map[index] {
                self.settings.kbd_map[index] = level;
                // 改的正好是当前档：顺手把新亮度刷上去，省一次点击
                if self.settings.on && self.settings.step == step {
                    retouch_current = true;
                }
            }
        }
        if retouch_current {
            *action = Some(Action::SendCurrent);
        }

        ui.add_space(4.0);
        theme::row(
            ui,
            &p,
            "整表操作",
            Some("三颗按钮作用在整张四档表上；写入前会先检查是否从暗到亮递增。"),
            |ui| {
                // 三颗按钮平分控件列，右边缘和上面的下拉框齐平
                let picked = theme::actions(
                    ui,
                    &p,
                    &[
                        ("恢复默认", false, "恢复成与官方四档观感一致的映射。"),
                        (
                            "写入驱动",
                            false,
                            "把这张表写进驱动的 manual_brightness_levels，后台服务随即按它映射。",
                        ),
                        ("读回驱动", false, "用驱动配置里的档位表覆盖界面上这张表。"),
                    ],
                );
                match picked {
                    Some(0) => {
                        self.settings.kbd_map = kbdmap::default_map();
                        *action = Some(Action::SendCurrent);
                    }
                    Some(1) => *action = Some(Action::SyncLevelsToDriver),
                    Some(2) => *action = Some(Action::LoadLevelsFromDriver),
                    _ => {}
                }
            },
        );

        if kbdmap::monotonic(&self.settings.kbd_map) {
            theme::note(ui, &p, "整表从暗到亮递增，可直接写入驱动配置。")
        } else {
            theme::row_hint(
                ui,
                &p,
                "校验",
                Some("驱动要求相邻档不得变暗，否则写入会被拒绝。"),
                "不是递增：写入驱动会被拒绝",
                |ui| ui.label(egui::RichText::new("未通过").size(12.5).color(WARN)),
            );
            theme::note(
                ui,
                &p,
                "当前档位表不是从暗到亮递增，驱动要求相邻档不得变暗，写入驱动配置会被拒绝。",
            );
        }
        match self.driver.cfg.manual_brightness_levels.as_ref() {
            Some(table) => {
                theme::read_row(
                    ui,
                    &p,
                    "驱动档位表",
                    &format!("{}（{} 档）", hex_table(table), table.len()),
                    Some("驱动配置里 manual_brightness_levels 的当前值，后台服务按这张表映射键盘四档。"),
                );
            }
            None => theme::note(ui, &p, "驱动配置当前未指定档位表（由驱动自己线性等分）。"),
        }
        theme::read_row(
            ui,
            &p,
            "四档报文",
            &kbdmap::frames_text(&self.settings.kbd_map),
            Some("四档依次下发的 FEATURE 0x0d 帧，写入驱动配置时就是这四行。"),
        );
    }

    fn draw_follow(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::section(ui, &p, "联动", |ui| {
            theme::check_row(
                ui,
                &p,
                &mut self.settings.follow_numlock,
                "跟随 NumLock",
                Some("NumLock 关掉就自动熄灯，打开就恢复。状态按下面的间隔轮询一次。"),
            );
            if self.settings.follow_numlock {
                theme::row(
                    ui,
                    &p,
                    "轮询间隔",
                    Some("读 NumLock 状态的周期。调小反应更快，但每秒会多读几次 sysfs。"),
                    |ui| {
                        ui.add(
                            egui::Slider::new(&mut self.settings.poll_ms, 100..=2000)
                                .step_by(50.0)
                                .suffix(" ms"),
                        );
                    },
                );
                theme::read_row(
                    ui,
                    &p,
                    "当前状态",
                    &self.numlock_text(),
                    Some("NumLock 的实时状态，取自驱动上报；与键盘指示灯应当一致。"),
                );
                if self.sent_numlock.is_none() {
                    theme::note(ui, &p, "等待第一次状态读取……")
                }
            }
            if !self.backend.is_real() {
                let mut mock_on = self.backend.numlock().unwrap_or(true);
                if theme::check_row(
                    ui,
                    &p,
                    &mut mock_on,
                    "模拟 NumLock 状态",
                    Some("只有模拟后端才有：手动设定 NumLock 真假，用来验证联动逻辑。"),
                )
                .changed()
                {
                    self.backend.set_numlock(mock_on);
                    self.last_numlock = Some(mock_on);
                }
            }
        });
    }

    fn draw_log(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::section(ui, &p, "报文日志", |ui| {
            let picked = theme::row(
                ui,
                &p,
                "记录",
                Some("每次下发都会记一行，用来对照硬件表现。"),
                |ui| {
                    theme::actions(
                        ui,
                        &p,
                        &[("清空", false, "清空这份日志；不影响硬件状态，也不动配置。")],
                    )
                },
            );
            if picked == Some(0) {
                self.log.clear();
            }
            if self.log.is_empty() {
                theme::note(ui, &p, "还没有下发过报文。");
                return;
            }
            egui::ScrollArea::vertical()
                .max_height(150.0)
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
        });
    }

    /// 只读的平台状态 + 后台服务开关。
    ///
    /// 灯不亮时这一区先给答案：机型认没认出来、布局取的是哪套、背光通道在不在、
    /// 后台服务是否在跑。省掉「回去敲命令行再看一遍」这一步。
    fn draw_system(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                theme::title_row(ui, &p, "系统状态");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::tip(
                        theme::secondary(ui, &p, "刷新"),
                        "重读机型、布局、背光通道与后台服务状态。插拔设备后点一下即可。",
                    )
                    .clicked()
                    {
                        *action = Some(Action::RefreshState);
                    }
                });
            });

            for (name, value) in &self.state.rows {
                theme::read_row(ui, &p, name, value, state_help(name));
            }

            for hint in &self.state.hints {
                theme::row(
                    ui,
                    &p,
                    "提示",
                    Some("驱动运行时给出的即时提示，出现这行黄字说明某项配置需要留意。"),
                    |ui| {
                        ui.label(
                            egui::RichText::new(hint.clone())
                                .small()
                                .color(egui::Color32::from_rgb(200, 150, 40)),
                        );
                    },
                );
            }

            if let Some(service) = &self.state.service {
                ui.add_space(4.0);
                let color = if service.is_active() {
                    egui::Color32::from_rgb(90, 170, 90)
                } else {
                    egui::Color32::from_rgb(200, 150, 40)
                };
                // 服务状态是一行值，走 read_row 就落在控件列；三颗按钮另起一行平分控件列，
                // 不再让「一段状态文字 + 三颗按钮」挤在一行、右边缘和别处对不上。
                theme::row(
                    ui,
                    &p,
                    "后台服务",
                    Some("用户单元不需要 root；停止后数字键与手势失效，触摸板恢复成普通触摸板。"),
                    |ui| {
                        ui.label(egui::RichText::new("状态").size(12.5).color(p.text_weak));
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{} / {}",
                                service.active, service.enabled
                            ))
                            .size(12.5)
                            .color(color),
                        );
                    },
                );
                let picked = theme::row(
                    ui,
                    &p,
                    "服务控制",
                    Some("systemd 用户单元 asus-numpad.service 的启停与重启；重启用于让刚写入的驱动配置生效。"),
                    |ui| {
                    theme::actions(
                        ui,
                        &p,
                        &[
                            ("启动", false, "systemctl --user start asus-numpad"),
                            ("停止", false, "systemctl --user stop asus-numpad"),
                            ("重启", false, "改完驱动配置后用它让新配置生效"),
                        ],
                    )
                });
                match picked {
                    Some(0) => *action = Some(Action::ServiceRun(ServiceVerb::Start)),
                    Some(1) => *action = Some(Action::ServiceRun(ServiceVerb::Stop)),
                    Some(2) => *action = Some(Action::ServiceRun(ServiceVerb::Restart)),
                    _ => {}
                }
                theme::note(
                    ui,
                    &p,
                    "用户单元不需要 root；停止后数字键与手势失效，触摸板恢复成普通触摸板。",
                )
            }

            // 诊断出口放最后：它是「要发出去问人」时才点的，不该挤掉上面的状态与服务操作
            ui.add_space(6.0);
            self.draw_diagnostics(ui);
        });
    }

    /// 诊断信息出口：内容与 `asus-numpad-gui --selftest` 同一份，省得为了看一眼接口和
    /// 报文去开终端。写文件只落在界面自己的配置目录旁边，不碰系统路径。
    fn draw_diagnostics(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let picked =
            theme::row(
                ui,
                &p,
                "诊断导出",
                Some("内容：平台状态、接口列表、8 档与熄灭报文——和 --selftest 打印的一致。"),
                |ui| {
                    theme::actions(
                    ui,
                    &p,
                    &[
                        ("复制诊断", false, "把整份诊断文本放进剪贴板，方便贴到 issue 里。"),
                        (
                            "保存报告",
                            false,
                            "在界面设置文件旁边写一份 diag-时间戳.txt，内容与 --selftest 一致。",
                        ),
                    ],
                )
                },
            );
        {
            if picked == Some(0) {
                let text = self.diagnostics();
                ui.ctx().copy_text(text);
                self.set_status("诊断信息已复制到剪贴板".to_string(), false);
            }
            if picked == Some(1) {
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
        }
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
        for (label, level) in [
            ("熄灭", led_proto::LED_OFF),
            ("普通点亮", led_proto::LED_ON),
        ] {
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
        let p = self.palette;
        // 页签已经是独立一页，内部不再折叠：收起来等于把功能藏起来。
        theme::section(ui, &p, "命令行（asus-numpad 全部子命令）", |ui| {
            self.draw_command_target(ui);
            ui.add_space(4.0);

            theme::row(
                ui,
                &p,
                "子命令",
                Some("选一条 asus-numpad 子命令，下面的参数会跟着变；执行时拼出的命令行与终端里完全一致。"),
                |ui| {
                    let combo = egui::ComboBox::from_id_salt("cmd_sub")
                        .width(crate::theme::ctrl_width())
                        .selected_text(self.cmd.sub.name())
                        .show_ui(ui, |ui| {
                            for sub in SubCmd::ALL {
                                ui.selectable_value(&mut self.cmd.sub, sub, sub.name());
                            }
                        });
                    theme::tip(combo.response, "切换子命令；右侧一句是该子命令的作用。");
                },
            );

            ui.add_space(2.0);
            // 子命令自己带一句说明（CmdForm::about），直接摆在下拉框下面：
            // 名字叫 markers / levels 的子命令，光看名字猜不出它是读还是写。
            theme::note(ui, &p, self.cmd.sub.about());
            ui.add_space(6.0);
            self.draw_command_args(ui);
            self.draw_command_advanced(ui);
            ui.add_space(4.0);
            self.draw_command_launcher(ui, action);
            self.draw_command_output(ui);
        });
    }

    fn draw_command_target(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        // 可执行文件是一行只读值，走 read_row 就自动落在控件列里、超长时截断，
        // 不再像以前那样「一段文字 + 一颗按钮」挤在一行，按钮宽度还跟别处不一样。
        let shown = match &self.cmd_bin {
            Some(path) => path.display().to_string(),
            None => "未找到，点下面的「重新查找」".to_string(),
        };
        theme::read_row(
            ui,
            &p,
            "可执行文件",
            &shown,
            Some("界面调用的就是同一个 asus-numpad 二进制，输出与终端完全一致。"),
        );
        let missing = self.cmd_bin.is_none();
        let picked = theme::row(ui, &p, "", None, |ui| {
            theme::actions(
                ui,
                &p,
                &[("重新查找", false, "按 PATH 与环境变量重找一次可执行文件。")],
            )
        });
        if picked == Some(0) {
            self.cmd_bin = cli_bridge::locate();
        }
        if missing {
            theme::note(
                ui,
                &p,
                "装好驱动包后再点「重新查找」；或用环境变量 ASUS_NUMPAD_BIN 指向可执行文件。",
            )
        }
        theme::note(
            ui,
            &p,
            "这里调的就是同一个 asus-numpad 二进制，输出与终端一致。`run` 是前台常驻引擎，界面上对应的入口在「系统状态」区的服务启停。",
        );
    }

    fn draw_command_args(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        match self.cmd.sub {
            SubCmd::Check => theme::note(ui, &p, "这条子命令不需要参数。"),
            SubCmd::Dump => {
                theme::note(
                    ui,
                    &p,
                    "打印原始触摸板事件，本该一直跑到 Ctrl+C。这里只跑 5 秒就收工，把这段时间里的事件贴到下面。",
                );
            }
            SubCmd::Bright => {
                let items: Vec<_> = BrightAction::ALL.iter().map(|a| (*a, a.label())).collect();
                theme::row(
                    ui,
                    &p,
                    "动作",
                    Some("bright 的动作：step 设档、cycle/up/down 挪一档、follow 开关跟随、status 只看状态。"),
                    |ui| combo(ui, "cmd_bright_action", &mut self.cmd.bright_action, &items),
                );
                match self.cmd.bright_action {
                    BrightAction::Step => {
                        field(
                            ui,
                            &p,
                            "档位",
                            &mut self.cmd.bright_step,
                            "1 起，取决于档位数",
                            Some("要切到的档位序号，从 1 开始，上限是当前档位组的大小。"),
                        );
                    }
                    BrightAction::Follow => {
                        theme::check_row(
                            ui,
                            &p,
                            &mut self.cmd.bright_follow_on,
                            "开启跟随",
                            Some(
                                "不勾＝关闭跟随；关闭后不管键盘背光怎么变，触摸板都保持固定亮度。",
                            ),
                        );
                        theme::note(ui, &p, "会写入配置并重启后台服务。")
                    }
                    BrightAction::Status => {
                        theme::note(ui, &p, "只打印当前档位与跟随状态，不改配置。")
                    }
                    BrightAction::Cycle | BrightAction::Up | BrightAction::Down => {
                        theme::note(ui, &p, "按当前档位表挪一档，并重启后台服务。")
                    }
                }
            }
            SubCmd::Levels => {
                let items: Vec<_> = LevelsAction::ALL.iter().map(|a| (*a, a.label())).collect();
                theme::row(
                    ui,
                    &p,
                    "动作",
                    Some("levels 的动作：table 打印档位表、set 单档改命令字、reset 恢复默认。"),
                    |ui| combo(ui, "cmd_levels_action", &mut self.cmd.levels_action, &items),
                );
                match self.cmd.levels_action {
                    LevelsAction::Table => theme::note(ui, &p, "打印每档对应的命令字。"),
                    LevelsAction::Reset => {
                        theme::note(ui, &p, "把档位表恢复成默认，并重启后台服务。")
                    }
                    LevelsAction::Set => {
                        field(
                            ui,
                            &p,
                            "档位",
                            &mut self.cmd.levels_step,
                            "从 1 开始",
                            Some("要改的是第几档。"),
                        );
                        field(
                            ui,
                            &p,
                            "命令字",
                            &mut self.cmd.levels_value,
                            "0x41 到 0x48，或十进制",
                            Some("这一档要写的 level 字节，合法范围 0x41..=0x48。"),
                        );
                        theme::note(ui, &p, "相邻档必须单调，写反了 CLI 会拒绝并保持原配置。")
                    }
                }
            }
            SubCmd::Led => {
                let items: Vec<_> = LedAction::ALL.iter().map(|a| (*a, a.label())).collect();
                theme::row(
                    ui,
                    &p,
                    "动作",
                    Some("led 的动作：on/off 直接下发，raw 绕开状态机写指定命令字。"),
                    |ui| combo(ui, "cmd_led_action", &mut self.cmd.led_action, &items),
                );
                match self.cmd.led_action {
                    LedAction::Status => {
                        theme::note(
                            ui,
                            &p,
                            "CLI 的 led 没有状态查询，不带值只会打印用法。当前命令字看「灯光」区或 check。",
                        );
                    }
                    LedAction::Raw => {
                        field(
                            ui,
                            &p,
                            "命令字",
                            &mut self.cmd.led_raw,
                            "如 0x41",
                            Some("要直接下发的 level 字节，十六进制或十进制都可以。"),
                        );
                        theme::note(
                            ui,
                            &p,
                            "绕开状态机直接下发：led on 亮而 led 0x41 不亮，就说明这台机器只吃两态。",
                        );
                    }
                    LedAction::On | LedAction::Off => {
                        theme::note(ui, &p, "直接从当前选中的接口下发一帧，不改配置。")
                    }
                }
            }
            SubCmd::Layout => {
                field(
                    ui,
                    &p,
                    "布局名",
                    &mut self.cmd.layout_name,
                    "留空＝只查看，如 g614",
                    Some("填布局名会写配置；留空只打印当前布局与可选列表。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut self.cmd.layout_ids,
                    "打印 HID 对照表",
                    Some("勾上相当于加 --ids：把 HID 号与布局的对照表一并打印出来。"),
                );
                if !self.cmd.layout_name.trim().is_empty() {
                    theme::note(ui, &p, "填了布局名就会写配置并重启后台服务。")
                }
            }
            SubCmd::Sku => {
                field(
                    ui,
                    &p,
                    "机型",
                    &mut self.cmd.sku_id,
                    "留空＝查看；list＝列全部；auto＝恢复自动识别",
                    Some("填具体机型 ID 会覆盖自动识别结果；list 列全部机型，auto 恢复自动。"),
                );
                theme::note(ui, &p, "指定具体机型 ID 会写配置，覆盖自动识别结果。")
            }
            SubCmd::Lang => {
                field(
                    ui,
                    &p,
                    "语言",
                    &mut self.cmd.lang_code,
                    "留空＝查看；auto / en / zh_CN / zh_TW",
                    Some("界面与日志的语言：auto 跟系统，也可指定 en / zh_CN / zh_TW。"),
                );
            }
        }
    }

    fn draw_command_advanced(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("高级选项（全局参数）")
            .default_open(false)
            .show(ui, |ui| {
                let p = self.palette;
                let adv = &mut self.cmd.advanced;
                let rows: [(&str, &mut String, &str, &str); 12] = [
                    (
                        "--config",
                        &mut adv.config,
                        "配置文件路径",
                        "改用另一份配置文件，默认走系统的 /etc/asus-numpad/config.json。",
                    ),
                    (
                        "--device",
                        &mut adv.device,
                        "指定触摸板设备",
                        "覆盖自动挑选的触摸板节点。",
                    ),
                    (
                        "--keyboard",
                        &mut adv.keyboard,
                        "键盘设备（读背光状态用）",
                        "跟着键盘背光走时，从这里读当前背光档。",
                    ),
                    (
                        "--led",
                        &mut adv.led,
                        "LED 设备",
                        "指定 LEDs 节点，一般是 input::numlock。",
                    ),
                    (
                        "--hidraw",
                        &mut adv.hidraw,
                        "hidraw 节点名，如 hidraw3",
                        "直连 hidraw 下发 FEATURE 报文时用。",
                    ),
                    (
                        "--i2c-bus",
                        &mut adv.i2c_bus,
                        "整数",
                        "走 i2c 通道时指定总线号。",
                    ),
                    (
                        "--i2c-addr",
                        &mut adv.i2c_addr,
                        "如 0x15",
                        "走 i2c 通道时指定从机地址。",
                    ),
                    (
                        "--i2c-transfer",
                        &mut adv.i2c_transfer,
                        "i2c 传输方式",
                        "i2c 的传输模式，默认 i2ctransfer。",
                    ),
                    (
                        "--key-mode",
                        &mut adv.key_mode,
                        "键盘扫描码模式",
                        "键码风格：raw / mapped 一类，和布局配套。",
                    ),
                    (
                        "--layout",
                        &mut adv.layout,
                        "覆盖布局名",
                        "临时覆盖配置里的布局，不落盘。",
                    ),
                    (
                        "--sku",
                        &mut adv.sku,
                        "覆盖机型 ID",
                        "临时覆盖配置里的机型，不落盘。",
                    ),
                    (
                        "--numlock",
                        &mut adv.numlock,
                        "覆盖 NumLock 状态来源",
                        "临时指定 NumLock 状态来源，如 sysfs / evdev。",
                    ),
                ];
                for (flag, value, hint, help) in rows {
                    theme::row(ui, &p, flag, Some(help), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(value)
                                .hint_text(hint)
                                .desired_width(crate::theme::ctrl_width()),
                        );
                    });
                }
                theme::row(
                    ui,
                    &p,
                    "--mixed-mode",
                    Some("混排模式：有虚拟触摸板就开（auto）/ 强制开 / 纯数字键盘。"),
                    |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut adv.mixed_mode)
                                .hint_text("混排模式")
                                .desired_width(crate::theme::ctrl_width()),
                        );
                    },
                );

                ui.add_space(2.0);
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.verbose,
                    "--verbose",
                    Some("打印识别过程与通道细节，排查收获接口时用。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.dump_config,
                    "--dump-config",
                    Some("打印解析前的原始配置，用来确认配置有没有被正确读到。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.no_follow_brightness,
                    "--no-follow-brightness",
                    Some("不跟随键盘背光，触摸板保持自己的档位。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.allow_unverified,
                    "--allow-unverified",
                    Some("允许在未验证机型 / 布局上运行，键位可能对不上。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.grab,
                    "--grab",
                    Some("独占抓取输入设备；与 --no-grab 互斥，只能勾一个。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.no_grab,
                    "--no-grab",
                    Some("不独占抓取设备；与 --grab 互斥。"),
                );
                theme::note(
                    ui,
                    &p,
                    "留空的项不会出现在命令行里；--grab 与 --no-grab 只能勾一个。",
                );
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
        let p = self.palette;
        let can_run = self.cmd_bin.is_some() && !self.cmd_running;

        // 命令行预览：等宽字体、整行可点选复制
        egui::Frame::new()
            .fill(p.sunk)
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(preview.clone()).monospace().size(12.5))
                        .wrap(),
                );
            });
        ui.add_space(6.0);

        let text = if self.cmd_running {
            "执行中……"
        } else {
            "执行"
        };
        let picked = theme::ctrl_column(ui, |ui| {
            theme::actions(
                ui,
                &p,
                &[
                    (text, true, "按上面这条命令跑一次，输出贴在下面。"),
                    ("复制命令", false, "把这条命令行复制到剪贴板。"),
                ],
            )
        });
        let _ = can_run;
        if picked == Some(0) && can_run {
            *action = Some(Action::RunCommand);
        }
        if picked == Some(1) {
            ui.ctx().copy_text(preview.clone());
        }
        if writes {
            theme::note(ui, &p, "这条子命令会写入配置文件。")
        }
    }

    fn draw_command_output(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        if self.cmd_running {
            theme::row(
                ui,
                &p,
                "状态",
                Some("命令已提交，正在等 asus-numpad 进程退出并回传输出。"),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(small("等待进程返回……"));
                    });
                },
            );
        }

        let Some(output) = self.cmd_out.clone() else {
            return;
        };
        let picked = theme::row(
            ui,
            &p,
            "输出",
            Some("上一条命令的输出，标准输出与标准错误合并显示；只是界面上的文本，不影响配置。"),
            |ui| {
                theme::actions(
                    ui,
                    &p,
                    &[
                        ("复制", false, "把这段输出复制到剪贴板。"),
                        ("清空", false, "只清界面上的这段输出，不影响配置。"),
                    ],
                )
            },
        );
        if picked == Some(0) {
            ui.ctx().copy_text(output.clone());
        }
        if picked == Some(1) {
            self.cmd_out = None;
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
        let p = self.palette;
        theme::section(ui, &p, "启动行为", |ui| {
            theme::check_row(
                ui,
                &p,
                &mut self.settings.apply_on_start,
                "启动时下发当前档位",
                Some("勾上后每次打开界面都重发一遍当前档位，省得手动点「点亮」。"),
            );
            theme::note(
                ui,
                &p,
                "休眠、热插拔后 EC 会丢状态；勾选后每次启动都会重发一遍。",
            );

            if autostart::supported() {
                let mut on = self.autostart_on;
                if theme::check_row(
                    ui,
                    &p,
                    &mut on,
                    "开机自动启动",
                    Some("写入 HKCU\\...\\Run，不需要管理员权限；取消勾选即删除该项。"),
                )
                .changed()
                {
                    *action = Some(Action::SetAutostart(on));
                }
            } else if sysinfo::has_service() {
                // Linux 的「自启」就是 systemd 用户单元的 enable，和后台服务共用一份状态
                let mut on = self.service_enabled;
                if theme::check_row(
                    ui,
                    &p,
                    &mut on,
                    "开机自启",
                    Some(
                        "等价于 systemctl --user enable asus-numpad，和系统状态页共用同一份状态。",
                    ),
                )
                .changed()
                {
                    *action = Some(Action::ServiceAutostart(on));
                }
            } else {
                theme::note(ui, &p, "当前平台不支持开机自启")
            }

            ui.add_space(8.0);
            let picked = theme::row(
                ui,
                &p,
                "设置",
                Some("写的是界面自己的设置文件；驱动配置不受影响。"),
                |ui| {
                    theme::actions(
                        ui,
                        &p,
                        &[
                            (
                                "保存设置",
                                true,
                                "把上面的开关写进界面的设置文件，下次打开沿用。",
                            ),
                            ("恢复默认", false, "界面设置回到初始值；驱动配置不受影响。"),
                        ],
                    )
                },
            );
            match picked {
                Some(0) => *action = Some(Action::SaveSettings),
                Some(1) => *action = Some(Action::ResetSettings),
                _ => {}
            }
            match &self.font_note {
                Some(path) => {
                    theme::read_row(ui, &p, "界面字体", path, Some("界面实际加载的中文字体文件"));
                }
                None => theme::note(ui, &p, "未找到可用中文字体，界面可能显示为方块"),
            };
        });

        ui.add_space(10.0);

        // 路径与版本：出问题时第一个要看的就是这几行
        theme::section(ui, &p, "路径与版本", |ui| {
            theme::read_row(
                ui,
                &p,
                "界面设置",
                &settings::config_path().display().to_string(),
                Some("界面自己的设置文件：档位、联动间隔、开自启等。"),
            );
            theme::read_row(
                ui,
                &p,
                "驱动配置",
                &self.driver.write_path.display().to_string(),
                Some("后台服务读的那份配置；界面上「保存并重启服务」改的就是它。"),
            );
            theme::read_row(
                ui,
                &p,
                "当前接口",
                &self
                    .devices
                    .get(self.selected.unwrap_or(usize::MAX))
                    .map(|device| device.path.clone())
                    .unwrap_or_else(|| "未选择".to_string()),
                Some("当前选中的 HID 接口节点，报文都从这里出去。"),
            );
            theme::read_row(
                ui,
                &p,
                "版本",
                &format!(
                    "界面 {} / 库 {}",
                    env!("CARGO_PKG_VERSION"),
                    asus_numpad::VERSION
                ),
                Some("界面与驱动库各自的版本；对不上说明装的是混搭包。"),
            );

            ui.add_space(6.0);
            let picked = theme::row(
                ui,
                &p,
                "打开目录",
                Some("在系统文件管理器里定位配置所在目录，方便直接改配置文件。"),
                |ui| {
                    theme::actions(
                        ui,
                        &p,
                        &[
                            ("设置目录", false, "在文件管理器里打开界面设置所在目录。"),
                            ("驱动目录", false, "在文件管理器里打开驱动配置所在目录。"),
                        ],
                    )
                },
            );
            match picked {
                Some(0) => {
                    let dir = settings::config_path();
                    let dir = dir.parent().unwrap_or(&dir).to_path_buf();
                    self.report_open(open_dir(&dir), &dir);
                }
                Some(1) => {
                    let file = self.driver.write_path.clone();
                    let dir = file.parent().unwrap_or(&file).to_path_buf();
                    self.report_open(open_dir(&dir), &dir);
                }
                _ => {}
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
        let p = self.palette;
        let bar = egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(p.panel)
                    .inner_margin(egui::Margin::symmetric(20, 8)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    let color = if self.status_bad { p.bad } else { p.text_weak };
                    ui.label(
                        egui::RichText::new(self.status.clone())
                            .size(12.5)
                            .color(color),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // 状态栏常显「键盘第几档 → 哪个亮度值」，看一眼就知道当前映射对不对。
                        // 左侧状态文字可能很长（报文 + 说明），这里只放最短形式，避免被挤出窗口。
                        let level = self.level_for_step(self.settings.step);
                        let text = match (self.settings.steps as usize == kbdmap::KBD_STEPS, level)
                        {
                            (true, Some(value)) => format!(
                                "键盘 {}/{} → 0x{value:02x}",
                                self.settings.step,
                                kbdmap::KBD_STEPS
                            ),
                            (false, Some(value)) => {
                                format!("{}/8 → 0x{value:02x}", self.settings.step)
                            }
                            (_, None) => {
                                format!("档位 {}/{}", self.settings.step, self.settings.steps)
                            }
                        };
                        theme::pill(ui, &text, p.text_weak).on_hover_text(
                            "当前生效的档位与下发值：键盘第几档对应 FEATURE 0x0d 的 level 字节",
                        );
                    });
                });
            });
        let r = bar.response.rect;
        root.painter()
            .hline(r.x_range(), r.top(), egui::Stroke::new(1.0, p.stroke));
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
            Action::SendLevel(level, note) => {
                self.settings.on = level != led_proto::LED_OFF;
                self.send(level, &note);
            }
            Action::SyncLevelsToDriver => self.sync_levels_to_driver(),
            Action::LoadLevelsFromDriver => self.load_levels_from_driver(),
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
                        Err(err) => {
                            self.set_status(format!("{msg}，但后台服务重启失败：{err:#}"), true)
                        }
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
    /// 截图模式用：每帧先把指针放到「要拍气泡的那个控件」上（正常启动是空操作）
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        crate::shot::raw_input_hook(raw_input);
    }

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

        let p = self.palette;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.bg)
                    .inner_margin(egui::Margin::symmetric(20, 16)),
            )
            .show(root, |ui| {
                if self.ui_full {
                    let pages = [
                        (
                            Page::Light,
                            Page::Light.title(),
                            "灯光：档位条、四档映射、NumLock 联动、报文日志",
                        ),
                        (
                            Page::Driver,
                            Page::Driver.title(),
                            "驱动配置：机型、布局、键码风格、手势阈值、背光通道、时序、语言",
                        ),
                        (
                            Page::Extend,
                            Page::Extend.title(),
                            "扩展：给表外机型补自定义布局、换点亮报文、压感切开关、绑全局热键",
                        ),
                        (
                            Page::Calib,
                            Page::Calib.title(),
                            "校准：给表外机型自定义网格分区，并按实测落点校准面板范围与分辨率",
                        ),
                        (
                            Page::System,
                            Page::System.title(),
                            "系统状态：认出哪台机、走哪条背光通道、后台服务状态、诊断导出",
                        ),
                        (
                            Page::Command,
                            Page::Command.title(),
                            "命令行：把 asus-numpad 全部子命令拼好再执行，输出与终端一致",
                        ),
                        (
                            Page::Settings,
                            Page::Settings.title(),
                            "设置：启动行为、开机自启、配置文件路径与版本",
                        ),
                    ];
                    if let Some(page) = theme::segmented(ui, &p, "tabs", self.tab, &pages) {
                        self.tab = page;
                    }
                    // 页签下面补一行当前页的说明：页签只有两个字，光靠它进来会不知道
                    // 这一页在管什么，又不该用大标题再重复一遍页签名，所以只留小字副题。
                    if let Some((_, _, help)) = pages.iter().find(|(page, _, _)| *page == self.tab)
                    {
                        if let Some((_, lead)) = help.split_once('：') {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(lead).size(12.5).color(p.text_weak));
                        }
                    }
                    ui.add_space(12.0);
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .id_salt(("page", self.tab))
                    .show(ui, |ui| {
                        // 控件列宽度整页只定一次，各卡片、各折叠区里的行都照这一个宽度排
                        theme::begin_page(ui);
                        match (self.ui_full, self.tab) {
                            (false, _) => {
                                self.draw_lighting(ui, &mut action);
                                ui.add_space(10.0);
                                self.draw_follow(ui);
                                ui.add_space(10.0);
                                self.draw_log(ui);
                                ui.add_space(10.0);
                                self.draw_lighting_note(ui, &mut action);
                                ui.add_space(10.0);
                                self.draw_settings(ui, &mut action);
                            }
                            (true, Page::Light) => {
                                self.draw_lighting(ui, &mut action);
                                ui.add_space(10.0);
                                self.draw_follow(ui);
                                ui.add_space(10.0);
                                self.draw_log(ui);
                            }
                            (true, Page::Driver) => self.draw_full_controls(ui, &mut action),
                            (true, Page::Extend) => self.draw_extension(ui, &mut action),
                            (true, Page::Calib) => self.draw_calib(ui, &mut action),
                            (true, Page::System) => self.draw_system(ui, &mut action),
                            (true, Page::Command) => self.draw_commands(ui, &mut action),
                            (true, Page::Settings) => self.draw_settings(ui, &mut action),
                        }
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

/// 顶栏左侧的品牌块：强调色圆角方块 + 白色「NP」。
///
/// 纯色块比放一张位图省事，深浅色主题下都成立，也不占资源。
fn brand_mark(ui: &mut egui::Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(34.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(10), p.accent);
    // 顶部压一道极淡的白色内高光：单色圆角方块看起来是贴上去的色块，
    // 有高光之后更像一枚有厚度的图标，成本只有一次圆角矩形填充。
    let sheen = egui::Rect::from_min_max(
        rect.shrink(1.0).left_top(),
        egui::pos2(rect.right() - 1.0, rect.top() + 15.0),
    );
    painter.rect_filled(
        sheen,
        egui::CornerRadius {
            nw: 9,
            ne: 9,
            sw: 0,
            se: 0,
        },
        egui::Color32::from_white_alpha(26),
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "NP",
        egui::FontId::proportional(14.5),
        egui::Color32::WHITE,
    );
}

fn small(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into()).small().weak()
}

/// 档位表展开成 `0x00 0x43 0x45 0x48`，用于把配置里的表直接摊在界面上。
fn hex_table(table: &[u8]) -> String {
    table
        .iter()
        .map(|value| format!("0x{value:02x}"))
        .collect::<Vec<_>>()
        .join("  ")
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
        .width(crate::theme::ctrl_width())
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (candidate, label) in items {
                ui.selectable_value(value, *candidate, *label);
            }
        });
}

/// 一行「标签 + 输入框」。空串一律视为「不传这个参数」，由 cmdform 判空。
///
/// 标签走统一的左列、输入框从控件列起，`help` 挂在标签右侧的 `i` 上悬浮显示。
fn field(
    ui: &mut egui::Ui,
    p: &crate::theme::Palette,
    label: &str,
    value: &mut String,
    hint: &str,
    help: Option<&str>,
) -> egui::Response {
    theme::row(ui, p, label, help, |ui| theme::text_field(ui, value, hint))
}

/// 系统状态每一行的悬浮说明：字段名对用户不直观，停一下鼠标给出含义与排查方向。
fn state_help(name: &str) -> Option<&'static str> {
    Some(match name {
        "平台" => "当前跑在哪个平台，以及数字键与手势由谁处理。",
        "配置文件" => "驱动读取的配置文件路径；发现改不生效时先看这里指的是哪一份。",
        "机型" => "驱动识别出的华硕机型 ID，它决定了默认布局与键位。",
        "布局" => "数字键区当前使用的键位布局；认错机型时这里通常也跟着错。",
        "背光通道" => "灯不亮先看这一行：能否找到带 FEATURE 0x0d 的 HID 接口。",
        "数字键 / 手势" => "数字键与手势事件的实现方；Linux 下应为本程序的后台服务。",
        _ => return None,
    })
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
