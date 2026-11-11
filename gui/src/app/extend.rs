//! 扩展页：自定义布局、自定义点亮报文、压感切换、全局热键。
//!
//! 这一页专门管「官方表还没覆盖到的机型」：贴纸 / 镭雕方案自己写键表，背光报文不同的
//! 机型自己填模板，带压感的触摸板用力度切开关，键盘上再绑一个组合键当兜底。
//!
//! 四项互不依赖，改哪一项都不影响其它项：布局只影响键位，报文只影响点亮，压感与热键
//! 只是两种「不用伸手去摸触摸板」的开关方式——原生机型也可以只用其中一项。
//!
//! 界面里改的全是驱动配置（`asus-numpad.json`）里的 `custom_layouts` / `led_proto` /
//! `pressure_switch` / `hotkeys`，保存走驱动配置页同一套落盘逻辑，不会多出第二份文件。

use eframe::egui;

use asus_numpad::custom::{self, CustomLayoutSpec, HotkeyAction, HotkeySpec, LedProtoSpec};
use asus_numpad::layout::LAYOUTS;

use super::{small, Action, App, WARN};
use crate::theme;

/// 键表固定 4 行，与 `Geometry` 的网格行数一致。
const KEY_ROWS: usize = 4;

impl App {
    /// 扩展页整体。
    ///
    /// 画完统一比对一次配置，有差异就标脏——与驱动配置页同一套「改了没保存」提示。
    pub(super) fn draw_extension(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let before = self.driver.cfg.clone();

        self.draw_ext_header(ui, action);
        ui.add_space(10.0);
        self.draw_ext_layouts(ui);
        ui.add_space(10.0);
        self.draw_ext_proto(ui);
        ui.add_space(10.0);
        self.draw_ext_pressure(ui);
        ui.add_space(10.0);
        self.draw_ext_hotkeys(ui);

        if self.driver.cfg != before {
            self.driver.dirty = true;
        }
    }

