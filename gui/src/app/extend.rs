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

use asus_numpad::custom::{
    self, CustomLayoutSpec, Hotkey, HotkeyAction, HotkeySpec, KeySlot, LayoutPatch, LedProtoSpec,
    PressureSwitch,
};
use asus_numpad::layout::{self, Layout, LAYOUTS};

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
        ui.add_space(10.0);
        self.draw_ext_fragment(ui);

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
                    (
                        "放弃改动",
                        false,
                        "重新从磁盘读一遍配置，没保存的改动全部丢掉；已保存的不受影响。",
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
            if picked == Some(3) {
                *action = Some(Action::ReloadDriver);
                self.ext_notes.clear();
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

            // 装不上的原因一次列全：命名 / HID 冲突 + 键表解析错误
            let mut issues = layout_issues(&self.driver.cfg.custom_layouts);
            for (index, spec) in self.driver.cfg.custom_layouts.iter().enumerate() {
                if custom::keys_inherit_base(&spec.keys) {
                    continue;
                }
                if let Err(err) = custom::KeyTable::parse(&spec.keys) {
                    let tag = if spec.name.trim().is_empty() {
                        format!("#{} 未命名", index + 1)
                    } else {
                        format!("#{} {}", index + 1, spec.name.trim())
                    };
                    issues.push(format!("{tag}：{err}"));
                }
            }
            for line in issues {
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
            // 档位预览：把 0..4 档真会下发的字节摊开，换模板时不用靠脑补对表
            let proto = self.driver.cfg.led_proto.clone();
            if proto.validate().is_ok() {
                let levels: Vec<(u8, String)> = (0..=4u8)
                    .map(|level| {
                        let text = proto
                            .encode(level)
                            .map(|bytes| hex_of(&bytes))
                            .unwrap_or_else(|| "—".to_string());
                        (level, text)
                    })
                    .collect();
                egui::CollapsingHeader::new("各档实际下发的字节")
                    .id_salt("ext_proto_preview")
                    .default_open(!builtin)
                    .show(ui, |ui| {
                        for (level, text) in levels {
                            theme::read_row(
                                ui,
                                &p,
                                &format!("{level} 档"),
                                &text,
                                Some("这是把占位符替换成该档位后的整条报文，驱动就是照这几串字节下发的。"),
                            );
                        }
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
            // 先按规整写法收一遍组合键，下面每行据此判断「有没有和别的条目撞车」
            let labels: Vec<String> = self
                .driver
                .cfg
                .hotkeys
                .iter()
                .map(|spec| hotkey_key(&spec.gesture))
                .collect();
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
                    });
                    draw_hotkey_check(ui, &p, spec);
                    let mine = hotkey_key(&spec.gesture);
                    if !mine.is_empty() && labels.iter().filter(|other| **other == mine).count() > 1
                    {
                        ui.label(
                            egui::RichText::new(
                                "与其它条目重复：同一个组合键只有排在前面的那条生效",
                            )
                            .size(12.0)
                            .color(WARN),
                        );
                    }
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

    /// 卡五：配置片段——四项打包成一段 JSON，同机型之间直接对拷。
    ///
    /// 只装四项扩展，不碰帧节拍、几何兜底、界面语言这些「换了机器就该重设」的项。
    fn draw_ext_fragment(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "配置片段",
                Some(
                    "把四项扩展打包成一段 JSON：同机型的人贴过去点「应用」就能用。\
                     只含 custom_layouts / led_proto / pressure_switch / hotkeys，\
                     不含帧节拍、几何兜底、界面语言这些按机器走的项。",
                ),
            );
            ui.add_space(4.0);
            theme::row(
                ui,
                &p,
                "片段",
                Some("点「导出」从当前配置生成；也可以直接粘贴别人给的片段。"),
                |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.ext_snippet)
                            .desired_rows(5)
                            .desired_width(theme::ctrl_width())
                            .hint_text("{ ... }"),
                    )
                },
            );
            ui.add_space(2.0);
            action_row(ui, |ui| {
                ui.horizontal(|ui| {
                    let resp = theme::secondary(ui, &p, "导出");
                    let resp = theme::tip(resp, "把当前四项扩展序列化成 JSON 文本，可直接复制走。");
                    if resp.clicked() {
                        self.ext_snippet = fragment_of(&self.driver.cfg);
                        let len = self.ext_snippet.len();
                        self.ext_snippet_msg = Some((format!("已生成 {len} 字节的片段"), false));
                    }
                    let resp = theme::secondary(ui, &p, "应用");
                    let resp = theme::tip(
                        resp,
                        "把框里的片段写进配置；缺的项保持原样，写坏的项会明确指出是哪一段。",
                    );
                    if resp.clicked() {
                        self.ext_snippet_msg =
                            Some(apply_fragment(&mut self.driver.cfg, &self.ext_snippet));
                    }
                });
            });
            if let Some((msg, bad)) = self.ext_snippet_msg.clone() {
                ui.add_space(2.0);
                theme::row(ui, &p, "", None, |ui| {
                    ui.label(egui::RichText::new(msg).size(12.0).color(if bad {
                        WARN
                    } else {
                        p.ok
                    }));
                });
            }
        });
    }
}

