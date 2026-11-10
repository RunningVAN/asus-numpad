//! 命令面板的表单状态与参数拼装。
//!
//! 这里只负责「界面上的控件值」到「`asus-numpad` 的 argv」的映射，不执行任何东西，
//! 因此可以脱离界面做单测。拼错参数名、把空串当成有效值、把互斥开关同时勾上这类
//! 问题，都由 [`CommandForm::argv`] 的测试兜住。
//!
//! 每个子命令的可选形态都照 CLI 的实际分支来：`bright` 的 VALUE 位既可以是档位数字，
//! 也可以是 `follow` / `cycle` / `up` / `down` 这类动作，`levels` 的 STEP 位在只写
//! `reset` 时是「恢复默认」，两者写法不同，所以界面用枚举区分而不是一个自由输入框。

use std::fmt;

/// 可选的子命令。`run` 不在列表里：它是前台常驻引擎，界面上等价的入口是
/// 「系统状态」区的服务启停。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubCmd {
    Check,
    Dump,
    Bright,
    Levels,
    Led,
    Layout,
    Sku,
    Lang,
}

impl SubCmd {
    pub const ALL: [SubCmd; 8] = [
        SubCmd::Check,
        SubCmd::Dump,
        SubCmd::Bright,
        SubCmd::Levels,
        SubCmd::Led,
        SubCmd::Layout,
        SubCmd::Sku,
        SubCmd::Lang,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SubCmd::Check => "check",
            SubCmd::Dump => "dump",
            SubCmd::Bright => "bright",
            SubCmd::Levels => "levels",
            SubCmd::Led => "led",
            SubCmd::Layout => "layout",
            SubCmd::Sku => "sku",
            SubCmd::Lang => "lang",
        }
    }

    /// 一句说明，显示在参数区上方，省得用户去翻 README。
    pub fn about(self) -> &'static str {
        match self {
            SubCmd::Check => "校验配置与识别结果，只读",
            SubCmd::Dump => "导出当前生效的全部配置，只读",
            SubCmd::Bright => "键盘背光联动：查看状态、开关跟随、按档位增减",
            SubCmd::Levels => "档位表：查看每档命令字，或改档位数、重排映射（写配置）",
            SubCmd::Led => "小键盘背光：点亮、熄灭或直接下发十六进制命令字",
            SubCmd::Layout => "触摸板布局：查看已识别的布局，或指定布局（写配置）",
            SubCmd::Sku => "机型：查看识别结果、列出已知机型，或手动指定 / 取消（写配置）",
            SubCmd::Lang => "界面与日志语言（写配置）",
        }
    }

    pub fn writes_config(self) -> bool {
        matches!(
            self,
            SubCmd::Levels | SubCmd::Layout | SubCmd::Sku | SubCmd::Lang
        )
    }
}

impl fmt::Display for SubCmd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// `bright` 的 VALUE 位。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BrightAction {
    /// 不传 VALUE，CLI 打印当前状态
    Status,
    /// 直接跳到某一档
    Step,
    /// 跟随键盘背光，开或关由 `bright_follow_on` 决定
    Follow,
    Cycle,
    Up,
    Down,
}

impl BrightAction {
    pub const ALL: [BrightAction; 6] = [
        BrightAction::Status,
        BrightAction::Step,
        BrightAction::Follow,
        BrightAction::Cycle,
        BrightAction::Up,
        BrightAction::Down,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BrightAction::Status => "查看状态",
            BrightAction::Step => "跳到指定档位",
            BrightAction::Follow => "跟随键盘背光（开 / 关）",
            BrightAction::Cycle => "走到下一档（cycle）",
            BrightAction::Up => "升一档（up）",
            BrightAction::Down => "降一档（down）",
        }
    }

    fn word(self) -> Option<&'static str> {
        match self {
            BrightAction::Status => None,
            BrightAction::Step => None,
            BrightAction::Follow => Some("follow"),
            BrightAction::Cycle => Some("cycle"),
            BrightAction::Up => Some("up"),
            BrightAction::Down => Some("down"),
        }
    }
}

/// `levels` 的形态。只写 STEP 数字是非法组合，CLI 会打用法并返回 2，界面因此拆成三态。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LevelsAction {
    /// 不传参数，打印档位表
    Table,
    /// STEP 位写 reset，恢复默认档表
    Reset,
    /// STEP 与 VALUE 同时给，改写某一档
    Set,
}

impl LevelsAction {
    pub const ALL: [LevelsAction; 3] =
        [LevelsAction::Table, LevelsAction::Reset, LevelsAction::Set];

    pub fn label(self) -> &'static str {
        match self {
            LevelsAction::Table => "查看档位表",
            LevelsAction::Reset => "恢复默认档位表（reset）",
            LevelsAction::Set => "改写某一档",
        }
    }
}