    /// 扩展页抬头：写完存哪、有没有改动、落盘与自检动作。
    ///
    /// 与驱动配置页同一套排法——保存类动作常驻页首。原来这张卡压在最底下，四张配置卡
    /// 一铺满就得滚到页尾才找得到「保存」，改完参数的人以为没地方存。
    fn draw_ext_header(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let dirty = self.driver.dirty;
        let write_path = self.driver.write_path.display().to_string();

        theme::card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                theme::title_row_help(
                    ui,
                    &p,
                    "扩展",
                    Some(
                        "四项扩展与驱动共用同一个配置文件；改完要保存，键表与新报文还要重启后台服务才生效。",
                    ),
                );
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

            ui.add_space(8.0);
            let picked = theme::action_row(
                ui,
                &p,
                "写入",
                Some("保存＝只落盘；保存并重启＝写回配置后 systemctl --user restart。"),
                &[
                    (
                        "保存并重启服务",
                        true,
                        "写回配置文件并 systemctl --user restart；换键表、换点亮报文后必须走这个。",
                    ),
                    (
                        "保存",
                        false,
                        "只落盘，不重启后台服务；正在用的触摸板不中断。",
                    ),
                    (
                        "校验扩展项",
                        false,
                        "在副本上跑一遍解析：布局能不能装、HID 有没有和内置表打架、报文模板合不合法、压感阈值会不会被夹、热键解析得出来吗。不动磁盘也不动设备。",
                    ),
                ],
            );
            match picked {
                Some(0) => *action = Some(Action::SaveDriverAndRestart),
                Some(1) => *action = Some(Action::SaveDriver),
                Some(2) => {
                    // 在副本上跑一遍真正的 sanitize，而不是只装布局：CLI `check` 能看到
                    // 的东西，在这里一次给全，不用先存盘再去命令行对账。
                    let mut probe = self.driver.cfg.clone();
                    let notes = probe.sanitize();
                    self.ext_notes = if notes.is_empty() {
                        vec!["全部通过：布局、HID 绑定、报文模板、压感与热键都没有问题。"
                            .to_string()]
                    } else {
                        notes
                    };
                }
                _ => {}
            }
            if !self.ext_notes.is_empty() {
                ui.add_space(6.0);
                for note in self.ext_notes.clone() {
                    theme::row(ui, &p, "", None, |ui| {
                        ui.add(egui::Label::new(small(note.clone())).wrap());
                    });
                }
            }
        });
    }

    /// 卡一：自定义布局（键表 + HID 绑定）。
    fn draw_ext_layouts(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "自定义布局",
                Some(
                    "官方表里没有的机型、或者给普通触摸板贴一张小键盘贴纸、镭雕一套键位——\
                     键表按实际排布怎么写就怎么填，列数自动跟着键表走。",
                ),
            );
            ui.add_space(4.0);
            theme::note(
                ui,
                &p,
                "键名照键盘上印的写：7 8 9 +、0 period enter、esc tab back、up down left right；\
                 空位写 none（`.` 与 `-` 同样表示空位），减号写 minus，小数点写 period。\
                 列与列之间空格分隔，最多 8 列，缺的行留空即可。",
            );

            let mut remove: Option<usize> = None;
            let count = self.driver.cfg.custom_layouts.len();
            for index in 0..count {
                ui.add_space(8.0);
                // 每条布局之间用一条淡线切开，避免几条挤成一片分不清
                let rect = ui.available_rect_before_wrap();
                ui.painter()
                    .hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0, p.stroke));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("#{}", index + 1))
                            .size(12.5)
                            .color(p.text_weak),
                    );
                    if theme::secondary(ui, &p, "删除这条").clicked() {
                        remove = Some(index);
                    }
                });
                draw_one_layout(ui, &p, &mut self.driver.cfg.custom_layouts[index], index);
            }
            if let Some(index) = remove {
                self.driver.cfg.custom_layouts.remove(index);
            }

            ui.add_space(8.0);
            action_row(ui, |ui| {
                if theme::secondary(ui, &p, "添加一条自定义布局").clicked() {
                    self.driver
                        .cfg
                        .custom_layouts
                        .push(new_layout(self.driver.cfg.custom_layouts.len()));
                }
            });

            // 已装上的自定义布局一览：让用户确认「写进去了」而不只是「存在配置里」
            let installed = custom::list_custom();
            if !installed.is_empty() {
                ui.add_space(6.0);
                theme::note(ui, &p, "已注册（校验后生效）：");
                for (name, hids, rows) in installed {
                    let bind = if hids.is_empty() {
                        "未绑 HID".to_string()
                    } else {
                        hids.join(" ")
                    };
                    theme::read_row(ui, &p, &bind, &format!("{name} · {} 行", rows.len()), None);
                }
            }

            let bad: Vec<String> = self
                .driver
                .cfg
                .custom_layouts
                .iter()
                .filter_map(|spec| {
                    if custom::keys_inherit_base(&spec.keys) {
                        return None;
                    }
                    custom::KeyTable::parse(&spec.keys).err().map(|err| {
                        format!(
                            "{}：{err}",
                            if spec.name.trim().is_empty() {
                                "未命名"
                            } else {
                                spec.name.trim()
                            }
                        )
                    })
                })
                .collect();
            for line in bad {
                ui.add_space(2.0);
                theme::row(ui, &p, "", None, |ui| {
                    ui.label(egui::RichText::new(line).size(12.0).color(WARN));
                });
            }
        });
    }

    /// 卡二：自定义点亮报文。
    fn draw_ext_proto(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "点亮报文",
                Some(
                    "NumberPad 的背光是 FEATURE 报告中第 4 字节写亮度档；少数机型的报告 ID、\
                     前缀或尾字节不一样，这里可以按机型整条换掉。",
                ),
            );
            ui.add_space(4.0);
            let builtin = self.driver.cfg.led_proto.is_builtin();
            theme::read_row(
                ui,
                &p,
                "当前",
                if builtin {
                    "内置 0d 14 03 <level> ad"
                } else {
                    "自定义模板"
                },
                Some("内置报文走原来的快路径；自定义模板每点亮一次按档位替换占位符。"),
            );
            let resp = theme::row(
                ui,
                &p,
                "模板",
                Some("十六进制字节，空格或逗号分隔。用 ff 标记被亮度档替换的那一字节，其余原样下发。"),
                |ui| theme::text_field(ui, &mut self.ext_proto, "0d 14 03 ff ad"),
            );
            if resp.changed() {
                self.ext_proto_err = None;
                match parse_hex(&self.ext_proto) {
                    Ok(template) => {
                        let spec = LedProtoSpec { template };
                        match spec.validate() {
                            Ok(()) => {
                                self.driver.cfg.led_proto = spec;
                                self.ext_proto_dirty = true;
                            }
                            Err(err) => self.ext_proto_err = Some(err),
                        }
                    }
                    Err(err) => self.ext_proto_err = Some(err),
                }
            }
            if let Some(err) = self.ext_proto_err.clone() {
                theme::row(ui, &p, "", None, |ui| {
                    ui.label(egui::RichText::new(err).size(12.0).color(WARN));
                });
            }
            ui.add_space(2.0);
            action_row(ui, |ui| {
                ui.horizontal(|ui| {
                    if theme::secondary(ui, &p, "恢复内置报文").clicked() {
                        self.driver.cfg.led_proto = LedProtoSpec::default();
                        self.ext_proto = hex_of(&self.driver.cfg.led_proto.template);
                        self.ext_proto_err = None;
                    }
                });
            });
            // 手改配置后模板会变，输入框跟着对齐一次（用户正在打字时不动它）
            if !self.ext_proto_dirty
                && self.ext_proto != hex_of(&self.driver.cfg.led_proto.template)
            {
                self.ext_proto = hex_of(&self.driver.cfg.led_proto.template);
            }
            self.ext_proto_dirty = false;
        });
    }

    /// 卡三：压感切换（用力按一下 = 开关灯）。
    fn draw_ext_pressure(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "压感切换",
                Some(
                    "带压感（ABS_PRESSURE / ABS_MT_PRESSURE）的触摸板，可以按到某个力度就切换\
                     小键盘开关，不用去够键盘上的开关。",
                ),
            );
            ui.add_space(4.0);
            theme::check_row(
                ui,
                &p,
                &mut self.driver.cfg.pressure_switch.enabled,
                "启用",
                Some("没有压力轴的机型打开也不会误触：驱动读不到满量程就自动停用。"),
            );
            theme::row_hint(
                ui,
                &p,
                "触发阈值",
                Some("按到满量程的百分之多少就算「用力」；默认 80%。"),
                "%",
                |ui| theme::pair_num(ui, &mut self.driver.cfg.pressure_switch.percent, 1..=100),
            );
            theme::row_hint(
                ui,
                &p,
                "持续帧数",
                Some("要连续保持多少帧才算数：一帧等于芯片帧间隔，通常是 8 ms。帧数少容易误触，多则需要按得久。"),
                "帧",
                |ui| theme::pair_num(ui, &mut self.driver.cfg.pressure_switch.frames, 1..=200),
            );
            theme::row_hint(
                ui,
                &p,
                "冷却",
                Some("两次切换之间的最小间隔，防止按住不放来回翻转。"),
                "ms",
                |ui| {
                    ui.add_sized(
                        egui::Vec2::new(theme::pair_b(), theme::CTRL_H),
                        egui::DragValue::new(&mut self.driver.cfg.pressure_switch.cooldown_ms)
                            .range(0.0..=5000.0)
                            .speed(10.0),
                    )
                },
            );
            theme::note(
                ui,
                &p,
                "压感与「混合模式」互不影响：这里只改开关状态，不合成按键，也不会把压感事件转发出去。",
            );
        });
    }

    /// 卡四：全局热键。
    fn draw_ext_hotkeys(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "全局热键",
                Some(
                    "在物理键盘上绑一个组合键直接切换小键盘：触摸板贴纸方案、镭雕方案都能用，\
                     也不需要打开任何界面。",
                ),
            );
            ui.add_space(4.0);
            theme::note(
                ui,
                &p,
                "热键只旁听键盘事件，不拦截按键，正常打字不受影响；触发时也不会额外闪烁灯。",
            );

            let mut remove: Option<usize> = None;
            let count = self.driver.cfg.hotkeys.len();
            for index in 0..count {
                ui.add_space(6.0);
                theme::block(ui, &p, &format!("热键 {}", index + 1), None, |ui| {
                    let spec = &mut self.driver.cfg.hotkeys[index];
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            egui::Vec2::new(theme::ctrl_width() - 96.0, theme::CTRL_H),
                            egui::TextEdit::singleline(&mut spec.gesture).hint_text("ctrl+alt+n"),
                        );
                        if theme::secondary(ui, &p, "删除").clicked() {
                            remove = Some(index);
                        }
                    });
                    ui.horizontal(|ui| {
                        let mut action =
                            HotkeyAction::parse(&spec.action).unwrap_or(HotkeyAction::Toggle);
                        let before = action;
                        egui::ComboBox::from_id_salt(("hotkey_action", index))
                            .width(theme::pair_b())
                            .selected_text(action_label(action))
                            .show_ui(ui, |ui| {
                                for item in
                                    [HotkeyAction::Toggle, HotkeyAction::On, HotkeyAction::Off]
                                {
                                    ui.selectable_value(&mut action, item, action_label(item));
                                }
                            });
                        if action != before {
                            spec.action = action.as_str().to_string();
                        }
                        if spec.gesture.trim().is_empty() {
                            ui.label(egui::RichText::new("未填组合键").size(12.0).color(WARN));
                        }
                    });
                });
            }
            if let Some(index) = remove {
                self.driver.cfg.hotkeys.remove(index);
            }

            ui.add_space(8.0);
            action_row(ui, |ui| {
                if theme::secondary(ui, &p, "添加一个热键").clicked() {
                    self.driver.cfg.hotkeys.push(HotkeySpec {
                        gesture: String::new(),
                        action: HotkeyAction::Toggle.as_str().to_string(),
                    });
                }
            });
        });
    }
}