/// 四项扩展打包成一段 JSON 文本（同机型对拷用）。
fn fragment_of(cfg: &asus_numpad::config::Config) -> String {
    let value = serde_json::json!({
        "custom_layouts": cfg.custom_layouts,
        "led_proto": cfg.led_proto,
        "pressure_switch": cfg.pressure_switch,
        "hotkeys": cfg.hotkeys,
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

/// 片段写回配置：缺的项保持原样，坏的那段直接报出是哪一项。
///
/// 返回值第二项为 `true` 表示出错。写进来的内容不做裁剪——真正能不能装，
/// 由保存时的 `sanitize` 统一判定，界面不替它猜。
fn apply_fragment(cfg: &mut asus_numpad::config::Config, text: &str) -> (String, bool) {
    let text = text.trim();
    if text.is_empty() {
        return (
            "片段是空的：先点「导出」，或粘贴别人的片段".to_string(),
            true,
        );
    }
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(err) => return (format!("不是合法 JSON：{err}"), true),
    };
    let mut touched: Vec<String> = Vec::new();
    // 先全部解析到局部，四项都过了再一次性写回：一段坏片段不该留下改了一半的配置。
    let mut layouts: Option<Vec<CustomLayoutSpec>> = None;
    let mut proto: Option<LedProtoSpec> = None;
    let mut pressure: Option<PressureSwitch> = None;
    let mut hotkeys: Option<Vec<HotkeySpec>> = None;

    if let Some(raw) = value.get("custom_layouts") {
        match serde_json::from_value::<Vec<CustomLayoutSpec>>(raw.clone()) {
            Ok(items) => {
                touched.push(format!("自定义布局 {} 条", items.len()));
                layouts = Some(items);
            }
            Err(err) => return (format!("custom_layouts 解析失败：{err}"), true),
        }
    }
    if let Some(raw) = value.get("led_proto") {
        match serde_json::from_value::<LedProtoSpec>(raw.clone()) {
            Ok(spec) => match spec.validate() {
                Ok(()) => {
                    touched.push("点亮报文".to_string());
                    proto = Some(spec);
                }
                Err(err) => return (format!("led_proto 模板不合法：{err}"), true),
            },
            Err(err) => return (format!("led_proto 解析失败：{err}"), true),
        }
    }
    if let Some(raw) = value.get("pressure_switch") {
        match serde_json::from_value::<PressureSwitch>(raw.clone()) {
            Ok(spec) => {
                touched.push("压感切换".to_string());
                pressure = Some(spec);
            }
            Err(err) => return (format!("pressure_switch 解析失败：{err}"), true),
        }
    }
    if let Some(raw) = value.get("hotkeys") {
        match serde_json::from_value::<Vec<HotkeySpec>>(raw.clone()) {
            Ok(items) => {
                touched.push(format!("热键 {} 条", items.len()));
                hotkeys = Some(items);
            }
            Err(err) => return (format!("hotkeys 解析失败：{err}"), true),
        }
    }

    if touched.is_empty() {
        return (
            "片段里没有这四项里的任何一项：需要 custom_layouts / led_proto / pressure_switch / hotkeys".to_string(),
            true,
        );
    }

    if let Some(items) = layouts {
        cfg.custom_layouts = items;
    }
    if let Some(spec) = proto {
        cfg.led_proto = spec;
    }
    if let Some(spec) = pressure {
        cfg.pressure_switch = spec;
    }
    if let Some(items) = hotkeys {
        cfg.hotkeys = items;
    }
    (
        format!("已应用 {}；记得点「保存」落盘", touched.join("、")),
        false,
    )
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

    // 键表：4 行单行输入，一行一串键名。
    // 只补齐、不截断：配置里若是手写了第 5 行，驱动侧解析会明确报错，界面这里
    // 悄悄把那行删掉就成了「打开界面即改配置」，保存后用户才发现内容少了。
    while spec.keys.len() < KEY_ROWS {
        spec.keys.push(String::new());
    }
    let extra_rows = spec.keys.len() - KEY_ROWS;
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
            ui.add_space(4.0);
            // 贴纸/镭雕多半是「照基准机型改一两个键」，先把基准键表抄成文本再改最省事
            ui.horizontal(|ui| {
                let base_name = spec.base.trim().to_string();
                let hint = if base_name.is_empty() {
                    "未选基准，会按内置键表填".to_string()
                } else {
                    format!("把 {base_name} 的键表抄成 4 行文字，在这上面逐个改键")
                };
                let resp = theme::secondary(ui, p, "按基准填满");
                let resp = theme::tip(resp, hint);
                if resp.clicked() {
                    spec.keys = custom::table_for(&base_name).describe();
                }
                let resp = theme::secondary(ui, p, "清空键表");
                let resp = theme::tip(resp, "清空表示键位沿用基准，只改行列几何时用这个。");
                if resp.clicked() {
                    spec.keys = vec![String::new(); KEY_ROWS];
                }
            });
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
            draw_key_preview(ui, p, &table);
        }
        Err(err) => {
            theme::row(ui, p, "键表解析", None, |ui| {
                ui.label(egui::RichText::new(err).size(12.0).color(WARN));
            });
        }
    }

    if extra_rows > 0 {
        theme::row(ui, p, "", None, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "配置里这段键表写了 {} 行，界面只编辑前 {KEY_ROWS} 行；驱动上限就是 {KEY_ROWS} 行，多出的行会在校验时报错。",
                    spec.keys.len()
                ))
                .size(12.0)
                .color(WARN),
            );
        });
    }

    // 几何覆盖：贴纸 / 镭雕方案真正要动的往往是这些 0.1 mm 的边界值，
    // 键表照抄基准就能用，键区却还压在基准机型的位置上。
    draw_one_patch(ui, p, index, &mut spec.patch, resolve_base(&spec.base));

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