/// `led` 的 VALUE 位。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LedAction {
    /// 不传参数，打印当前值与通道
    Status,
    On,
    Off,
    /// 原样下发一个字节，如 `0x41`
    Raw,
}

impl LedAction {
    pub const ALL: [LedAction; 4] = [
        LedAction::Status,
        LedAction::On,
        LedAction::Off,
        LedAction::Raw,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LedAction::Status => "不带值（打印用法）",
            LedAction::On => "点亮（on）",
            LedAction::Off => "熄灭（off）",
            LedAction::Raw => "直接下发命令字",
        }
    }
}

/// 全局开关与高级选项。字段都是界面原样输入，拼装时才判空与校验。
#[derive(Default)]
pub struct Advanced {
    pub config: String,
    pub device: String,
    pub keyboard: String,
    pub led: String,
    pub hidraw: String,
    pub i2c_bus: String,
    pub i2c_addr: String,
    pub i2c_transfer: String,
    pub key_mode: String,
    pub layout: String,
    pub sku: String,
    pub numlock: String,
    pub mixed_mode: String,
    pub no_follow_brightness: bool,
    pub grab: bool,
    pub no_grab: bool,
    pub allow_unverified: bool,
    pub dump_config: bool,
    pub verbose: bool,
}

impl Advanced {
    fn append(&self, argv: &mut Vec<String>) -> Result<(), String> {
        let mut push = |flag: &str, value: &str| {
            let value = value.trim();
            if !value.is_empty() {
                argv.push(flag.to_string());
                argv.push(value.to_string());
            }
        };
        push("--config", &self.config);
        push("--device", &self.device);
        push("--keyboard", &self.keyboard);
        push("--led", &self.led);
        push("--hidraw", &self.hidraw);
        push("--i2c-addr", &self.i2c_addr);
        push("--i2c-transfer", &self.i2c_transfer);
        push("--key-mode", &self.key_mode);
        push("--layout", &self.layout);
        push("--sku", &self.sku);
        push("--numlock", &self.numlock);
        push("--mixed-mode", &self.mixed_mode);

        if !self.i2c_bus.trim().is_empty() {
            let bus: i32 = self
                .i2c_bus
                .trim()
                .parse()
                .map_err(|_| format!("--i2c-bus 需要整数：{}", self.i2c_bus.trim()))?;
            argv.push("--i2c-bus".to_string());
            argv.push(bus.to_string());
        }

        for (on, flag) in [
            (self.no_follow_brightness, "--no-follow-brightness"),
            (self.allow_unverified, "--allow-unverified"),
            (self.dump_config, "--dump-config"),
            (self.verbose, "--verbose"),
        ] {
            if on {
                argv.push(flag.to_string());
            }
        }

        if self.grab && self.no_grab {
            return Err("--grab 与 --no-grab 互斥，只能勾一个".to_string());
        }
        if self.grab {
            argv.push("--grab".to_string());
        }
        if self.no_grab {
            argv.push("--no-grab".to_string());
        }
        Ok(())
    }
}

/// 命令面板的完整表单。
pub struct CommandForm {
    pub sub: SubCmd,
    pub bright_action: BrightAction,
    pub bright_step: String,
    pub bright_follow_on: bool,
    pub levels_action: LevelsAction,
    pub levels_step: String,
    pub levels_value: String,
    pub led_action: LedAction,
    pub led_raw: String,
    pub layout_name: String,
    pub layout_ids: bool,
    pub sku_id: String,
    pub lang_code: String,
    pub advanced: Advanced,
}

impl Default for CommandForm {
    fn default() -> Self {
        Self {
            sub: SubCmd::Check,
            bright_action: BrightAction::Status,
            bright_step: "2".to_string(),
            bright_follow_on: true,
            levels_action: LevelsAction::Table,
            levels_step: "1".to_string(),
            levels_value: "0x41".to_string(),
            led_action: LedAction::Status,
            led_raw: "0x41".to_string(),
            layout_name: String::new(),
            layout_ids: false,
            sku_id: String::new(),
            lang_code: String::new(),
            advanced: Advanced::default(),
        }
    }
}

