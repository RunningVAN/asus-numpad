//! Linux 全套控制面板。
//!
//! 这张面板把驱动配置文件里的每一项都做成控件：机型、布局、键码风格、NumLock 联动、
//! 混合模式、手势阈值、背光通道、跟灯参数、时序参数、语言。改完点保存即落盘，
//! 「保存并重启服务」再多做一步 `systemctl --user restart`，让后台服务读到新配置。
//!
//! 做法是直接读改写 [`asus_numpad::config::Config`] 的 JSON，而不是拼命令行参数：
//! 命令行只覆盖了其中一部分（`sku` / `layout` / `lang` / `bright`），而阈值与通道这
//! 类参数只有配置文件一条路；界面里两者落在同一份文件上，不会出现「界面一套、CLI 一套」。

use eframe::egui;

use asus_numpad::config::{CalcMode, MixedMode, NumLockMode};
use asus_numpad::layout::{KeyMode, LayoutSupport, LAYOUTS};
use asus_numpad::sku::{self, SKUS};

use super::{field, small, Action, App, WARN};

/// 机型下拉里「自动识别」的取值（配置里 `sku: null`）。
const SKU_AUTO: &str = "";

impl App {
    /// Linux 侧的全部调节项。画完统一比对一次，有差异就标脏。
    pub(super) fn draw_full_controls(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let before = self.driver.cfg.clone();

        self.draw_cfg_header(ui, action);
        self.draw_identity(ui);
        self.draw_behavior(ui);
        self.draw_channel(ui, action);
        self.draw_gestures(ui);
        self.draw_timing(ui);
        self.draw_language(ui);
        self.draw_cfg_json(ui);
        self.draw_cfg_actions(ui, action);

        if self.driver.cfg != before {
            self.driver.dirty = true;
        }
    }