/// 组合键的比对键：能解析就用规整写法，解析不出来退回去按原文小写比。
///
/// 判断「两条热键是不是同一个组合键」不能直接比字符串——`ctrl+alt+n` 与
/// `Alt+Ctrl+N` 在驱动里是同一个，界面上却是两串不同的字。
fn hotkey_key(gesture: &str) -> String {
    match Hotkey::parse(gesture) {
        Ok(hotkey) => hotkey.label(),
        Err(_) => gesture.trim().to_ascii_lowercase(),
    }
}

/// 布局列表里那些「装不上」的问题：空名、与内置重名、互相重名、HID 重复绑定。
///
/// 这些正是 `install_layouts` 会退回并写进校验提示的几种，提前在界面上标出来，
/// 省得存盘之后再去命令行对账。
fn layout_issues(specs: &[CustomLayoutSpec]) -> Vec<String> {
    let mut out = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        let name = spec.name.trim();
        let tag = if name.is_empty() {
            format!("#{} 未命名", index + 1)
        } else {
            format!("#{} {name}", index + 1)
        };
        if name.is_empty() {
            out.push(format!("{tag}：布局名为空，这条不会被装上"));
        } else {
            if layout::builtin_by_name(name).is_some() {
                out.push(format!("{tag}：与内置布局重名，会被退回"));
            }
            let clash = specs
                .iter()
                .skip(index + 1)
                .any(|other| other.name.trim().eq_ignore_ascii_case(name));
            if clash {
                out.push(format!("{tag}：与后面的另一条自定义布局重名"));
            }
        }
        for hid in &spec.hid_ids {
            let hid = hid.trim().to_ascii_uppercase();
            if hid.is_empty() {
                continue;
            }
            let repeated = specs.iter().enumerate().any(|(other_index, other)| {
                other_index != index
                    && other
                        .hid_ids
                        .iter()
                        .any(|h| h.trim().eq_ignore_ascii_case(&hid))
            });
            if repeated {
                out.push(format!("{tag}：{hid} 同时绑在了多条自定义布局上"));
            }
        }
    }
    out
}

