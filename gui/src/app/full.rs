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
use crate::theme;

/// 机型下拉里「自动识别」的取值（配置里 `sku: null`）。
const SKU_AUTO: &str = "";

impl App {
    /// Linux 侧的全部调节项。画完统一比对一次，有差异就标脏。
    pub(super) fn draw_full_controls(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let before = self.driver.cfg.clone();

        self.draw_cfg_header(ui, action);
        ui.add_space(10.0);
        self.draw_identity(ui);
        ui.add_space(10.0);
        self.draw_behavior(ui);
        ui.add_space(10.0);
        self.draw_channel(ui, action);
        ui.add_space(10.0);
        self.draw_gestures(ui);
        ui.add_space(10.0);
        self.draw_timing(ui);
        ui.add_space(10.0);
        self.draw_language(ui);
        ui.add_space(10.0);
        self.draw_cfg_json(ui);
        ui.add_space(10.0);
        self.draw_cfg_actions(ui, action);

        if self.driver.cfg != before {
            self.driver.dirty = true;
        }
    }

    /// 驱动配置页的抬头：写在哪、有没有改动、以及四个落盘动作。
    ///
    /// 四个按钮按「动词轻重」排：主按钮是保存并重启（改完参数的常规动作），
    /// 只保存和重新载入是次级，恢复默认放最后且用次级样式，避免误点。
    fn draw_cfg_header(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let dirty = self.driver.dirty;
        let write_path = self.driver.write_path.display().to_string();
        let read_path = self.driver.path.display().to_string();
        let same_path = self.driver.path == self.driver.write_path;
        let existed = self.driver.existed;
        let notes = self.driver.notes.clone();

        theme::card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                theme::title_row(ui, &p, "驱动配置");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if dirty {
                        theme::pill(ui, "有改动未保存", p.warn)
                            .on_hover_text("点「保存」或「保存并重启服务」落盘");
                    } else {
                        theme::pill(ui, "已与磁盘一致", p.ok);
                    }
                });
            });
            ui.add_space(2.0);
            theme::note(ui, &p, format!("写入：{write_path}"));
            if !same_path {
                theme::note(ui, &p, format!("读取自：{read_path}"));
            }
            if !existed {
                theme::note(ui, &p, "磁盘上还没有配置文件，保存后按官方默认值创建。");
            }
            for note in notes {
                ui.label(egui::RichText::new(note).small().color(p.warn));
            }

            ui.add_space(8.0);
            // 四颗按钮各自统一宽度，从控件列起排；右边缘不会因为文字长短而参差
            let picked = theme::action_row(
                ui,
                &p,
                "写入",
                Some("保存＝只落盘；保存并重启＝写回配置文件后 systemctl --user restart。"),
                &[
                    (
                        "保存并重启",
                        true,
                        "写回配置文件并 systemctl --user restart，改机型/布局/阈值后必须走这个。",
                    ),
                    (
                        "保存",
                        false,
                        "只落盘，不重启后台服务；正在用的触摸板不中断。",
                    ),
                    (
                        "重新载入",
                        false,
                        "丢掉界面上的改动，重新从磁盘读一遍配置。",
                    ),
                    (
                        "恢复默认",
                        false,
                        "把驱动配置改回官方默认值；需要再保存才落盘。",
                    ),
                ],
            );
            match picked {
                Some(0) => *action = Some(Action::SaveDriverAndRestart),
                Some(1) => *action = Some(Action::SaveDriver),
                Some(2) => *action = Some(Action::ReloadDriver),
                Some(3) => *action = Some(Action::ResetDriver),
                _ => {}
            }
            theme::note(
                ui,
                &p,
                "只点保存不会打断正在用的触摸板；改机型/布局/阈值这类参数要点「保存并重启服务」才生效。",
            );
        });
    }

    /// 机型与布局：这两项决定键位怎么映射，放最前面。
    fn draw_identity(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("机型与布局")
            .default_open(true)
            .show(ui, |ui| {
                // ---- 机型 ----
                let mut manual = self.driver.cfg.sku.clone().unwrap_or_default();
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "机型",
                        Some("驱动认哪台机器：自动识别按 HID 模组判定。认错会让键位、背光档位一起错，先看下面的判定依据。"),
                    );
                    let text = if manual.is_empty() {
                        "自动识别".to_string()
                    } else {
                        manual.clone()
                    };
                    egui::ComboBox::from_id_salt("cfg_sku")
                        .width(theme::ctrl_width())
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
                        theme::note(ui, &p, format!(
                            "当前判定：{}（来源 {}）",
                            resolved.id(),
                            source_label(resolved.source)
                        ));
                        if let Some(detail) = &resolved.detail {
                            theme::note(ui, &p, format!("依据：{detail}"));
                        }
                        theme::note(ui, &p, resolved.sku.note);
                    }
                    Err(err) => {
                        ui.label(
                            egui::RichText::new(format!("当前判定：未识别（{err}）"))
                                .small()
                                .color(WARN),
                        );
                        theme::note(ui, &p, "未识别不代表不能跑：手动选一台同模组的机型，或先看下面的系统状态里探测到的模组名。",);
                    }
                }

                // ---- 布局 ----
                ui.add_space(4.0);
                let mut layout = self.driver.cfg.layout_name.clone();
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "布局",
                        Some("数字键区的键表。auto 按 HID 模组挑；列数决定键区宽度，认错时按 7 会串到 8。"),
                    );
                    let text = if layout == "auto" || layout.is_empty() {
                        "自动（按 HID 模组）".to_string()
                    } else {
                        layout.clone()
                    };
                    egui::ComboBox::from_id_salt("cfg_layout")
                        .width(theme::ctrl_width())
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
                    theme::note(ui, &p, format!(
                        "{} 列 × 4 行，{} 项官方参数",
                        entry.layout.columns,
                        15
                    ));
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
                theme::check_row(
                    ui,
                    &p,
                    &mut self.driver.cfg.allow_unverified_layout,
                    "允许未核对布局",
                    Some("family / unsupported 的键表也放行：能用，但没在真机上逐键核对过。"),
                );

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "键码风格",
                        Some("keypad 发 KEY_KP* 小键盘码；digits 发主键区 0-9——某些游戏和输入法只认后者。"),
                    );
                    let mut mode = self.driver.cfg.key_mode;
                    egui::ComboBox::from_id_salt("cfg_key_mode")
                        .width(theme::ctrl_width())
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
                theme::note(
                    ui,
                    &p,
                    "keypad 对小键盘层有效；digits 让数字进主键区，某些游戏与输入法只认这一种。",
                );
            });
        });
    }

    fn draw_behavior(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("键盘行为")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "NumLock 联动",
                        Some("点数字键区时怎么处理系统 NumLock：sync 先补一次按键、toggle 无条件切换、off 完全不碰。"),
                    );
                    egui::ComboBox::from_id_salt("cfg_numlock")
                        .width(theme::ctrl_width())
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
                theme::note(
                    ui,
                    &p,
                    "驱动自己不模拟 NumLock 灯，这里决定的是「点数字键区时要不要顺手把系统 NumLock 打开」。",
                );

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "混合模式",
                        Some("混排模式：auto 检测到虚拟触摸板才开，force 总是开，off 就只剩纯数字键盘。"),
                    );
                    egui::ComboBox::from_id_salt("cfg_mixed")
                        .width(theme::ctrl_width())
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
                theme::note(ui, &p, "关闭混合模式后触摸板只出数字、没有光标，等同官方 dwSupport_MixedMode = 0。",);

                ui.add_space(4.0);
                theme::check_row(
                    ui,
                    &p,
                    &mut self.driver.cfg.calc_gesture,
                    "计算器手势（长按左上图标区）",
                    Some("长按图标区弹出计算器；模组不支持时勾了也不生效。"),
                );
                if self.driver.cfg.calc_gesture {
                    ui.horizontal(|ui| {
                        theme::label_cell(
                            ui,
                            &p,
                            "手势模式",
                            Some("触摸手势的触发方式，取决于这套模组是否支持；改了之后要点「保存并重启服务」。"),
                        );
                        egui::ComboBox::from_id_salt("cfg_calc_mode")
                            .width(theme::ctrl_width())
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
                theme::check_row(
                    ui,
                    &p,
                    &mut self.driver.cfg.grab,
                    "独占触摸板（关掉后系统手势也能同时收到事件）",
                    Some("独占后系统手势拿不到事件；想同时用系统手势就取消勾选。"),
                );
            });
        });
    }

    /// 背光通道：灯不亮时第一个要看的地方。
    fn draw_channel(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("背光通道与跟灯")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "通道",
                        Some("背光报文走哪条链路：自动挑选，或强制 hidraw / i2c。灯不亮时先在这里换一条试。"),
                    );
                    egui::ComboBox::from_id_salt("cfg_led_kind")
                        .width(theme::ctrl_width())
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
                if field(
                    ui,
                    &p,
                    "hidraw 设备",
                    &mut hidraw,
                    "留空 = 自动挑选",
                    Some("指定 hidraw 节点名（如 hidraw3）；留空就按机型自动挑带 FEATURE 0x0d 的那个。"),
                )
                .changed()
                {
                    let text = hidraw.trim().to_string();
                    self.driver.cfg.hidraw = if text.is_empty() { None } else { Some(text) };
                }

                theme::row(
                    ui,
                    &p,
                    "i2c 总线",
                    Some("走 i2c 通道时的总线号，一般配合下面的从机地址一起用。"),
                    |ui| {
                        // 勾选框与数字框并排，两段加起来正好是整列宽，右边缘与别的行齐平
                        let cfg_bus = self.driver.cfg.i2c_bus;
                        let mut auto_bus = cfg_bus.is_none();
                        let mut bus = cfg_bus.unwrap_or(0);
                        let manual_bus = !auto_bus;
                        let (auto_resp, bus_resp) = theme::pair(
                            ui,
                            |ui| {
                                let resp = theme::switch(ui, &p, &mut auto_bus);
                                ui.label(egui::RichText::new("自动").size(12.5).color(p.text_weak));
                                resp
                            },
                            |ui| {
                                ui.add_enabled_ui(manual_bus, |ui| {
                                    theme::pair_num(ui, &mut bus, 0..=99)
                                })
                                .inner
                            },
                        );
                        if auto_resp.changed() {
                            self.driver.cfg.i2c_bus = if auto_bus { None } else { Some(bus) };
                        } else if bus_resp.changed() {
                            self.driver.cfg.i2c_bus = Some(bus);
                        }
                    },
                );
                theme::row(
                    ui,
                    &p,
                    "i2c 地址",
                    Some("走 i2c 通道时的从机地址，触摸板通常在 0x15 附近。"),
                    |ui| {
                        let cfg_addr = self.driver.cfg.i2c_addr;
                        let mut auto_addr = cfg_addr.is_none();
                        let mut addr = cfg_addr.unwrap_or(0x38);
                        let manual_addr = !auto_addr;
                        let (auto_resp, addr_resp) = theme::pair(
                            ui,
                            |ui| {
                                let resp = theme::switch(ui, &p, &mut auto_addr);
                                ui.label(egui::RichText::new("自动").size(12.5).color(p.text_weak));
                                resp
                            },
                            |ui| {
                                ui.add_enabled_ui(manual_addr, |ui| {
                                    ui.add_sized(
                                        egui::Vec2::new(theme::pair_b(), theme::CTRL_H),
                                        egui::DragValue::new(&mut addr)
                                            .range(0..=0x7f)
                                            .custom_formatter(|v, _| format!("0x{:02x}", v as u8)),
                                    )
                                })
                                .inner
                            },
                        );
                        if auto_resp.changed() {
                            self.driver.cfg.i2c_addr = if auto_addr { None } else { Some(addr) };
                        } else if addr_resp.changed() {
                            self.driver.cfg.i2c_addr = Some(addr);
                        }
                    },
                );
                let mut payload = self.driver.cfg.i2c_payload.clone();
                if field(
                    ui,
                    &p,
                    "i2c 载荷首字节",
                    &mut payload,
                    "0x00",
                    Some("走 i2c 时每帧的第一个字节，官方默认 0x00。"),
                )
                .changed()
                {
                    self.driver.cfg.i2c_payload = payload.trim().to_string();
                }
                let mut transfer = self.driver.cfg.i2c_transfer.clone();
                if field(
                    ui,
                    &p,
                    "i2c 工具",
                    &mut transfer,
                    "i2ctransfer",
                    Some("用来发 i2c 帧的外部命令，默认 i2ctransfer。"),
                )
                .changed()
                {
                    self.driver.cfg.i2c_transfer = transfer.trim().to_string();
                }
                theme::note(
                    ui,
                    &p,
                    "条件是 hidraw 上没有 FEATURE 0x0d（少数模组被官方驱动占掉了报告）。i2c 只在 Linux 有效。",
                );

                ui.add_space(6.0);
                theme::check_row(
                    ui,
                    &p,
                    &mut self.driver.cfg.follow_keyboard_brightness,
                    "跟随键盘背光",
                    Some("默认开启：键盘背光变几档，触摸板跟着变几档。关掉后各走各的。"),
                );
                if self.driver.cfg.follow_keyboard_brightness {
                    let mut dir = self.driver.cfg.keyboard_brightness_dir.clone();
                    if field(
                        ui,
                        &p,
                        "键盘背光目录",
                        &mut dir,
                        "/sys/class/leds/…",
                        Some("从哪个 leds 节点读键盘背光档位；机型不同节点名也不同。"),
                    )
                    .changed()
                    {
                        self.driver.cfg.keyboard_brightness_dir = dir.trim().to_string();
                    }
                    theme::row(
                        ui,
                        &p,
                        "轮询间隔",
                        Some("多久读一次键盘背光状态。调小跟得更紧，但每秒读的次数也更多。"),
                        |ui| {
                            ui.add_sized(
                                [theme::ctrl_width(), theme::CTRL_H],
                                egui::DragValue::new(&mut self.driver.cfg.brightness_poll_ms)
                                    .range(50..=5000)
                                    .suffix(" ms"),
                            );
                        },
                    );
                } else {
                    ui.label(small("不跟灯时用固定档位，见下面的手动档位表。"));
                }

                ui.add_space(6.0);
                theme::label_cell(
                    ui,
                    &p,
                    "手动档位表",
                    Some("自己填每档对应的命令字；相邻档必须单调，否则 CLI 会拒绝。"),
                );
                self.draw_manual_levels(ui, action);
            });
        });
    }

    /// `manual_brightness_levels` 的编辑器：每档一个命令字，增删档都直接改表长。
    fn draw_manual_levels(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let mut levels = self.driver.cfg.manual_brightness_levels.clone();
        let explicit = levels.is_some();
        let mut use_explicit = explicit;
        if theme::check_row(
            ui,
            &p,
            &mut use_explicit,
            "显式给出档位表（不勾则由驱动线性等分）",
            Some("勾上就自己填每档的命令字；不勾则由驱动按硬件档位线性等分。"),
        )
        .changed()
        {
            levels = if use_explicit {
                Some(vec![0x00, 0x43, 0x45, 0x48])
            } else {
                None
            };
        }

        if let Some(table) = levels.as_mut() {
            // 四格一行铺满控件列：档数变化时换行位置固定，第二行的格子与第一行对着
            theme::block(
                ui,
                &p,
                "档位表",
                Some("按 1、2、3… 的顺序每档填一个命令字，相邻档必须单调。"),
                |ui| theme::slot_grid(ui, &p, table, 0x00, 0x48),
            );
            let picked = theme::action_row(
                ui,
                &p,
                "增删档",
                Some("表长即档数（1..=8）；只有 0x00（灭）与 0x41..=0x48 是硬件真实档位，写别的数不会更亮。"),
                &[
                    ("加一档", false, "在表尾接一档，命令字接上一档往后排。"),
                    ("减一档", false, "删掉表尾一档。"),
                    ("线性四档", false, "重置成 0x00 / 0x43 / 0x45 / 0x48 四档。"),
                ],
            );
            match picked {
                Some(0) if table.len() < 8 => {
                    let last = table.last().copied().unwrap_or(0x41);
                    table.push((last + 1).min(0x48));
                }
                Some(1) if table.len() > 1 => {
                    table.pop();
                }
                Some(2) => *table = vec![0x00, 0x43, 0x45, 0x48],
                _ => {}
            }
            // 日常按键盘四档调亮度在灯光页做，这里只负责和驱动配置对齐，避免两处各改一套。
            let picked = theme::action_row(
                ui,
                &p,
                "与灯页对齐",
                Some("按键盘四档调亮度在灯光页做；这里只负责把两边档位表对齐。"),
                &[
                    (
                        "用四档填充",
                        false,
                        "把灯光页的四档映射写进这份驱动档位表。",
                    ),
                    (
                        "从驱动读回",
                        false,
                        "反过来：用驱动配置里的档位表覆盖界面。",
                    ),
                ],
            );
            match picked {
                Some(0) => *action = Some(Action::SyncLevelsToDriver),
                Some(1) => *action = Some(Action::LoadLevelsFromDriver),
                _ => {}
            }
        }

        if levels != self.driver.cfg.manual_brightness_levels {
            self.driver.cfg.manual_brightness_levels = levels;
        }

        theme::row_hint(
            ui,
            &p,
            "固定档位序号",
            Some("固定停在这一档，不再跟随键盘背光。"),
            "不跟灯时固定亮到第几档（1..8）",
            |ui| {
                ui.add_sized(
                    [theme::ctrl_width(), theme::CTRL_H],
                    egui::DragValue::new(&mut self.driver.cfg.manual_brightness_step).range(1..=8),
                );
            },
        );
    }

    fn draw_gestures(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
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
                        theme::row(ui, &p, label, None, |ui| {
                            ui.add_sized(
                                [theme::ctrl_width(), theme::CTRL_H],
                                egui::DragValue::new(value)
                                    .speed(1.0)
                                    .suffix(format!(" {unit}")),
                            );
                        });
                    }
                    theme::row(
                        ui,
                        &p,
                        "触摸板尺寸",
                        Some("触摸板物理尺寸（mm），手势阈值按它换算。"),
                        |ui| {
                            // 两个数字框 + 中间的「×」与单位「mm」，加起来正好一列宽
                            let side = (theme::ctrl_width() - 52.0) / 2.0;
                            let dim = egui::Vec2::new(side, theme::CTRL_H);
                            let res = ui.add_sized(
                                dim,
                                egui::DragValue::new(&mut self.driver.cfg.touchpad_width_mm),
                            );
                            theme::tip(res, "触摸板宽（mm），手势阈值按它换算。");
                            ui.label(egui::RichText::new("×").color(p.text_weak));
                            let res = ui.add_sized(
                                dim,
                                egui::DragValue::new(&mut self.driver.cfg.touchpad_height_mm),
                            );
                            theme::tip(res, "触摸板高（mm），手势阈值按它换算。");
                            ui.label(egui::RichText::new("mm").color(p.text_weak));
                        },
                    );
                });
        });
    }

    fn draw_timing(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
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
                        theme::row(ui, &p, label, None, |ui| {
                            ui.add_sized(
                                [theme::ctrl_width(), theme::CTRL_H],
                                egui::DragValue::new(value),
                            );
                        });
                    }
                    theme::row(
                        ui,
                        &p,
                        "SENDKEY 判定比例",
                        Some("手势里判定为 SENDKEY 的比例阈值，越小越灵敏。"),
                        |ui| {
                            ui.add_sized(
                                [theme::ctrl_width(), theme::CTRL_H],
                                egui::DragValue::new(&mut self.driver.cfg.touch_sendkey_ratio),
                            );
                        },
                    );
                });
        });
    }

    fn draw_language(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("驱动语言")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        theme::label_cell(
                            ui,
                            &p,
                            "语言",
                            Some("界面与日志的语言，auto 跟系统语言走。"),
                        );
                        egui::ComboBox::from_id_salt("cfg_lang")
                            .width(theme::ctrl_width())
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
                    theme::note(
                        ui,
                        &p,
                        "驱动与 CLI 的输出文案跟着这一项走，界面自身语言不受影响。",
                    );
                });
        });
    }

    fn draw_cfg_json(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("配置文件内容（JSON）")
                .default_open(false)
                .show(ui, |ui| {
                    let text = self.driver.json();
                    ui.horizontal(|ui| {
                        if theme::secondary(ui, &p, "复制").clicked() {
                            ui.ctx().copy_text(text.clone());
                        }
                        theme::note(ui, &p, "与 asus-numpad --dump-config 输出一致");
                    });
                    let mut shown = text;
                    ui.add(
                        egui::TextEdit::multiline(&mut shown)
                            .code_editor()
                            .desired_rows(14)
                            .desired_width(f32::INFINITY),
                    );
                });
        });
    }

    fn draw_cfg_actions(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            ui.add_space(6.0);
            let picked = theme::action_row(
                ui,
                &p,
                "收尾",
                Some("和顶部的两颗保存按钮等价，滚到页底时不用再滚回去。"),
                &[
                    ("保存配置", false, "只落盘，不重启后台服务。"),
                    (
                        "保存并重启",
                        true,
                        "写回配置文件并重启后台服务，让新配置生效。",
                    ),
                ],
            );
            match picked {
                Some(0) => *action = Some(Action::SaveDriver),
                Some(1) => *action = Some(Action::SaveDriverAndRestart),
                _ => {}
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