    fn draw_cfg_header(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("驱动配置");
            if self.driver.dirty {
                ui.label(egui::RichText::new("有改动未保存").small().color(WARN));
            }
        });
        ui.label(small(format!("写入：{}", self.driver.write_path.display())));
        if self.driver.path != self.driver.write_path {
            ui.label(small(format!("读取自：{}", self.driver.path.display())));
        }
        if !self.driver.existed {
            ui.label(small("磁盘上还没有配置文件，保存后按官方默认值创建。"));
        }
        for note in self.driver.notes.clone() {
            ui.label(egui::RichText::new(note).small().color(WARN));
        }

        ui.horizontal(|ui| {
            if ui.button("保存").clicked() {
                *action = Some(Action::SaveDriver);
            }
            if ui.button("保存并重启服务").clicked() {
                *action = Some(Action::SaveDriverAndRestart);
            }
            if ui.button("重新载入").clicked() {
                *action = Some(Action::ReloadDriver);
            }
            if ui.button("恢复默认").clicked() {
                *action = Some(Action::ResetDriver);
            }
        });
        ui.label(small(
            "只点保存不会打断正在用的触摸板；改机型/布局/阈值这类参数要点「保存并重启服务」才生效。",
        ));
    }

    /// 机型与布局：这两项决定键位怎么映射，放最前面。
    fn draw_identity(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("机型与布局")
            .default_open(true)
            .show(ui, |ui| {
                // ---- 机型 ----
                let mut manual = self.driver.cfg.sku.clone().unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label("机型");
                    let text = if manual.is_empty() {
                        "自动识别".to_string()
                    } else {
                        manual.clone()
                    };
                    egui::ComboBox::from_id_salt("cfg_sku")
                        .width(320.0)
                        .selected_text(text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut manual, SKU_AUTO.to_string(), "自动识别");
                            for row in SKUS {
                                ui.selectable_value(
                                    &mut manual,
                                    row.id.to_string(),
                                    format!("{} · {}", row.id, row.display),
                                );
                            }
                        });
                });
                let new_sku = if manual.trim().is_empty() {
                    None
                } else {
                    Some(manual.trim().to_string())
                };
                if new_sku != self.driver.cfg.sku {
                    self.driver.cfg.sku = new_sku;
                }

                match sku::current(self.driver.cfg.sku.as_deref()) {
                    Ok(resolved) => {
                        ui.label(small(format!(
                            "当前判定：{}（来源 {}）",
                            resolved.id(),
                            source_label(resolved.source)
                        )));
                        if let Some(detail) = &resolved.detail {
                            ui.label(small(format!("依据：{detail}")));
                        }
                        ui.label(small(resolved.sku.note));
                    }
                    Err(err) => {
                        ui.label(
                            egui::RichText::new(format!("当前判定：未识别（{err}）"))
                                .small()
                                .color(WARN),
                        );
                        ui.label(small(
                            "未识别不代表不能跑：手动选一台同模组的机型，或先看下面的系统状态里探测到的模组名。",
                        ));
                    }
                }

                // ---- 布局 ----
                ui.add_space(4.0);
                let mut layout = self.driver.cfg.layout_name.clone();
                ui.horizontal(|ui| {
                    ui.label("布局");
                    let text = if layout == "auto" || layout.is_empty() {
                        "自动（按 HID 模组）".to_string()
                    } else {
                        layout.clone()
                    };
                    egui::ComboBox::from_id_salt("cfg_layout")
                        .width(320.0)
                        .selected_text(text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut layout,
                                "auto".to_string(),
                                "自动（按 HID 模组）",
                            );
                            for entry in LAYOUTS {
                                ui.selectable_value(
                                    &mut layout,
                                    entry.name.to_string(),
                                    format!(
                                        "{} · {} 列 · {}",
                                        entry.name,
                                        entry.layout.columns,
                                        support_label(entry.support)
                                    ),
                                );
                            }
                        });
                });
                if layout != self.driver.cfg.layout_name {
                    self.driver.cfg.layout_name = layout;
                }

                if let Some(entry) = LAYOUTS
                    .iter()
                    .find(|e| e.name == self.driver.cfg.layout_name)
                {
                    ui.label(small(format!(
                        "{} 列 × 4 行，{} 项官方参数",
                        entry.layout.columns,
                        15
                    )));
                    if entry.support.needs_confirm() {
                        ui.label(
                            egui::RichText::new(format!(
                                "这套键表状态是 {}：未在真机上逐键核对，需要显式放行才会用。",
                                support_label(entry.support)
                            ))
                            .small()
                            .color(WARN),
                        );
                    }
                }
                ui.checkbox(
                    &mut self.driver.cfg.allow_unverified_layout,
                    "允许未核对的布局（family / unsupported 也放行）",
                );

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label("键码风格");
                    let mut mode = self.driver.cfg.key_mode;
                    egui::ComboBox::from_id_salt("cfg_key_mode")
                        .width(200.0)
                        .selected_text(match mode {
                            KeyMode::Keypad => "keypad（KEY_KP*）",
                            KeyMode::Digits => "digits（主键区 0-9）",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut mode,
                                KeyMode::Keypad,
                                "keypad（KEY_KP*）",
                            );
                            ui.selectable_value(
                                &mut mode,
                                KeyMode::Digits,
                                "digits（主键区 0-9）",
                            );
                        });
                    self.driver.cfg.key_mode = mode;
                });
                ui.label(small(
                    "keypad 对小键盘层有效；digits 让数字进主键区，某些游戏与输入法只认这一种。",
                ));
            });
    }

    fn draw_behavior(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("键盘行为")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("NumLock 联动");
                    egui::ComboBox::from_id_salt("cfg_numlock")
                        .width(320.0)
                        .selected_text(numlock_label(self.driver.cfg.numlock))
                        .show_ui(ui, |ui| {
                            for (mode, label) in [
                                (NumLockMode::Sync, "sync：关闭时补一次按键再点灯"),
                                (NumLockMode::Toggle, "toggle：无条件切换 NumLock"),
                                (NumLockMode::Off, "off：完全不碰 NumLock"),
                            ] {
                                ui.selectable_value(
                                    &mut self.driver.cfg.numlock,
                                    mode,
                                    label,
                                );
                            }
                        });
                });
                ui.label(small(
                    "驱动自己不模拟 NumLock 灯，这里决定的是「点数字键区时要不要顺手把系统 NumLock 打开」。",
                ));

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("混合模式");
                    egui::ComboBox::from_id_salt("cfg_mixed")
                        .width(320.0)
                        .selected_text(mixed_label(self.driver.cfg.mixed_mode))
                        .show_ui(ui, |ui| {
                            for (mode, label) in [
                                (MixedMode::Auto, "auto：有虚拟触摸板就开（官方默认）"),
                                (MixedMode::On, "on：强制开启"),
                                (MixedMode::Off, "off：纯数字键盘，光标不可用"),
                            ] {
                                ui.selectable_value(
                                    &mut self.driver.cfg.mixed_mode,
                                    mode,
                                    label,
                                );
                            }
                        });
                });
                ui.label(small(
                    "关闭混合模式后触摸板只出数字、没有光标，等同官方 dwSupport_MixedMode = 0。",
                ));

                ui.add_space(4.0);
                ui.checkbox(&mut self.driver.cfg.calc_gesture, "计算器手势（长按左上图标区）");
                if self.driver.cfg.calc_gesture {
                    ui.horizontal(|ui| {
                        ui.label("手势模式");
                        egui::ComboBox::from_id_salt("cfg_calc_mode")
                            .width(200.0)
                            .selected_text(match self.driver.cfg.calc_gesture_mode {
                                CalcMode::Long => "long：长按触发",
                                CalcMode::Off => "off：关闭",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.driver.cfg.calc_gesture_mode,
                                    CalcMode::Long,
                                    "long：长按触发",
                                );
                                ui.selectable_value(
                                    &mut self.driver.cfg.calc_gesture_mode,
                                    CalcMode::Off,
                                    "off：关闭",
                                );
                            });
                    });
                }

                ui.add_space(4.0);
                ui.checkbox(
                    &mut self.driver.cfg.grab,
                    "独占触摸板（关掉后系统手势也能同时收到事件）",
                );
            });
    }

    /// 背光通道：灯不亮时第一个要看的地方。
    fn draw_channel(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        egui::CollapsingHeader::new("背光通道与跟灯")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("通道");
                    egui::ComboBox::from_id_salt("cfg_led_kind")
                        .width(240.0)
                        .selected_text(self.driver.cfg.led_kind.clone())
                        .show_ui(ui, |ui| {
                            for kind in ["auto", "hidraw", "i2c", "none"] {
                                ui.selectable_value(
                                    &mut self.driver.cfg.led_kind,
                                    kind.to_string(),
                                    match kind {
                                        "auto" => "auto：按探测结果挑",
                                        "hidraw" => "hidraw：FEATURE 0x0d",
                                        "i2c" => "i2c：直接写 EC 寄存器",
                                        _ => "none：不控制背光",
                                    },
                                );
                            }
                        });
                });

                let mut hidraw = self.driver.cfg.hidraw.clone().unwrap_or_default();
                if field(ui, "hidraw 设备", &mut hidraw, "留空 = 自动挑选").changed() {
                    let text = hidraw.trim().to_string();
                    self.driver.cfg.hidraw = if text.is_empty() { None } else { Some(text) };
                }

                ui.horizontal(|ui| {
                    ui.label("i2c 总线");
                    let mut auto_bus = self.driver.cfg.i2c_bus.is_none();
                    if ui.checkbox(&mut auto_bus, "自动").changed() {
                        self.driver.cfg.i2c_bus = if auto_bus { None } else { Some(0) };
                    }
                    if let Some(bus) = self.driver.cfg.i2c_bus.as_mut() {
                        ui.add(egui::DragValue::new(bus).range(0..=99));
                    }
                    ui.label("地址");
                    let mut auto_addr = self.driver.cfg.i2c_addr.is_none();
                    if ui.checkbox(&mut auto_addr, "自动").changed() {
                        self.driver.cfg.i2c_addr = if auto_addr { None } else { Some(0x38) };
                    }
                    if let Some(addr) = self.driver.cfg.i2c_addr.as_mut() {
                        ui.add(
                            egui::DragValue::new(addr)
                                .range(0..=0x7f)
                                .custom_formatter(|v, _| format!("0x{:02x}", v as u8)),
                        );
                    }
                });
                let mut payload = self.driver.cfg.i2c_payload.clone();
                if field(ui, "i2c 载荷首字节", &mut payload, "0x00").changed() {
                    self.driver.cfg.i2c_payload = payload.trim().to_string();
                }
                let mut transfer = self.driver.cfg.i2c_transfer.clone();
                if field(ui, "i2c 工具", &mut transfer, "i2ctransfer").changed() {
                    self.driver.cfg.i2c_transfer = transfer.trim().to_string();
                }
                ui.label(small(
                    "条件是 hidraw 上没有 FEATURE 0x0d（少数模组被官方驱动占掉了报告）。i2c 只在 Linux 有效。",
                ));

                ui.add_space(6.0);
                ui.checkbox(
                    &mut self.driver.cfg.follow_keyboard_brightness,
                    "跟随键盘背光（默认）",
                );
                if self.driver.cfg.follow_keyboard_brightness {
                    let mut dir = self.driver.cfg.keyboard_brightness_dir.clone();
                    if field(ui, "键盘背光目录", &mut dir, "/sys/class/leds/…").changed() {
                        self.driver.cfg.keyboard_brightness_dir = dir.trim().to_string();
                    }
                    ui.horizontal(|ui| {
                        ui.label("轮询间隔");
                        ui.add(
                            egui::DragValue::new(&mut self.driver.cfg.brightness_poll_ms)
                                .range(50..=5000)
                                .suffix(" ms"),
                        );
                    });
                } else {
                    ui.label(small("不跟灯时用固定档位，见下面的手动档位表。"));
                }

                ui.add_space(6.0);
                ui.label("手动档位表");
                self.draw_manual_levels(ui, action);
            });
    }

    /// `manual_brightness_levels` 的编辑器：每档一个命令字，增删档都直接改表长。
    fn draw_manual_levels(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let mut levels = self.driver.cfg.manual_brightness_levels.clone();
        let explicit = levels.is_some();
        let mut use_explicit = explicit;
        if ui
            .checkbox(&mut use_explicit, "显式给出档位表（不勾则由驱动线性等分）")
            .changed()
        {
            levels = if use_explicit {
                Some(vec![0x00, 0x43, 0x45, 0x48])
            } else {
                None
            };
        }

        if let Some(table) = levels.as_mut() {
            ui.horizontal_wrapped(|ui| {
                for (i, value) in table.iter_mut().enumerate() {
                    ui.label(format!("{}.", i + 1));
                    ui.add(
                        egui::DragValue::new(value)
                            .range(0x00..=0x48)
                            .custom_formatter(|v, _| format!("0x{:02x}", v as u8)),
                    );
                }
            });
            ui.horizontal(|ui| {
                if ui.button("加一档").clicked() && table.len() < 8 {
                    let last = table.last().copied().unwrap_or(0x41);
                    table.push((last + 1).min(0x48));
                }
                if ui.button("减一档").clicked() && table.len() > 1 {
                    table.pop();
                }
                if ui.button("改回线性四档").clicked() {
                    *table = vec![0x00, 0x43, 0x45, 0x48];
                }
            });
            ui.label(small(
                "取值只有 0x00（灭）与 0x41..=0x48 是硬件真实档位，写别的数不会更亮。表长即档数（1..=8）。",
            ));
            ui.horizontal_wrapped(|ui| {
                // 日常按键盘四档调亮度在灯光页做，这里只负责和驱动配置对齐，避免两处各改一套。
                ui.label(small("要按键盘四档调就走灯光页，两边对齐用："));
                if ui.button("用灯光页四档填充").clicked() {
                    *action = Some(Action::SyncLevelsToDriver);
                }
                if ui.button("从驱动读回灯光页").clicked() {
                    *action = Some(Action::LoadLevelsFromDriver);
                }
            });
        }

        if levels != self.driver.cfg.manual_brightness_levels {
            self.driver.cfg.manual_brightness_levels = levels;
        }

        ui.horizontal(|ui| {
            ui.label("固定档位序号");
            ui.add(egui::DragValue::new(&mut self.driver.cfg.manual_brightness_step).range(1..=8));
            ui.label(small("不跟灯时亮到第几档"));
        });
    }

    fn draw_gestures(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("手势与阈值（改错会影响手感）")
            .default_open(false)
            .show(ui, |ui| {
                let rows: [(&str, &mut f64, &str); 5] = [
                    (
                        "应用手势触发距离",
                        &mut self.driver.cfg.app_gesture_trig_um,
                        "μm",
                    ),
                    ("手势触发距离", &mut self.driver.cfg.gesture_trig_um, "μm"),
                    ("轻触阈值", &mut self.driver.cfg.tap_threshold_mm, "mm"),
                    (
                        "双指滚动距离",
                        &mut self.driver.cfg.scroll_finger_dist_mm,
                        "mm",
                    ),
                    (
                        "连击最小间隔",
                        &mut self.driver.cfg.tap_interval_min_ms,
                        "ms",
                    ),
                ];
                for (label, value, unit) in rows {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.add(egui::DragValue::new(value).speed(1.0));
                        ui.label(unit);
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("触摸板尺寸");
                    ui.add(egui::DragValue::new(&mut self.driver.cfg.touchpad_width_mm));
                    ui.label("×");
                    ui.add(egui::DragValue::new(
                        &mut self.driver.cfg.touchpad_height_mm,
                    ));
                    ui.label("mm");
                });
            });
    }

    fn draw_timing(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("时序参数（固件帧计数，一般不用动）")
            .default_open(false)
            .show(ui, |ui| {
                let rows: [(&str, &mut u32); 6] = [
                    ("帧间隔", &mut self.driver.cfg.frame_ms),
                    (
                        "NumLock 触发帧",
                        &mut self.driver.cfg.numlock_trigger_frames,
                    ),
                    (
                        "SENDKEY 连续触发帧",
                        &mut self.driver.cfg.cont_sendkey_trigger_frames,
                    ),
                    ("图标区长按帧", &mut self.driver.cfg.numadv_trigger_frames),
                    ("按键保持帧", &mut self.driver.cfg.touch_sendkey_frames),
                    ("最长工作时间", &mut self.driver.cfg.max_work_time_ms),
                ];
                for (label, value) in rows {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.add(egui::DragValue::new(value));
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("SENDKEY 判定比例");
                    ui.add(egui::DragValue::new(
                        &mut self.driver.cfg.touch_sendkey_ratio,
                    ));
                });
            });
    }

    fn draw_language(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("驱动语言")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("语言");
                    egui::ComboBox::from_id_salt("cfg_lang")
                        .width(240.0)
                        .selected_text(lang_label(&self.driver.cfg.language))
                        .show_ui(ui, |ui| {
                            for code in ["auto", "zh-CN", "zh-TW", "en"] {
                                ui.selectable_value(
                                    &mut self.driver.cfg.language,
                                    code.to_string(),
                                    lang_label(code),
                                );
                            }
                        });
                });
                ui.label(small(
                    "驱动与 CLI 的输出文案跟着这一项走，界面自身语言不受影响。",
                ));
            });
    }

    fn draw_cfg_json(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("配置文件内容（JSON）")
            .default_open(false)
            .show(ui, |ui| {
                let text = self.driver.json();
                ui.horizontal(|ui| {
                    if ui.button("复制").clicked() {
                        ui.ctx().copy_text(text.clone());
                    }
                    ui.label(small("与 asus-numpad --dump-config 输出一致"));
                });
                let mut shown = text;
                ui.add(
                    egui::TextEdit::multiline(&mut shown)
                        .code_editor()
                        .desired_rows(14)
                        .desired_width(f32::INFINITY),
                );
            });
    }

    fn draw_cfg_actions(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("保存配置").clicked() {
                *action = Some(Action::SaveDriver);
            }
            if ui.button("保存并重启服务").clicked() {
                *action = Some(Action::SaveDriverAndRestart);
            }
        });
    }
}