/// 热键动作的界面文案。
fn action_label(action: HotkeyAction) -> &'static str {
    match action {
        HotkeyAction::Toggle => "切换开关",
        HotkeyAction::On => "强制打开",
        HotkeyAction::Off => "强制关闭",
    }
}

/// 配置里 `base` 指向的基准布局：先查内置表，再查已注册的自定义布局。
///
/// 只用于把基准数值显示在界面上给人对照，不参与命中判定。
fn resolve_base(name: &str) -> Option<Layout> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    if let Some(entry) = layout::builtin_by_name(name) {
        return Some(entry.layout);
    }
    custom::entry_by_name(name).map(|entry| entry.layout)
}

/// 键表预览：照贴纸的排布摆成网格，列数、行数、空位一眼看清。
///
/// 只画解析结果，不参与命中；真正生效的行列数仍以驱动解析为准。
fn draw_key_preview(ui: &mut egui::Ui, p: &theme::Palette, table: &custom::KeyTable) {
    let cols = table.cols().clamp(1, 8);
    let gap = 4.0;
    let cell_w = ((theme::ctrl_width() - gap * (cols as f32 - 1.0)) / cols as f32).min(76.0);
    ui.horizontal_top(|ui| {
        ui.add_space(theme::LABEL_W + ui.spacing().item_spacing.x);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = gap;
            for row in 0..KEY_ROWS {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for col in 0..cols {
                        let slot = table.get(row, col).unwrap_or(KeySlot::None);
                        key_cell(ui, p, slot, cell_w);
                    }
                });
            }
        });
    });
}

/// 预览里的一格：有键实底、空位凹底，空位印 `·` 以免看成少了格子。
fn key_cell(ui: &mut egui::Ui, p: &theme::Palette, slot: KeySlot, cell_w: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(cell_w, 26.0), egui::Sense::hover());
    let empty = matches!(slot, KeySlot::None);
    let fill = if empty { p.sunk } else { p.field };
    let color = if empty { p.text_weak } else { p.text };
    let text = if empty {
        "·".to_string()
    } else {
        slot.label()
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(6),
        fill,
        egui::Stroke::new(1.0, p.stroke),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(12.0),
        color,
    );
}