/// 画一条自定义布局的全部字段。
///
/// 走自由函数而不是 `&mut self`：编辑 `custom_layouts[i]` 时不能同时借整个 `App`。
fn draw_one_layout(
    ui: &mut egui::Ui,
    p: &theme::Palette,
    spec: &mut CustomLayoutSpec,
    index: usize,
) {
    eprintln!(
        "[dbg] before name row y={:.1} x={:.1} max={:?} clip={:?}",
        ui.next_widget_position().y,
        ui.next_widget_position().x,
        ui.max_rect(),
        ui.clip_rect()
    );
    theme::row(
        ui,
        p,
        "布局名",
        Some("写进驱动配置的 layout_name，也是自动识别命中时显示的名字。别和内置布局重名。"),
        |ui| theme::text_field(ui, &mut spec.name, "例如 GH3X-贴纸"),
    );

    // 基准布局：内置 + 已注册的自定义（可以基于上一条继续改）
    theme::row(
        ui,
        p,
        "基准",
        Some("只有写到的参数才覆盖基准，其余沿用。键表列数与基准不同也没关系。"),
        |ui| {
            let mut current = spec.base.clone();
            let text = if current.trim().is_empty() {
                "（未选）".to_string()
            } else {
                current.clone()
            };
            egui::ComboBox::from_id_salt(("custom_base", index))
                .width(theme::ctrl_width())
                .selected_text(text)
                .show_ui(ui, |ui| {
                    for entry in LAYOUTS {
                        ui.selectable_value(
                            &mut current,
                            entry.name.to_string(),
                            format!("{} · {} 列", entry.name, entry.layout.columns),
                        );
                    }
                    for (name, _hids, _rows) in custom::list_custom() {
                        ui.selectable_value(&mut current, name.clone(), format!("{name} · 自定义"));
                    }
                });
            if current != spec.base {
                spec.base = current;
            }
        },
    );

    // 键表：4 行单行输入，一行一串键名
    while spec.keys.len() < KEY_ROWS {
        spec.keys.push(String::new());
    }
    spec.keys.truncate(KEY_ROWS);
    theme::block(
        ui,
        p,
        "键表",
        Some("按小键盘实际排布逐行写。每行从左到右，空格分隔；行数与列数不必和官方一致。"),
        |ui| {
            for row in 0..KEY_ROWS {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        egui::Vec2::new(28.0, theme::CTRL_H),
                        egui::Label::new(
                            egui::RichText::new(format!("R{}", row + 1))
                                .size(12.0)
                                .color(p.text_weak),
                        ),
                    );
                    ui.add_sized(
                        egui::Vec2::new(theme::ctrl_width() - 40.0, theme::CTRL_H),
                        egui::TextEdit::singleline(&mut spec.keys[row]).hint_text(match row {
                            0 => "7 8 9 +",
                            1 => "4 5 6 -",
                            2 => "1 2 3 *",
                            _ => "0 . enter /",
                        }),
                    );
                });
            }
        },
    );

    // 实时解析结果：不用点保存就能知道这行键表能不能认
    if custom::keys_inherit_base(&spec.keys) {
        theme::read_row(
            ui,
            p,
            "键表解析",
            &format!(
                "留空，沿用 {} 的键表",
                if spec.base.trim().is_empty() {
                    "默认布局"
                } else {
                    spec.base.trim()
                }
            ),
            Some("键位照抄基准机型、只改行列几何时，把这一栏清空即可。"),
        );
        return;
    }
    match custom::KeyTable::parse(&spec.keys) {
        Ok(table) => {
            theme::read_row(
                ui,
                p,
                "键表解析",
                &format!("{} 列 × {} 行，可用", table.cols(), KEY_ROWS),
                Some("列数以键表为准：贴纸几列，键区就几列，不会串到相邻列。"),
            );
        }
        Err(err) => {
            theme::row(ui, p, "键表解析", None, |ui| {
                ui.label(egui::RichText::new(err).size(12.0).color(WARN));
            });
        }
    }

    // HID 绑定
    theme::block(
        ui,
        p,
        "绑定 HID",
        Some("填触摸板 HID 模组 ID（如 ASUE1416）。layout 为 auto 时命中即自动用这套键表。"),
        |ui| {
            let mut remove_hid: Option<usize> = None;
            for slot in 0..spec.hid_ids.len() {
                let id = slot;
                ui.horizontal(|ui| {
                    ui.add_sized(
                        egui::Vec2::new(theme::ctrl_width() - 96.0, theme::CTRL_H),
                        egui::TextEdit::singleline(&mut spec.hid_ids[id]).hint_text("ASUE1416"),
                    );
                    if theme::secondary(ui, p, "删除").clicked() {
                        remove_hid = Some(id);
                    }
                });
            }
            if let Some(slot) = remove_hid {
                spec.hid_ids.remove(slot);
            }
            ui.horizontal(|ui| {
                if theme::secondary(ui, p, "添加 HID").clicked() {
                    spec.hid_ids.push(String::new());
                }
            });
        },
    );
}