fn support_label(support: LayoutSupport) -> &'static str {
    match support {
        LayoutSupport::Verified => "真机核对",
        LayoutSupport::Family => "同族未核对",
        LayoutSupport::Unsupported => "未核对、默认不放行",
    }
}

fn source_label(source: sku::SkuSource) -> &'static str {
    match source {
        sku::SkuSource::Manual => "手动指定",
        sku::SkuSource::Dmi => "DMI 机型",
        sku::SkuSource::Hardware => "触摸板模组",
    }
}

fn numlock_label(mode: NumLockMode) -> &'static str {
    match mode {
        NumLockMode::Sync => "sync：关闭时补一次按键再点灯",
        NumLockMode::Toggle => "toggle：无条件切换 NumLock",
        NumLockMode::Off => "off：完全不碰 NumLock",
    }
}

fn mixed_label(mode: MixedMode) -> &'static str {
    match mode {
        MixedMode::Auto => "auto：有虚拟触摸板就开（官方默认）",
        MixedMode::On => "on：强制开启",
        MixedMode::Off => "off：纯数字键盘",
    }
}

fn lang_label(code: &str) -> &'static str {
    match code {
        "auto" => "auto：跟随系统 locale",
        "zh-CN" => "zh-CN 简体中文",
        "zh-TW" => "zh-TW 繁體中文",
        "en" => "en English",
        _ => "auto：跟随系统 locale",
    }
}