impl CommandForm {
    /// 拼出参数列表（不含可执行文件路径）。
    pub fn argv(&self) -> Result<Vec<String>, String> {
        let mut argv = Vec::new();
        self.advanced.append(&mut argv)?;

        let sub = self.sub;
        argv.push(sub.name().to_string());

        match sub {
            SubCmd::Check | SubCmd::Dump => {}
            SubCmd::Bright => {
                if let Some(word) = self.bright_action.word() {
                    argv.push(word.to_string());
                    if self.bright_action == BrightAction::Follow {
                        argv.push(if self.bright_follow_on { "on" } else { "off" }.to_string());
                    }
                }
                if self.bright_action == BrightAction::Step {
                    let step = self.bright_step.trim();
                    let parsed: u8 = step.parse().map_err(|_| format!("档位需要数字：{step}"))?;
                    argv.push(parsed.to_string());
                }
            }
            SubCmd::Levels => match self.levels_action {
                LevelsAction::Table => {}
                LevelsAction::Reset => argv.push("reset".to_string()),
                LevelsAction::Set => {
                    let step = self.levels_step.trim();
                    let value = self.levels_value.trim();
                    if step.is_empty() || value.is_empty() {
                        return Err("改写某一档需要同时给出档位与命令字".to_string());
                    }
                    let parsed: usize =
                        step.parse().map_err(|_| format!("档位需要数字：{step}"))?;
                    argv.push(parsed.to_string());
                    argv.push(value.to_string());
                }
            },
            SubCmd::Led => match self.led_action {
                LedAction::Status => {}
                LedAction::On => argv.push("on".to_string()),
                LedAction::Off => argv.push("off".to_string()),
                LedAction::Raw => {
                    let raw = self.led_raw.trim();
                    if raw.is_empty() {
                        return Err("直接下发命令字时要填一个字节，如 0x41".to_string());
                    }
                    argv.push(raw.to_string());
                }
            },
            SubCmd::Layout => {
                let name = self.layout_name.trim();
                if !name.is_empty() {
                    argv.push(name.to_string());
                }
                if self.layout_ids {
                    argv.push("--ids".to_string());
                }
            }
            SubCmd::Sku => {
                let id = self.sku_id.trim();
                if !id.is_empty() {
                    argv.push(id.to_string());
                }
            }
            SubCmd::Lang => {
                let code = self.lang_code.trim();
                if !code.is_empty() {
                    argv.push(code.to_string());
                }
            }
        }

        Ok(argv)
    }

    /// 界面上要显示的一行预览，便于复制到终端复现。
    pub fn preview(&self, bin: &str) -> String {
        match self.argv() {
            Ok(argv) => {
                let mut text = bin.to_string();
                for arg in argv {
                    text.push(' ');
                    if arg.contains(' ') {
                        text.push('"');
                        text.push_str(&arg);
                        text.push('"');
                    } else {
                        text.push_str(&arg);
                    }
                }
                text
            }
            Err(err) => format!("参数有误：{err}"),
        }
    }

    /// 这次执行要不要给个时间上限。
    ///
    /// `dump` 在 CLI 里是「一直打印事件直到 Ctrl+C」，界面上必须有个头，否则面板会
    /// 永远停在「执行中」；其余子命令都是跑完就退出，交回 `None`。
    pub fn timeout(&self) -> Option<std::time::Duration> {
        match self.sub {
            SubCmd::Dump => Some(std::time::Duration::from_secs(5)),
            _ => None,
        }
    }