/// 新加一条自定义布局的初值：键表留空，用户照贴纸填。
fn new_layout(index: usize) -> CustomLayoutSpec {
    CustomLayoutSpec {
        name: format!("custom-{}", index + 1),
        ..CustomLayoutSpec::default()
    }
}

/// 没有标签的一排控件：缩进到控件列，和带标签的行左右对齐。
///
/// 这里不用 [`theme::ctrl_column`]：它内部的格子按「剩余可用高度」开，放进滚动区的
/// 长页面里时，那次 `available_height()` 会把整行撑成整屏高——页面上看来就是一行按钮
/// 之后凭空多出一大片空白。缩进 + `horizontal` 的效果一样，行高只由控件自己决定。
fn action_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(theme::LABEL_W + theme::ROW_GAP);
        add(ui);
    });
}

/// 解析十六进制字节串：接受空格 / 逗号 / `0x` 前缀与连写（`0d1403`）。
fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = text
        .replace("0x", " ")
        .replace("0X", " ")
        .replace([',', ':'], " ");
    let mut out = Vec::new();
    for token in cleaned.split_whitespace() {
        if token.len() % 2 != 0 {
            return Err(format!("`{token}` 是奇数个十六进制字符"));
        }
        for pair in token.as_bytes().chunks(2) {
            let pair = std::str::from_utf8(pair).map_err(|_| "非 ASCII".to_string())?;
            let byte =
                u8::from_str_radix(pair, 16).map_err(|_| format!("`{pair}` 不是十六进制"))?;
            out.push(byte);
        }
    }
    if out.is_empty() {
        return Err("模板不能为空".to_string());
    }
    Ok(out)
}

/// 字节串转回可编辑的十六进制文本。
pub(super) fn hex_of(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 热键动作的界面文案。
fn action_label(action: HotkeyAction) -> &'static str {
    match action {
        HotkeyAction::Toggle => "切换开关",
        HotkeyAction::On => "强制打开",
        HotkeyAction::Off => "强制关闭",
    }
}