/// 几何覆盖：`LayoutPatch` 的 15 项，逐项「勾选启用 + 数值」。
///
/// 贴纸 / 镭雕方案常常只动 `columns` 与四边内缩、键位照抄基准；界面原先只给键表
/// 不给这些，改完键表还得去手改 JSON，等于半个功能。
fn draw_one_patch(
    ui: &mut egui::Ui,
    p: &theme::Palette,
    index: usize,
    patch: &mut LayoutPatch,
    base: Option<Layout>,
) {
    theme::block(
        ui,
        p,
        "几何覆盖",
        Some(
            "单位 0.1 mm，对应官方 AsusNUMPADFilter.inf 的 NUMPADLAYOUT_<机型>_00..14。\
             只有勾上的项才写进配置，其余沿用基准；不勾时右侧显示基准原值。",
        ),
        |ui| {
            let covered = patch_count(patch);
            let head = if covered == 0 {
                "未覆盖，全按基准几何".to_string()
            } else {
                format!("已覆盖 {covered} 项")
            };
            egui::CollapsingHeader::new(head)
                .id_salt(("custom_patch", index))
                .default_open(covered > 0)
                .show(ui, |ui| {
                    patch_section(ui, p, "键区");
                    patch_row(
                        ui,
                        p,
                        "列数",
                        "键区摊开成几列，1..=8。填了键表时列数以键表为准，这一项会被键表覆盖。",
                        base.map(|l| l.columns),
                        &mut patch.columns,
                        1..=8,
                    );
                    patch_row(
                        ui,
                        p,
                        "首列左留 _01",
                        "第 0 列左侧留白，越小键区越靠左。",
                        base.map(|l| l.index0),
                        &mut patch.index0,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "末列右留 _02",
                        "最后一列右侧留白；列数不是 5 时它压在末列上。",
                        base.map(|l| l.index4),
                        &mut patch.index4,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "首行上留 _03",
                        "第 0 行上侧留白。",
                        base.map(|l| l.level0),
                        &mut patch.level0,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "末行下留 _04",
                        "最后一行下侧留白。",
                        base.map(|l| l.level3),
                        &mut patch.level3,
                        0..=250,
                    );

                    patch_section(ui, p, "有效区内缩");
                    patch_row(
                        ui,
                        p,
                        "左 _07",
                        "有效区左边内缩，防止边缘误触。",
                        base.map(|l| l.left),
                        &mut patch.left,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "右 _08",
                        "有效区右边内缩。",
                        base.map(|l| l.right),
                        &mut patch.right,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "上 _09",
                        "有效区上边内缩。",
                        base.map(|l| l.up),
                        &mut patch.up,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "下 _10",
                        "有效区下边内缩。",
                        base.map(|l| l.down),
                        &mut patch.down,
                        0..=250,
                    );

                    patch_section(ui, p, "NumLock 热区");
                    patch_row(
                        ui,
                        p,
                        "Edge Index _05",
                        "NumLock 边缘的热区索引。当前实现不把它用于热区判定，按官方原值填即可。",
                        base.map(|l| l.numlock_index),
                        &mut patch.numlock_index,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "Edge Level _06",
                        "NumLock 边缘的热区高度。",
                        base.map(|l| l.numlock_level),
                        &mut patch.numlock_level,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "Adv Index _11",
                        "NumLock 扩展热区索引。",
                        base.map(|l| l.numadv_index),
                        &mut patch.numadv_index,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "Adv Level _12",
                        "NumLock 扩展热区高度。",
                        base.map(|l| l.numadv_level),
                        &mut patch.numadv_level,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "InValid Index _13",
                        "NumLock 无效区索引，决定「不算按到键」的那条边界。",
                        base.map(|l| l.invalid_index),
                        &mut patch.invalid_index,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "InValid Level _14",
                        "NumLock 无效区高度。",
                        base.map(|l| l.invalid_level),
                        &mut patch.invalid_level,
                        0..=250,
                    );
                });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let has_base = base.is_some();
                let resp = ui.add_enabled(has_base, {
                    let size = egui::Vec2::new(theme::BTN_W, theme::CTRL_H);
                    egui::Button::new(egui::RichText::new("全部填入基准数值").color(p.text))
                        .fill(p.field)
                        .stroke(egui::Stroke::new(1.0, p.stroke))
                        .min_size(size)
                });
                let resp = theme::tip(
                    resp,
                    "把基准机型的 15 项原值一次性抄进配置，方便在此基础上逐项微调。",
                );
                if resp.clicked() {
                    if let Some(base) = base {
                        patch_fill_base(patch, base);
                    }
                }
                if covered > 0 {
                    let resp = ui.add({
                        let size = egui::Vec2::new(theme::BTN_W, theme::CTRL_H);
                        egui::Button::new(egui::RichText::new("清空覆盖").color(p.text))
                            .fill(p.field)
                            .stroke(egui::Stroke::new(1.0, p.stroke))
                            .min_size(size)
                    });
                    let resp = theme::tip(resp, "撤回这 15 项的全部覆盖，改回「全按基准」。");
                    if resp.clicked() {
                        *patch = LayoutPatch::default();
                    }
                }
            });
        },
    );
}

/// 几何区里的小标题：把 15 项分成键区 / 内缩 / 热区三组。
fn patch_section(ui: &mut egui::Ui, p: &theme::Palette, text: &str) {
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(text)
            .size(11.5)
            .strong()
            .color(p.text_weak),
    );
    ui.add_space(2.0);
}