    /// 这次执行会不会落盘改配置，界面据此把按钮标成需要留心的一项。
    pub fn writes_config(&self) -> bool {
        if !self.sub.writes_config() {
            return false;
        }
        match self.sub {
            SubCmd::Levels => self.levels_action != LevelsAction::Table,
            SubCmd::Layout => !self.layout_name.trim().is_empty(),
            SubCmd::Sku => !self.sku_id.trim().is_empty(),
            SubCmd::Lang => !self.lang_code.trim().is_empty(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(sub: SubCmd) -> CommandForm {
        CommandForm {
            sub,
            ..Default::default()
        }
    }

    #[test]
    fn check_and_dump_take_no_arguments() {
        assert_eq!(form(SubCmd::Check).argv().unwrap(), vec!["check"]);
        assert_eq!(form(SubCmd::Dump).argv().unwrap(), vec!["dump"]);
    }

    #[test]
    fn bright_status_sends_no_value() {
        assert_eq!(form(SubCmd::Bright).argv().unwrap(), vec!["bright"]);
    }

    #[test]
    fn bright_step_is_a_bare_number() {
        let mut f = form(SubCmd::Bright);
        f.bright_action = BrightAction::Step;
        f.bright_step = " 3 ".to_string();
        assert_eq!(f.argv().unwrap(), vec!["bright", "3"]);

        f.bright_step = "亮".to_string();
        assert!(f.argv().unwrap_err().contains("档位需要数字"));
    }

    #[test]
    fn bright_follow_carries_on_or_off() {
        let mut f = form(SubCmd::Bright);
        f.bright_action = BrightAction::Follow;
        assert_eq!(f.argv().unwrap(), vec!["bright", "follow", "on"]);
        f.bright_follow_on = false;
        assert_eq!(f.argv().unwrap(), vec!["bright", "follow", "off"]);
    }

    #[test]
    fn bright_cycle_and_up_down() {
        let mut f = form(SubCmd::Bright);
        f.bright_action = BrightAction::Cycle;
        assert_eq!(f.argv().unwrap(), vec!["bright", "cycle"]);
        f.bright_action = BrightAction::Up;
        assert_eq!(f.argv().unwrap(), vec!["bright", "up"]);
        f.bright_action = BrightAction::Down;
        assert_eq!(f.argv().unwrap(), vec!["bright", "down"]);
    }

    #[test]
    fn levels_table_and_reset() {
        assert_eq!(form(SubCmd::Levels).argv().unwrap(), vec!["levels"]);

        let mut f = form(SubCmd::Levels);
        f.levels_action = LevelsAction::Reset;
        assert_eq!(f.argv().unwrap(), vec!["levels", "reset"]);
    }

    #[test]
    fn levels_set_needs_both_fields() {
        let mut f = form(SubCmd::Levels);
        f.levels_action = LevelsAction::Set;
        f.levels_value.clear();
        assert!(f.argv().unwrap_err().contains("同时给出"));

        f.levels_value = "0x48".to_string();
        f.levels_step = "2".to_string();
        assert_eq!(f.argv().unwrap(), vec!["levels", "2", "0x48"]);
    }

    #[test]
    fn led_variants() {
        assert_eq!(form(SubCmd::Led).argv().unwrap(), vec!["led"]);

        let mut f = form(SubCmd::Led);
        f.led_action = LedAction::On;
        assert_eq!(f.argv().unwrap(), vec!["led", "on"]);
        f.led_action = LedAction::Off;
        assert_eq!(f.argv().unwrap(), vec!["led", "off"]);

        f.led_action = LedAction::Raw;
        assert_eq!(f.argv().unwrap(), vec!["led", "0x41"]);
        f.led_raw.clear();
        assert!(f.argv().unwrap_err().contains("0x41"));
    }

    #[test]
    fn layout_name_and_ids_flag() {
        let mut f = form(SubCmd::Layout);
        f.layout_name = "g614".to_string();
        f.layout_ids = true;
        assert_eq!(f.argv().unwrap(), vec!["layout", "g614", "--ids"]);
        assert!(f.writes_config());
    }

    #[test]
    fn layout_without_name_is_read_only() {
        let mut f = form(SubCmd::Layout);
        f.layout_ids = true;
        assert_eq!(f.argv().unwrap(), vec!["layout", "--ids"]);
        assert!(!f.writes_config());
    }

    #[test]
    fn sku_and_lang_take_optional_positional() {
        let mut f = form(SubCmd::Sku);
        f.sku_id = "g614jv".to_string();
        assert_eq!(f.argv().unwrap(), vec!["sku", "g614jv"]);

        let mut f = form(SubCmd::Lang);
        assert_eq!(f.argv().unwrap(), vec!["lang"]);
        f.lang_code = "zh_TW".to_string();
        assert_eq!(f.argv().unwrap(), vec!["lang", "zh_TW"]);
    }

    #[test]
    fn advanced_flags_go_before_subcommand() {
        let mut f = form(SubCmd::Check);
        f.advanced.verbose = true;
        f.advanced.config = "/tmp/a.json".to_string();
        assert_eq!(
            f.argv().unwrap(),
            vec!["--config", "/tmp/a.json", "--verbose", "check"]
        );
    }

    #[test]
    fn advanced_numbers_are_validated() {
        let mut f = form(SubCmd::Check);
        f.advanced.i2c_bus = "abc".to_string();
        assert!(f.argv().unwrap_err().contains("--i2c-bus"));

        f.advanced.i2c_bus = "12".to_string();
        assert_eq!(f.argv().unwrap(), vec!["--i2c-bus", "12", "check"]);
    }

    #[test]
    fn grab_and_nograb_are_exclusive() {
        let mut f = form(SubCmd::Check);
        f.advanced.grab = true;
        f.advanced.no_grab = true;
        assert!(f.argv().unwrap_err().contains("互斥"));
    }

    #[test]
    fn preview_quotes_paths_with_spaces() {
        let mut f = form(SubCmd::Dump);
        f.advanced.config = "/tmp/my dir/a.json".to_string();
        let text = f.preview("asus-numpad");
        assert!(text.contains("\"/tmp/my dir/a.json\""));
        assert!(text.ends_with("dump"));
    }

    #[test]
    fn write_commands_are_flagged() {
        assert!(!form(SubCmd::Check).writes_config());
        assert!(!form(SubCmd::Bright).writes_config());
        assert!(!form(SubCmd::Led).writes_config());
        assert!(form(SubCmd::Sku).with_sku("auto").writes_config());
        assert!(form(SubCmd::Lang).with_lang("en").writes_config());
    }

    impl CommandForm {
        fn with_sku(mut self, id: &str) -> Self {
            self.sku_id = id.to_string();
            self
        }

        fn with_lang(mut self, code: &str) -> Self {
            self.lang_code = code.to_string();
            self
        }
    }
}