/// 一行几何参数：开关决定这写不写进配置，未勾时右侧灰字显示基准原值。
fn patch_row(
    ui: &mut egui::Ui,
    p: &theme::Palette,
    label: &str,
    help: &str,
    base: Option<i32>,
    field: &mut Option<i32>,
    range: std::ops::RangeInclusive<i32>,
) {
    ui.horizontal(|ui| {
        let mut on = field.is_some();
        let resp = theme::switch(ui, p, &mut on);
        let resp = theme::tip(resp, format!("勾上即写进配置。{help}"));
        if resp.changed() {
            *field = if on {
                Some(base.unwrap_or(*range.start()))
            } else {
                None
            };
        }
        ui.add_sized(
            egui::Vec2::new(theme::LABEL_W - 20.0, theme::CTRL_H),
            egui::Label::new(egui::RichText::new(label).size(12.0).color(p.text_weak)).truncate(),
        )
        .on_hover_text(help);
        match field.as_mut() {
            Some(value) => {
                ui.add_sized(
                    egui::Vec2::new(theme::pair_b(), theme::CTRL_H),
                    egui::DragValue::new(value).range(range.clone()),
                );
                match base {
                    Some(base) if base != *value => {
                        ui.label(
                            egui::RichText::new(format!("基准 {base}（{:+}）", *value - base))
                                .size(11.5)
                                .color(p.accent),
                        );
                    }
                    Some(base) => {
                        ui.label(
                            egui::RichText::new(format!("基准 {base}"))
                                .size(11.5)
                                .color(p.text_weak),
                        );
                    }
                    None => {}
                }
            }
            None => {
                let text = base
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| "—".to_string());
                ui.add_sized(
                    egui::Vec2::new(theme::pair_b(), theme::CTRL_H),
                    egui::Label::new(
                        egui::RichText::new(text)
                            .size(12.0)
                            .monospace()
                            .color(p.text_weak),
                    ),
                );
            }
        }
    });
}

/// 已覆盖的项数，用于折叠标题上标一句「改了几项」。
fn patch_count(patch: &LayoutPatch) -> usize {
    [
        patch.columns,
        patch.index0,
        patch.index4,
        patch.level0,
        patch.level3,
        patch.numlock_index,
        patch.numlock_level,
        patch.left,
        patch.right,
        patch.up,
        patch.down,
        patch.numadv_index,
        patch.numadv_level,
        patch.invalid_index,
        patch.invalid_level,
    ]
    .iter()
    .filter(|v| v.is_some())
    .count()
}

/// 把基准布局的 15 项原值整组抄进覆盖表。
fn patch_fill_base(patch: &mut LayoutPatch, base: Layout) {
    patch.columns = Some(base.columns);
    patch.index0 = Some(base.index0);
    patch.index4 = Some(base.index4);
    patch.level0 = Some(base.level0);
    patch.level3 = Some(base.level3);
    patch.numlock_index = Some(base.numlock_index);
    patch.numlock_level = Some(base.numlock_level);
    patch.left = Some(base.left);
    patch.right = Some(base.right);
    patch.up = Some(base.up);
    patch.down = Some(base.down);
    patch.numadv_index = Some(base.numadv_index);
    patch.numadv_level = Some(base.numadv_level);
    patch.invalid_index = Some(base.invalid_index);
    patch.invalid_level = Some(base.invalid_level);
}

/// 组合键校验：写得对就显示规整写法，写得不对就把驱动会报的错原样摆出来。
fn draw_hotkey_check(ui: &mut egui::Ui, p: &theme::Palette, spec: &mut HotkeySpec) {
    if spec.gesture.trim().is_empty() {
        ui.label(
            egui::RichText::new("未填组合键，这条不会生效")
                .size(12.0)
                .color(WARN),
        );
        return;
    }
    match Hotkey::parse(&spec.gesture) {
        Ok(hotkey) => {
            let canonical = hotkey.label();
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("识别为 {canonical}"))
                        .size(12.0)
                        .color(p.ok),
                );
                if canonical != spec.gesture.trim() {
                    let resp = theme::secondary(ui, p, "规整写法");
                    let resp = theme::tip(
                        resp,
                        "把 `Ctrl+Alt+N`、`ctrl + alt + n` 这类写法统一成配置里的规范形式。",
                    );
                    if resp.clicked() {
                        spec.gesture = canonical;
                    }
                }
            });
        }
        Err(err) => {
            ui.label(
                egui::RichText::new(format!("解析失败：{err}"))
                    .size(12.0)
                    .color(WARN),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asus_numpad::config::Config;

    /// 导出再导入必须得到同一份四项配置——片段是对外分享的文件格式，不能单向。
    #[test]
    fn fragment_round_trip() {
        let source = Config {
            custom_layouts: vec![CustomLayoutSpec {
                name: "GH3X-贴纸".to_string(),
                base: "GH4XLNDZ".to_string(),
                hid_ids: vec!["ASUE1416".to_string()],
                keys: vec!["7 8 9 +".to_string(), "4 5 6 -".to_string()],
                patch: LayoutPatch {
                    columns: Some(4),
                    ..Default::default()
                },
            }],
            hotkeys: vec![HotkeySpec {
                gesture: "ctrl+alt+n".to_string(),
                action: "toggle".to_string(),
            }],
            ..Default::default()
        };

        let text = fragment_of(&source);
        let mut target = Config::default();
        let (msg, bad) = apply_fragment(&mut target, &text);
        assert!(!bad, "导入片段失败：{msg}");
        assert_eq!(target.custom_layouts, source.custom_layouts);
        assert_eq!(target.hotkeys, source.hotkeys);
        assert_eq!(target.led_proto, source.led_proto);
        assert_eq!(target.pressure_switch, source.pressure_switch);
    }

    /// 空片段、非 JSON、四项都不沾的 JSON 都要明确报错，而不是「悄悄什么都没改」。
    #[test]
    fn fragment_rejects_bad_input() {
        let mut cfg = Config::default();
        let (msg, bad) = apply_fragment(&mut cfg, "   ");
        assert!(bad && msg.contains("空"), "{msg}");

        let (msg, bad) = apply_fragment(&mut cfg, "numpad = 1");
        assert!(bad && msg.contains("JSON"), "{msg}");

        let (msg, bad) = apply_fragment(&mut cfg, r#"{"frame_ms": 8}"#);
        assert!(bad && msg.contains("四项"), "{msg}");
        assert_eq!(cfg, Config::default(), "报错时不该改动配置");
    }

    /// 报文模板不合法时整段拒绝，不能把坏模板写进配置。
    #[test]
    fn fragment_rejects_bad_led_proto() {
        let mut cfg = Config::default();
        let (msg, bad) = apply_fragment(&mut cfg, r#"{"led_proto": {"template": [13, 20]}}"#);
        assert!(bad && msg.contains("led_proto"), "{msg}");
        assert_eq!(cfg, Config::default());
    }

    /// 两组字面不同、实际同义的写法要认成同一条热键，否则重复检测形同虚设。
    #[test]
    fn hotkey_key_normalises_case_and_order() {
        assert_eq!(hotkey_key("ctrl+alt+n"), hotkey_key("Alt+Ctrl+N"));
        assert_ne!(hotkey_key("ctrl+alt+n"), hotkey_key("ctrl+shift+n"));
        assert_eq!(hotkey_key("  "), "");
    }

    /// 命名与 HID 冲突要在界面上先报出来，覆盖 `install_layouts` 会退回的几种。
    #[test]
    fn layout_issues_flags_conflicts() {
        let builtin = LAYOUTS[0].name.to_string();
        let specs = vec![
            CustomLayoutSpec {
                name: String::new(),
                ..Default::default()
            },
            CustomLayoutSpec {
                name: builtin,
                ..Default::default()
            },
            CustomLayoutSpec {
                name: "贴纸".to_string(),
                hid_ids: vec!["ASUE1416".to_string()],
                ..Default::default()
            },
            CustomLayoutSpec {
                name: "贴纸".to_string(),
                hid_ids: vec!["asue1416".to_string()],
                ..Default::default()
            },
        ];
        let issues = layout_issues(&specs);
        assert!(
            issues.iter().any(|line| line.contains("未命名")),
            "{issues:?}"
        );
        assert!(
            issues.iter().any(|line| line.contains("内置布局重名")),
            "{issues:?}"
        );
        assert!(
            issues.iter().any(|line| line.contains("重名")),
            "{issues:?}"
        );
        assert!(
            issues.iter().any(|line| line.contains("ASUE1416")),
            "{issues:?}"
        );
    }
}
