//! 自定义扩展层：自定义布局（几何 + 键表）、自定义 LED 报文、压感切换、全局热键。
//!
//! 官方布局表（[`crate::layout::LAYOUTS`]）只覆盖 INF 里出现过的机型：一旦遇到
//! 没收录的触摸板模组，或者用户想在没有镭雕的普通触摸板上「贴一张小键盘贴纸」，
//! 就只剩改源码一条路。本模块把这三件事从代码里搬进配置：
//!
//! * [`CustomLayoutSpec`]：一份布局 = 15 个几何参数（可只写与基准布局不同的项）
//!   + 4 行键表 + 可选绑定的 HID 模组 ID；
//! * [`LedProtoSpec`]：点亮报文模板，`0xff` 是「亮度字节」的占位；
//! * [`PressureSwitch`]：压感触摸板用力按压 → 切换数字键盘开关；
//! * [`HotkeySpec`]：物理键盘上的全局组合键 → 切换 / 开 / 关。
//!
//! 键表与报文都是纯数据，因此可以在非 Linux 平台跑单测。

use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};

use crate::keycodes as kc;
use crate::layout::{self, Cell, KeyMode, Layout, LayoutEntry, LayoutSupport, ROWS};

// --------------------------------------------------------------------------- //
// 键名表
// --------------------------------------------------------------------------- //

/// 字母键：内核里 `KEY_A` 与 `KEY_B` 并不连续（30 之后是 48），只能查表。
const LETTERS: [(char, u16); 26] = [
    ('A', kc::KEY_A),
    ('B', kc::KEY_B),
    ('C', kc::KEY_C),
    ('D', kc::KEY_D),
    ('E', kc::KEY_E),
    ('F', kc::KEY_F),
    ('G', kc::KEY_G),
    ('H', kc::KEY_H),
    ('I', kc::KEY_I),
    ('J', kc::KEY_J),
    ('K', kc::KEY_K),
    ('L', kc::KEY_L),
    ('M', kc::KEY_M),
    ('N', kc::KEY_N),
    ('O', kc::KEY_O),
    ('P', kc::KEY_P),
    ('Q', kc::KEY_Q),
    ('R', kc::KEY_R),
    ('S', kc::KEY_S),
    ('T', kc::KEY_T),
    ('U', kc::KEY_U),
    ('V', kc::KEY_V),
    ('W', kc::KEY_W),
    ('X', kc::KEY_X),
    ('Y', kc::KEY_Y),
    ('Z', kc::KEY_Z),
];

/// `F1..F24` 的键码表（`F1..F10` 连续，`F11` 起不连续）。
const FUNCTIONS: [u16; 24] = [
    kc::KEY_F1,
    kc::KEY_F2,
    kc::KEY_F3,
    kc::KEY_F4,
    kc::KEY_F5,
    kc::KEY_F6,
    kc::KEY_F7,
    kc::KEY_F8,
    kc::KEY_F9,
    kc::KEY_F10,
    kc::KEY_F11,
    kc::KEY_F12,
    kc::KEY_F13,
    kc::KEY_F14,
    kc::KEY_F15,
    kc::KEY_F16,
    kc::KEY_F17,
    kc::KEY_F18,
    kc::KEY_F19,
    kc::KEY_F20,
    kc::KEY_F21,
    kc::KEY_F22,
    kc::KEY_F23,
    kc::KEY_F24,
];

/// 自定义键表里可以写的主键区数字键名（`0`..`9`）。
fn digit_code(d: u8) -> Option<u16> {
    Some(match d {
        0 => kc::KEY_0,
        1 => kc::KEY_1,
        2 => kc::KEY_2,
        3 => kc::KEY_3,
        4 => kc::KEY_4,
        5 => kc::KEY_5,
        6 => kc::KEY_6,
        7 => kc::KEY_7,
        8 => kc::KEY_8,
        9 => kc::KEY_9,
        _ => return None,
    })
}

/// 键名 → Linux 键码。
///
/// 名字大小写不敏感，`_` / `-` / 空格一律忽略；`KP` 前缀表示小键盘键（`kp1` 与
/// `kp_1` 等价）。除下表列出的别名外，还接受纯文本键名（见 [`key_label`]）。
pub fn key_code(name: &str) -> Option<u16> {
    let norm: String = name
        .chars()
        .filter(|c| !matches!(c, '_' | '-' | ' '))
        .collect::<String>()
        .to_ascii_uppercase();
    if norm.is_empty() {
        return None;
    }
    // 单字符：数字 / 字母
    let mut chars = norm.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if let Some(d) = c.to_digit(10) {
            return digit_code(d as u8);
        }
        if c.is_ascii_alphabetic() {
            return LETTERS
                .iter()
                .find(|(ch, _)| *ch == c.to_ascii_uppercase())
                .map(|(_, code)| *code);
        }
    }
    // 小键盘数字：KP0..KP9
    if let Some(rest) = norm.strip_prefix("KP") {
        if rest.len() == 1 {
            if let Some(d) = rest.chars().next().and_then(|c| c.to_digit(10)) {
                return Some(match d as u8 {
                    0 => kc::KEY_KP0,
                    1 => kc::KEY_KP1,
                    2 => kc::KEY_KP2,
                    3 => kc::KEY_KP3,
                    4 => kc::KEY_KP4,
                    5 => kc::KEY_KP5,
                    6 => kc::KEY_KP6,
                    7 => kc::KEY_KP7,
                    8 => kc::KEY_KP8,
                    9 => kc::KEY_KP9,
                    _ => return None,
                });
            }
        }
    }
    let code = match norm.as_str() {
        // 小键盘符号
        "KP" | "KPDOT" | "KPPERIOD" => kc::KEY_KPDOT,
        "KPSLASH" | "KPDIV" | "KPDIVIDE" => kc::KEY_KPSLASH,
        "KPASTERISK" | "KPSTAR" | "KPMUL" => kc::KEY_KPASTERISK,
        "KPMINUS" | "KPSUB" => kc::KEY_KPMINUS,
        "KPPLUS" | "KPADD" => kc::KEY_KPPLUS,
        "KPENTER" | "KPRETURN" => kc::KEY_KPENTER,
        "KPEQUAL" => kc::KEY_KPEQUAL,
        "KPCOMMA" => kc::KEY_KPCOMMA,
        // 主键区符号与常用编辑键
        "DOT" | "PERIOD" => kc::KEY_DOT,
        "COMMA" => kc::KEY_COMMA,
        "SLASH" | "DIV" => kc::KEY_SLASH,
        "ASTERISK" | "STAR" => kc::KEY_ASTERISK,
        "MINUS" => kc::KEY_MINUS,
        "PLUS" | "EQUAL" => kc::KEY_EQUAL,
        "SPACE" => kc::KEY_SPACE,
        "TAB" => kc::KEY_TAB,
        "BACKSPACE" | "BS" => kc::KEY_BACKSPACE,
        "ENTER" | "RETURN" | "RET" => kc::KEY_ENTER,
        "ESC" | "ESCAPE" => kc::KEY_ESC,
        "DELETE" | "DEL" => kc::KEY_DELETE,
        "INSERT" | "INS" => kc::KEY_INSERT,
        "HOME" => kc::KEY_HOME,
        "END" => kc::KEY_END,
        "PAGEUP" | "PGUP" => kc::KEY_PAGEUP,
        "PAGEDOWN" | "PGDN" => kc::KEY_PAGEDOWN,
        "UP" => kc::KEY_UP,
        "DOWN" => kc::KEY_DOWN,
        "LEFT" => kc::KEY_LEFT,
        "RIGHT" => kc::KEY_RIGHT,
        // 指示灯 / 锁定键
        "NUMLOCK" | "NUMLK" => kc::KEY_NUMLOCK,
        "CAPSLOCK" | "CAPSLK" => kc::KEY_CAPSLOCK,
        "SCROLLLOCK" | "SCROLLLK" => kc::KEY_SCROLLLOCK,
        // 计算器等
        "CALC" | "CALCULATOR" => kc::KEY_CALC,
        "MAIL" => kc::KEY_MAIL,
        "HOMEPAGE" | "WWW" => kc::KEY_HOMEPAGE,
        "BACK" => kc::KEY_BACK,
        "FORWARD" => kc::KEY_FORWARD,
        "REFRESH" => kc::KEY_REFRESH,
        "SEARCH" => kc::KEY_SEARCH,
        // 媒体键
        "MUTE" => kc::KEY_MUTE,
        "VOLUMEDOWN" | "VOLDOWN" => kc::KEY_VOLUMEDOWN,
        "VOLUMEUP" | "VOLUP" => kc::KEY_VOLUMEUP,
        "PLAYPAUSE" | "PLAY" => kc::KEY_PLAYPAUSE,
        "NEXTSONG" | "NEXT" => kc::KEY_NEXTSONG,
        "PREVIOUSSONG" | "PREV" => kc::KEY_PREVIOUSSONG,
        "STOPCD" | "STOP" => kc::KEY_STOPCD,
        "BRIGHTNESSDOWN" => kc::KEY_BRIGHTNESSDOWN,
        "BRIGHTNESSUP" => kc::KEY_BRIGHTNESSUP,
        // 修饰键（也可以出现在热键组合里）
        // 不带左右的名字默认取左侧修饰键，热键写法（`ctrl+alt+n`）才够自然
        "CTRL" | "CONTROL" | "LEFTCTRL" | "LCTRL" => kc::KEY_LEFTCTRL,
        "RIGHTCTRL" | "RCTRL" => kc::KEY_RIGHTCTRL,
        "ALT" | "LEFTALT" | "LALT" => kc::KEY_LEFTALT,
        "RIGHTALT" | "RALT" | "ALTGR" => kc::KEY_RIGHTALT,
        "SHIFT" | "LEFTSHIFT" | "LSHIFT" => kc::KEY_LEFTSHIFT,
        "RIGHTSHIFT" | "RSHIFT" => kc::KEY_RIGHTSHIFT,
        "META" | "LEFTMETA" | "LMETA" | "SUPER" | "WIN" => kc::KEY_LEFTMETA,
        "RIGHTMETA" | "RMETA" => kc::KEY_RIGHTMETA,
        _ => return function_or_raw(&norm),
    };
    Some(code)
}

/// `F1..F24` 与 `0x..` / 十进制原始键码。
fn function_or_raw(norm: &str) -> Option<u16> {
    if let Some(rest) = norm.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Some(FUNCTIONS[(n - 1) as usize]);
            }
        }
    }
    let lower = norm.to_ascii_lowercase();
    if let Some(hex) = lower.strip_prefix("0x") {
        return u16::from_str_radix(hex, 16).ok().filter(|c| *c > 0);
    }
    if norm.chars().all(|c| c.is_ascii_digit()) {
        return norm.parse::<u16>().ok().filter(|c| *c > 0);
    }
    None
}

/// 键码 → 便于人读的名字（写回配置时用得到，日志里也用它）。
pub fn key_label(code: u16) -> String {
    let named = [
        (kc::KEY_KP0, "kp0"),
        (kc::KEY_KP1, "kp1"),
        (kc::KEY_KP2, "kp2"),
        (kc::KEY_KP3, "kp3"),
        (kc::KEY_KP4, "kp4"),
        (kc::KEY_KP5, "kp5"),
        (kc::KEY_KP6, "kp6"),
        (kc::KEY_KP7, "kp7"),
        (kc::KEY_KP8, "kp8"),
        (kc::KEY_KP9, "kp9"),
        (kc::KEY_KPDOT, "kpdot"),
        (kc::KEY_KPSLASH, "kpslash"),
        (kc::KEY_KPASTERISK, "kpasterisk"),
        (kc::KEY_KPMINUS, "kpminus"),
        (kc::KEY_KPPLUS, "kpplus"),
        (kc::KEY_KPENTER, "kpenter"),
        (kc::KEY_BACKSPACE, "backspace"),
        (kc::KEY_ENTER, "enter"),
        (kc::KEY_DOT, "dot"),
        (kc::KEY_SPACE, "space"),
        (kc::KEY_TAB, "tab"),
        (kc::KEY_ESC, "esc"),
        (kc::KEY_DELETE, "delete"),
        (kc::KEY_NUMLOCK, "numlock"),
        (kc::KEY_CALC, "calc"),
    ];
    for (c, name) in named {
        if c == code {
            return name.to_string();
        }
    }
    if (kc::KEY_0..=kc::KEY_9).contains(&code) {
        return (code - kc::KEY_0).to_string();
    }
    if let Some((ch, _)) = LETTERS.iter().find(|(_, c)| *c == code) {
        return ch.to_ascii_lowercase().to_string();
    }
    if let Some(pos) = FUNCTIONS.iter().position(|c| *c == code) {
        return format!("F{}", pos + 1);
    }
    let ascii = kc::name_of(code);
    if ascii != "?" && ascii != "UNKNOWN" {
        return ascii.to_ascii_lowercase();
    }
    format!("0x{code:02x}")
}

// --------------------------------------------------------------------------- //
// 键表
// --------------------------------------------------------------------------- //

/// 一格里放什么：无键 / 内置符号（跟随 `key_mode` 切换主键区或小键盘）/ 固定键码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySlot {
    None,
    Builtin(Cell),
    Code(u16),
}

impl KeySlot {
    pub fn key(self, mode: KeyMode) -> Option<u16> {
        match self {
            KeySlot::None => None,
            KeySlot::Builtin(cell) => layout::key_for_cell(cell, mode),
            KeySlot::Code(code) => Some(code),
        }
    }

    /// 这一格的短名，供 `describe` 把键表打印成又一种可以再喂回 `parse` 的文本。
    ///
    /// 空位、小数点、减号在键盘上都只是一个符号（`.` `-`），表里却要区分三样东西：
    /// 空位不出键、小数点是 `period`、减号是 `minus`。所以这三格印英文名，其余照键盘
    /// 上印的写——`describe` 的结果因此总能原样解析回去，GUI 里改一格不会顺带改掉别人。
    pub fn label(self) -> String {
        match self {
            KeySlot::None => "none".to_string(),
            KeySlot::Builtin(Cell::Period) => "period".to_string(),
            KeySlot::Builtin(Cell::Minus) => "minus".to_string(),
            KeySlot::Builtin(cell) => layout::cell_label(cell).to_string(),
            KeySlot::Code(code) => key_label(code),
        }
    }
}

/// 4 行键表。列数由表长决定（1..=8），行数固定 [`ROWS`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyTable {
    rows: Vec<Vec<KeySlot>>,
    cols: usize,
}

/// 内置键表（`KEY_GRID` 的运行时形态），进程内只构造一次。
pub fn builtin_table() -> &'static KeyTable {
    static BUILTIN: OnceLock<KeyTable> = OnceLock::new();
    BUILTIN.get_or_init(|| {
        let rows = layout::KEY_GRID
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| match cell {
                        Some(c) => KeySlot::Builtin(*c),
                        None => KeySlot::None,
                    })
                    .collect()
            })
            .collect();
        KeyTable {
            rows,
            cols: layout::COLS,
        }
    })
}

impl KeyTable {
    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> &[Vec<KeySlot>] {
        &self.rows
    }

    pub fn get(&self, row: usize, col: usize) -> Option<KeySlot> {
        self.rows.get(row)?.get(col).copied()
    }

    pub fn key_at(&self, row: usize, col: usize, mode: KeyMode) -> Option<u16> {
        self.get(row, col)?.key(mode)
    }

    /// 解析配置里的 4 行文本；`Err` 里带出行号，方便定位写错的那一行。
    ///
    /// 每格的写法（大小写不敏感）：
    /// * `.` / `-` / `none`：空位，不出键；
    /// * `7` `dot` `slash` `asterisk` `minus` `plus` `backspace` `enter`：
    ///   内置符号，跟随 `key_mode`（keypad / digits）自动切换；
    ///
    /// 符号键也可以直接照键盘上印的样子写：`+` `*` `/` 与 `plus` `asterisk` `slash`
    /// 等价。只有 `-` 例外——它早就被当成空位（与 [`Self::describe`] 打印空位的写法
    /// 对齐），减号得写 `minus`；小数点写 `period`（`dot` 是主键盘上那个固定键码）。
    /// * `kp7` `F13` `playpause` `volup` 等：固定键码，见 [`key_code`]；
    /// * `0x64`：直接写 Linux 键码（十进制或十六进制）。
    pub fn parse(rows: &[String]) -> Result<Self, String> {
        if rows.is_empty() || rows.len() > ROWS {
            return Err(format!("rows: need 1..={ROWS} lines, got {}", rows.len()));
        }
        let mut table: Vec<Vec<KeySlot>> = Vec::with_capacity(rows.len());
        let mut cols = 0usize;
        for (r, line) in rows.iter().enumerate() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.is_empty() {
                return Err(format!("row {}: empty", r + 1));
            }
            if r == 0 {
                cols = tokens.len();
                if !(1..=8).contains(&cols) {
                    return Err(format!("row 1: {} columns, need 1..=8", cols));
                }
            } else if tokens.len() != cols {
                return Err(format!(
                    "row {}: {} columns, row 1 has {}",
                    r + 1,
                    tokens.len(),
                    cols
                ));
            }
            let mut row = Vec::with_capacity(cols);
            for (c, tok) in tokens.iter().enumerate() {
                row.push(
                    parse_slot(tok).map_err(|e| format!("row {}, col {}: {e}", r + 1, c + 1))?,
                );
            }
            table.push(row);
        }
        // 不足 4 行时补空行：Geometry 固定按 4 行切分网格
        while table.len() < ROWS {
            table.push(vec![KeySlot::None; cols]);
        }
        Ok(KeyTable { rows: table, cols })
    }

    /// 供 CLI / GUI 打印的网格文本（每格一个短名）。
    pub fn describe(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|row| row.iter().map(|s| s.label()).collect::<Vec<_>>().join(" "))
            .collect()
    }
}

fn parse_slot(tok: &str) -> Result<KeySlot, String> {
    let t = tok.trim();
    if t.is_empty() || t == "." || t == "-" || t.eq_ignore_ascii_case("none") {
        return Ok(KeySlot::None);
    }
    // 内置符号：与 key_mode 联动
    // 符号键两种写法都收：写 `plus` 或直接写键盘上那个 `+`，用户不该为了对齐去查文档。
    // 只把 `-` 留给空位（`describe` 打印空位也用 `.`），减号因此仍要写 `minus`。
    let builtin = match t.to_ascii_lowercase().as_str() {
        "period" => Some(Cell::Period),
        "backspace" | "bs" => Some(Cell::Backspace),
        "return" | "ret" | "enter" => Some(Cell::Return),
        "slash" | "div" | "/" => Some(Cell::Slash),
        "asterisk" | "star" | "mul" | "*" => Some(Cell::Asterisk),
        "minus" | "sub" => Some(Cell::Minus),
        "plus" | "add" | "+" => Some(Cell::Plus),
        _ => None,
    };
    if let Some(cell) = builtin {
        return Ok(KeySlot::Builtin(cell));
    }
    if let Some(d) = t.parse::<u8>().ok().filter(|d| *d <= 9) {
        return Ok(KeySlot::Builtin(Cell::Digit(d)));
    }
    if let Some(code) = key_code(t) {
        return Ok(KeySlot::Code(code));
    }
    Err(format!("unknown key `{t}`"))
}

// --------------------------------------------------------------------------- //
// 自定义布局
// --------------------------------------------------------------------------- //

/// 15 个官方布局参数的可选覆盖（未写的项继承 `base`）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LayoutPatch {
    pub columns: Option<i32>,
    pub index0: Option<i32>,
    pub index4: Option<i32>,
    pub level0: Option<i32>,
    pub level3: Option<i32>,
    pub numlock_index: Option<i32>,
    pub numlock_level: Option<i32>,
    pub left: Option<i32>,
    pub right: Option<i32>,
    pub up: Option<i32>,
    pub down: Option<i32>,
    pub numadv_index: Option<i32>,
    pub numadv_level: Option<i32>,
    pub invalid_index: Option<i32>,
    pub invalid_level: Option<i32>,
}

impl LayoutPatch {
    /// 把已写的项覆盖到 `base` 上。
    pub fn apply(&self, base: Layout) -> Layout {
        let mut l = base;
        macro_rules! take {
            ($($f:ident),*) => {$( if let Some(v) = self.$f { l.$f = v; } )*};
        }
        take!(
            columns,
            index0,
            index4,
            level0,
            level3,
            numlock_index,
            numlock_level,
            left,
            right,
            up,
            down,
            numadv_index,
            numadv_level,
            invalid_index,
            invalid_level
        );
        l
    }
}

/// 配置里的一份自定义布局。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct CustomLayoutSpec {
    /// 布局名（写进 `layout_name` 就能选中它）。
    pub name: String,
    /// 参数基准：内置布局名（如 `GH3XLNDZ`）或上一条自定义布局。
    pub base: String,
    /// 绑定的触摸板 HID 模组 ID（如 `ASUE1416`）；`auto` 时命中即自动选用。
    pub hid_ids: Vec<String>,
    /// 4 行键表；留空 = 沿用基准布局的键表。
    pub keys: Vec<String>,
    #[serde(flatten)]
    pub patch: LayoutPatch,
}

impl Default for CustomLayoutSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            base: layout::DEFAULT_LAYOUT_NAME.to_string(),
            hid_ids: Vec::new(),
            keys: Vec::new(),
            patch: LayoutPatch::default(),
        }
    }
}

/// 已注册的自定义布局：Leaked 成 `'static`，好让 `Geometry` / `LayoutEntry` 的
/// 现有 `&'static str` 接口原样复用。
pub struct CustomLayout {
    pub entry: &'static LayoutEntry,
    pub table: KeyTable,
    pub hid_ids: Vec<String>,
}

fn registry() -> &'static RwLock<Vec<CustomLayout>> {
    static REG: OnceLock<RwLock<Vec<CustomLayout>>> = OnceLock::new();
    REG.get_or_init(|| RwLock::new(Vec::new()))
}

/// 注册全部自定义布局（进程启动时调一次）；返回给用户看的提示。
///
/// 两阶段：先在**不持锁**的状态下把每个 spec 解析成 [`CustomLayout`]（`base` 既可
/// 指向内置布局，也可指向同一份配置里排在前面的另一条自定义布局），最后一次性
/// 提交进全局表。先持锁再解析会踩到「写锁持有期间调用 `layout::by_name` 又要读锁」
/// 的重入死锁——`std::sync::RwLock` 不可重入。
pub fn install_layouts(specs: &[CustomLayoutSpec]) -> Vec<String> {
    // 先清空：每次安装都是「配置文件里写了什么就是什么」
    {
        let mut reg = match registry().write() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        reg.clear();
    }
    let mut notes = Vec::new();
    if specs.is_empty() {
        return notes;
    }
    let mut pending: Vec<CustomLayout> = Vec::new();
    for spec in specs {
        let name = spec.name.trim();
        if name.is_empty() {
            notes.push(crate::i18n::t("note.custom_layout_no_name"));
            continue;
        }
        if layout::builtin_by_name(name).is_some() {
            notes.push(crate::i18n::tf(
                "note.custom_layout_builtin_clash",
                &[("name", name)],
            ));
            continue;
        }
        let base = layout::builtin_by_name(&spec.base).or_else(|| {
            pending
                .iter()
                .find(|c| c.entry.name.eq_ignore_ascii_case(spec.base.trim()))
                .map(|c| c.entry)
        });
        let Some(base) = base else {
            notes.push(crate::i18n::tf(
                "note.custom_layout_base_missing",
                &[("name", name), ("base", spec.base.trim())],
            ));
            continue;
        };
        let mut patch = spec.patch;
        let table = match KeyTable::parse(&spec.keys) {
            Ok(t) => {
                // 列数以键表为准：贴纸方案天然由贴纸的列数决定
                patch.columns = Some(t.cols() as i32);
                t
            }
            Err(err) => {
                notes.push(crate::i18n::tf(
                    "note.custom_layout_bad_keys",
                    &[("name", name), ("err", &err)],
                ));
                continue;
            }
        };
        let layout = patch.apply(base.layout);
        if layout.columns < 1 || layout.columns > 8 {
            notes.push(crate::i18n::tf(
                "note.custom_layout_bad_columns",
                &[("name", name), ("n", &layout.columns.to_string())],
            ));
            continue;
        }
        let leaked_name: &'static str = Box::leak(name.to_string().into_boxed_str());
        let entry: &'static LayoutEntry = Box::leak(Box::new(LayoutEntry {
            name: leaked_name,
            layout,
            support: LayoutSupport::Verified,
        }));
        let hid_ids: Vec<String> = spec
            .hid_ids
            .iter()
            .map(|s| s.trim().to_ascii_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        for hid in &hid_ids {
            if let Some(builtin) = layout::builtin_for_hid_id(hid) {
                notes.push(crate::i18n::tf(
                    "note.custom_hid_override",
                    &[("hid", hid), ("name", name), ("builtin", builtin.name)],
                ));
            }
        }
        pending.push(CustomLayout {
            entry,
            table,
            hid_ids,
        });
    }
    let mut reg = match registry().write() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    *reg = pending;
    notes
}

fn read_registry<T>(f: impl FnOnce(&[CustomLayout]) -> T) -> Option<T> {
    let reg = registry().read().ok()?;
    if reg.is_empty() {
        return None;
    }
    Some(f(&reg))
}

/// 按名字查自定义布局。
pub fn entry_by_name(name: &str) -> Option<&'static LayoutEntry> {
    read_registry(|reg| {
        reg.iter()
            .find(|c| c.entry.name.eq_ignore_ascii_case(name))
            .map(|c| c.entry)
    })?
}

/// 按触摸板 HID 模组 ID 查自定义布局。
pub fn entry_by_hid(hid: &str) -> Option<&'static LayoutEntry> {
    let wanted = hid.trim().to_ascii_uppercase();
    read_registry(|reg| {
        reg.iter()
            .find(|c| c.hid_ids.contains(&wanted))
            .map(|c| c.entry)
    })?
}

/// 当前配置下应该用哪张键表：命中自定义布局用它的，否则用内置表。
pub fn table_for(layout_name: &str) -> KeyTable {
    let found: Option<KeyTable> = read_registry(|reg| {
        reg.iter()
            .find(|c| c.entry.name.eq_ignore_ascii_case(layout_name))
            .map(|c| c.table.clone())
    })
    .flatten();
    if let Some(table) = found {
        return table.flatten();
    }
    builtin_table().clone()
}

impl KeyTable {
    /// 补齐为固定 4 行（`Geometry` 始终按 4 行切分网格）。
    fn flatten(mut self) -> Self {
        while self.rows.len() < ROWS {
            self.rows.push(vec![KeySlot::None; self.cols]);
        }
        self
    }
}

/// 已注册的自定义布局一览（CLI / GUI 展示用）：`(名字, 绑定 HID, 网格文本)`。
pub fn list_custom() -> Vec<(String, Vec<String>, Vec<String>)> {
    read_registry(|reg| {
        reg.iter()
            .map(|c| {
                (
                    c.entry.name.to_string(),
                    c.hid_ids.clone(),
                    c.table.describe(),
                )
            })
            .collect()
    })
    .unwrap_or_default()
}

// --------------------------------------------------------------------------- //
// 自定义点亮报文
// --------------------------------------------------------------------------- //

/// 报文模板里的「亮度字节」占位符。
pub const LEVEL_MARK: u8 = 0xff;

/// 点亮报文模板：`0xff` 换成实际亮度字节，其余字节原样下发。
///
/// 默认就是 NumberPad 的 FEATURE 报告 `0d 14 03 ff ad`。换成别的机型/别的
/// 触摸板时，照着它的 HID 报告描述符把固定部分抄进 `template`，把亮度所在
/// 位置写成 `255` 即可。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LedProtoSpec {
    pub template: Vec<u8>,
}

impl Default for LedProtoSpec {
    fn default() -> Self {
        Self {
            template: crate::led_proto::encode(0)
                .map(|b| {
                    let mut v = b.to_vec();
                    v[3] = LEVEL_MARK;
                    v
                })
                .unwrap_or_default(),
        }
    }
}

impl LedProtoSpec {
    /// 是否就是内置报文（是的话走原来的快路径，不多一次拼接）。
    pub fn is_builtin(&self) -> bool {
        self.template.len() == crate::led_proto::LED_FEATURE_LEN
            && self.template[..3]
                == [
                    crate::led_proto::LED_FEATURE_REPORT_ID,
                    crate::led_proto::LED_FEATURE_BODY[0],
                    crate::led_proto::LED_FEATURE_BODY[1],
                ]
            && self.template[3] == LEVEL_MARK
            && self.template[4] == crate::led_proto::LED_FEATURE_TAIL
    }

    /// 结构校验：长度、占位符个数、首字节（report id）。
    pub fn validate(&self) -> Result<(), String> {
        if self.template.is_empty() {
            return Err("template is empty".to_string());
        }
        if self.template.len() > 64 {
            return Err(format!(
                "template has {} bytes, max 64",
                self.template.len()
            ));
        }
        let marks = self.template.iter().filter(|b| **b == LEVEL_MARK).count();
        if marks != 1 {
            return Err(format!(
                "template needs exactly one {LEVEL_MARK} (level placeholder), found {marks}"
            ));
        }
        if self.template[0] == LEVEL_MARK {
            return Err("template byte 1 is the report id, cannot be the level".to_string());
        }
        Ok(())
    }

    /// 生成待写入报文；`0x60`（i2c 解锁字）在 FEATURE 通道无意义。
    pub fn encode(&self, value: u8) -> Option<Vec<u8>> {
        let level = crate::led_proto::level_byte(value)?;
        let mut out = self.template.clone();
        for b in out.iter_mut() {
            if *b == LEVEL_MARK {
                *b = level;
            }
        }
        Some(out)
    }
}

// --------------------------------------------------------------------------- //
// 压感切换
// --------------------------------------------------------------------------- //

/// 压感触摸板的「用力按 = 切换」配置。
///
/// 只有上报 `ABS_PRESSURE` / `ABS_MT_PRESSURE` 的模组才谈得上压感；没有压力轴
/// 的机型开了本项不会有任何效果（也不会报错），所以默认关闭。
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PressureSwitch {
    pub enabled: bool,
    /// 触发阈值：压力轴满量程的百分比（1..=100）。
    pub percent: u8,
    /// 需要连续保持多少帧（一帧 = `frame_ms`，默认 8 ms）。
    pub frames: u32,
    /// 两次切换之间的最小间隔（ms），防止按住不放连续翻转。
    pub cooldown_ms: f64,
}

impl Default for PressureSwitch {
    fn default() -> Self {
        Self {
            enabled: false,
            percent: 80,
            frames: 6,
            cooldown_ms: 600.0,
        }
    }
}

impl PressureSwitch {
    pub fn sanitized(mut self) -> Self {
        self.percent = self.percent.clamp(1, 100);
        self.frames = self.frames.clamp(1, 200);
        if !(self.cooldown_ms.is_finite() && self.cooldown_ms >= 0.0) {
            self.cooldown_ms = 600.0;
        }
        self
    }
}

/// 压感切换状态机（纯逻辑，可单测）。
#[derive(Debug, Clone)]
pub struct PressureGate {
    cfg: PressureSwitch,
    run: u32,
    armed: bool,
    last_ms: f64,
    fired: u64,
}

impl PressureGate {
    pub fn new(cfg: PressureSwitch) -> Self {
        Self {
            cfg: cfg.sanitized(),
            run: 0,
            armed: true,
            last_ms: f64::NEG_INFINITY,
            fired: 0,
        }
    }

    pub fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    pub fn config(&self) -> PressureSwitch {
        self.cfg
    }

    /// 累计触发次数（测试与日志用）。
    pub fn fired(&self) -> u64 {
        self.fired
    }

    /// 喂一个压力采样，返回是否应当在本次触发切换。
    ///
    /// * `value` 压力原始值，`max` 该轴的 `maximum`（<=0 表示没有压力轴）；
    /// * `contact` 当前是否有触点，抬起即清零；
    /// * 触发后必须等压力回落到阈值以下才会重新武装，避免一次长按翻转多次。
    pub fn feed(&mut self, value: i32, max: i32, contact: bool, now_ms: f64) -> bool {
        if !self.cfg.enabled || max <= 0 || !contact {
            self.run = 0;
            self.armed = true;
            return false;
        }
        let threshold = ((max as i64 * self.cfg.percent as i64) / 100).max(1) as i32;
        if value < threshold {
            self.run = 0;
            self.armed = true;
            return false;
        }
        if !self.armed {
            return false;
        }
        self.run += 1;
        if self.run < self.cfg.frames {
            return false;
        }
        if now_ms - self.last_ms < self.cfg.cooldown_ms {
            return false;
        }
        self.run = 0;
        self.armed = false;
        self.last_ms = now_ms;
        self.fired += 1;
        true
    }
}

// --------------------------------------------------------------------------- //
// 全局热键
// --------------------------------------------------------------------------- //

pub const MOD_CTRL: u8 = 1;
pub const MOD_ALT: u8 = 2;
pub const MOD_SHIFT: u8 = 4;
pub const MOD_SUPER: u8 = 8;

fn mod_of(code: u16) -> Option<u8> {
    Some(match code {
        kc::KEY_LEFTCTRL | kc::KEY_RIGHTCTRL => MOD_CTRL,
        kc::KEY_LEFTALT | kc::KEY_RIGHTALT => MOD_ALT,
        kc::KEY_LEFTSHIFT | kc::KEY_RIGHTSHIFT => MOD_SHIFT,
        kc::KEY_LEFTMETA | kc::KEY_RIGHTMETA => MOD_SUPER,
        _ => return None,
    })
}

/// 一条组合键：修饰键掩码 + 主键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub mods: u8,
    pub code: u16,
}

impl Hotkey {
    /// 解析 `ctrl+alt+n`、`super+F13`、`shift+kpenter` 这类写法。
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut mods = 0u8;
        let mut main: Option<u16> = None;
        for part in text.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()) {
            let Some(code) = key_code(part) else {
                return Err(format!("unknown key `{part}`"));
            };
            if let Some(m) = mod_of(code) {
                mods |= m;
                continue;
            }
            if let Some(prev) = main {
                if prev != code {
                    return Err(format!("`{text}` has two main keys"));
                }
            }
            main = Some(code);
        }
        let Some(code) = main else {
            return Err(format!("`{text}` has no main key"));
        };
        Ok(Hotkey { mods, code })
    }

    /// 规整写法（`ctrl+alt+n` 形式），写回配置时用。
    pub fn label(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.mods & MOD_CTRL != 0 {
            parts.push("ctrl");
        }
        if self.mods & MOD_ALT != 0 {
            parts.push("alt");
        }
        if self.mods & MOD_SHIFT != 0 {
            parts.push("shift");
        }
        if self.mods & MOD_SUPER != 0 {
            parts.push("super");
        }
        let mut out = parts.join("+");
        if !out.is_empty() {
            out.push('+');
        }
        out.push_str(&key_label(self.code));
        out
    }
}

/// 热键要执行的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    /// 切换数字键盘开关
    Toggle,
    /// 强制开启（不动键盘 LED 状态时也点亮）
    On,
    /// 强制关闭
    Off,
}

impl HotkeyAction {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim().to_ascii_lowercase().as_str() {
            "" | "toggle" => Ok(HotkeyAction::Toggle),
            "on" | "enable" => Ok(HotkeyAction::On),
            "off" | "disable" => Ok(HotkeyAction::Off),
            other => Err(format!("unknown action `{other}`")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            HotkeyAction::Toggle => "toggle",
            HotkeyAction::On => "on",
            HotkeyAction::Off => "off",
        }
    }
}

/// 配置里的一条热键绑定。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HotkeySpec {
    /// 组合键写法，如 `ctrl+alt+n`。
    pub gesture: String,
    /// 动作：`toggle`（默认）/ `on` / `off`。
    pub action: String,
}

impl Default for HotkeySpec {
    fn default() -> Self {
        Self {
            gesture: "ctrl+alt+n".to_string(),
            action: "toggle".to_string(),
        }
    }
}

/// 热键匹配器：喂物理键盘的 `EV_KEY` 事件，命中时给出动作名。
#[derive(Debug, Default)]
pub struct HotkeyState {
    binds: Vec<(Hotkey, HotkeyAction)>,
    down: HashSet<u16>,
}

impl HotkeyState {
    /// 从配置构造；解析失败的行返回提示并跳过。
    pub fn new(specs: &[HotkeySpec]) -> (Self, Vec<String>) {
        let mut binds = Vec::new();
        let mut notes = Vec::new();
        for spec in specs {
            if spec.gesture.trim().is_empty() {
                continue;
            }
            match Hotkey::parse(&spec.gesture) {
                Ok(hk) => match HotkeyAction::parse(&spec.action) {
                    Ok(action) => binds.push((hk, action)),
                    Err(err) => notes.push(crate::i18n::tf(
                        "note.hotkey_bad",
                        &[("spec", spec.gesture.trim()), ("err", &err)],
                    )),
                },
                Err(err) => notes.push(crate::i18n::tf(
                    "note.hotkey_bad",
                    &[("spec", spec.gesture.trim()), ("err", &err)],
                )),
            }
        }
        (
            Self {
                binds,
                down: HashSet::new(),
            },
            notes,
        )
    }

    pub fn is_empty(&self) -> bool {
        self.binds.is_empty()
    }

    /// 绑定一览（日志/GUI 用）。
    pub fn describe(&self) -> Vec<String> {
        self.binds
            .iter()
            .map(|(hk, action)| format!("{} -> {}", hk.label(), action.as_str()))
            .collect()
    }

    /// 喂一个按键事件（`value`：1 按下 / 0 抬起 / 2 长按重复）。
    ///
    /// 只在「按下」那一刻判定一次；主键按住不放不会重复触发，必须抬起再按。
    pub fn feed(&mut self, code: u16, value: i32) -> Option<HotkeyAction> {
        if value == 0 {
            self.down.remove(&code);
            return None;
        }
        if value != 1 {
            return None;
        }
        self.down.insert(code);
        let mods_now = self
            .down
            .iter()
            .filter_map(|c| mod_of(*c))
            .fold(0u8, |a, b| a | b);
        for (hk, action) in &self.binds {
            if hk.code == code && hk.mods == mods_now {
                return Some(*action);
            }
        }
        None
    }

    /// 键状态复位（例如切 tty 后残留按下态）。
    pub fn clear(&mut self) {
        self.down.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::GH3XLNDZ;

    #[test]
    fn key_code_aliases() {
        assert_eq!(key_code("7"), Some(kc::KEY_7));
        assert_eq!(key_code("kp7"), Some(kc::KEY_KP7));
        assert_eq!(key_code("KP_7"), Some(kc::KEY_KP7));
        assert_eq!(key_code("kp-dot"), Some(kc::KEY_KPDOT));
        assert_eq!(key_code("F13"), Some(kc::KEY_F13));
        assert_eq!(key_code("backspace"), Some(kc::KEY_BACKSPACE));
        assert_eq!(key_code("volup"), Some(kc::KEY_VOLUMEUP));
        assert_eq!(key_code("0x64"), Some(0x64));
        assert_eq!(key_code("296"), Some(296));
        assert_eq!(key_code("nope"), None);
    }

    #[test]
    fn key_label_roundtrip() {
        for code in [
            kc::KEY_KP7,
            kc::KEY_KPENTER,
            kc::KEY_BACKSPACE,
            kc::KEY_F13,
            kc::KEY_7,
            0x123,
        ] {
            let label = key_label(code);
            let back = key_code(&label).unwrap_or_else(|| {
                // F13 / 数字 / kp7 都能解回；未知键码走 `0x..` 形式
                assert!(label.starts_with("0x"), "{label}");
                u16::from_str_radix(&label[2..], 16).unwrap()
            });
            assert_eq!(back, code, "label {label}");
        }
    }

    #[test]
    fn keytable_parse_and_lookup() {
        const BASE: Layout = GH3XLNDZ;
        let rows = vec![
            "7 8 9 slash .".to_string(),
            "4 5 6 asterisk backspace".to_string(),
            "1 2 3 minus enter".to_string(),
            "0 0 period plus enter".to_string(),
        ];
        let t = KeyTable::parse(&rows).unwrap();
        assert_eq!(t.cols(), 5);
        assert_eq!(t.key_at(0, 0, KeyMode::Keypad), Some(kc::KEY_KP7));
        assert_eq!(t.key_at(0, 0, KeyMode::Digits), Some(kc::KEY_7));
        assert_eq!(t.key_at(2, 4, KeyMode::Keypad), Some(kc::KEY_KPENTER));
        // 内置表与手写同构表在关键格上一致
        let builtin = builtin_table();
        for row in 0..ROWS {
            for col in 0..builtin.cols() {
                assert_eq!(
                    builtin.key_at(row, col, KeyMode::Keypad),
                    t.key_at(row, col, KeyMode::Keypad),
                    "cell ({row},{col})"
                );
            }
        }
        let _ = BASE;
    }

    #[test]
    fn keytable_accepts_fixed_codes_and_gaps() {
        let t = KeyTable::parse(&[
            "kp1 kp2 kp3 . F13".to_string(),
            "kp4 kp5 kp6 . 0xAA".to_string(),
        ])
        .unwrap();
        assert_eq!(t.cols(), 5);
        assert_eq!(t.key_at(0, 0, KeyMode::Digits), Some(kc::KEY_KP1));
        assert_eq!(t.key_at(0, 3, KeyMode::Keypad), None);
        assert_eq!(t.key_at(0, 4, KeyMode::Keypad), Some(kc::KEY_F13));
        assert_eq!(t.key_at(1, 4, KeyMode::Keypad), Some(0xAA));
        // 不足 4 行的补齐为空行
        assert_eq!(t.key_at(3, 0, KeyMode::Keypad), None);
    }

    /// 符号键允许照键盘上的样子直接写：`+` `*` `/`，省得用户先去查一遍英文名。
    /// `-` 仍是空位（与 `describe` 打印空位一致），减号只能写 `minus`。
    #[test]
    fn keytable_accepts_printed_symbols() {
        let t = KeyTable::parse(&[
            "7 8 9 +".to_string(),
            "4 5 6 *".to_string(),
            "1 2 3 /".to_string(),
            "0 . minus -".to_string(),
        ])
        .unwrap();
        assert_eq!(t.key_at(0, 3, KeyMode::Keypad), Some(kc::KEY_KPPLUS));
        assert_eq!(t.key_at(1, 3, KeyMode::Keypad), Some(kc::KEY_KPASTERISK));
        assert_eq!(t.key_at(2, 3, KeyMode::Keypad), Some(kc::KEY_KPSLASH));
        assert_eq!(t.key_at(3, 2, KeyMode::Keypad), Some(kc::KEY_KPMINUS));
        // `-` 与 `.` 都当空位
        assert_eq!(t.key_at(3, 3, KeyMode::Keypad), None);
        assert_eq!(t.key_at(3, 1, KeyMode::Keypad), None);
        // 打印回来仍是英文名，能再解析回去
        let again = KeyTable::parse(&t.describe()).unwrap();
        assert_eq!(again, t);
    }

    #[test]
    fn keytable_rejects_bad_input() {
        assert!(KeyTable::parse(&[]).is_err());
        assert!(KeyTable::parse(&["1 2".into(), "1 2 3".into()]).is_err());
        assert!(KeyTable::parse(&["1 bogus 3".into()]).is_err());
        assert!(KeyTable::parse(&["1 2 3 4 5 6 7 8 9".into()]).is_err());
    }

    #[test]
    fn layout_patch_overrides_only_written_fields() {
        let patch = LayoutPatch {
            columns: Some(4),
            left: Some(20),
            ..Default::default()
        };
        let l = patch.apply(GH3XLNDZ);
        assert_eq!(l.columns, 4);
        assert_eq!(l.left, 20);
        assert_eq!(l.index0, GH3XLNDZ.index0);
    }

    #[test]
    fn custom_layout_registers_and_resolves() {
        let spec = CustomLayoutSpec {
            name: "PASTE40".to_string(),
            hid_ids: vec!["asue9999".to_string()],
            keys: vec![
                "7 8 9 slash".to_string(),
                "4 5 6 asterisk".to_string(),
                "1 2 3 minus".to_string(),
                "0 period plus enter".to_string(),
            ],
            patch: LayoutPatch {
                columns: Some(4),
                ..Default::default()
            },
            ..Default::default()
        };
        let notes = install_layouts(&[spec]);
        assert!(notes.iter().all(|n| !n.contains("ignored")), "{notes:?}");
        let entry = entry_by_name("PASTE40").expect("registered");
        assert_eq!(entry.layout.columns, 4);
        assert_eq!(entry_by_hid("ASUE9999").map(|e| e.name), Some("PASTE40"));
        let table = table_for("PASTE40");
        assert_eq!(table.cols(), 4);
        assert_eq!(table.key_at(0, 0, KeyMode::Keypad), Some(kc::KEY_KP7));
        // 内置布局仍然解析到静态表
        assert_eq!(table_for("GH3XLNDZ").cols(), 5);
        assert!(layout::by_name("PASTE40").is_some());
    }

    #[test]
    fn custom_layout_rejects_builtin_name() {
        let notes = install_layouts(&[CustomLayoutSpec {
            name: "GH3XLNDZ".to_string(),
            keys: vec!["1 2 3 4 5".to_string()],
            ..Default::default()
        }]);
        // 文案走 i18n（默认英文），断言用文案里的关键词而不是整句
        assert!(
            notes
                .iter()
                .any(|n| n.contains("clashes") && n.contains("GH3XLNDZ")),
            "{notes:?}"
        );
    }

    #[test]
    fn proto_default_matches_builtin_report() {
        let spec = LedProtoSpec::default();
        assert!(spec.is_builtin());
        assert!(spec.validate().is_ok());
        assert_eq!(
            spec.encode(0x45),
            crate::led_proto::encode(0x45).map(|b| b.to_vec())
        );
        assert_eq!(spec.encode(0x60), None);
    }

    #[test]
    fn proto_custom_template() {
        let spec = LedProtoSpec {
            template: vec![0x0a, 0x01, LEVEL_MARK, 0x00],
        };
        assert!(!spec.is_builtin());
        assert!(spec.validate().is_ok());
        assert_eq!(spec.encode(0x43), Some(vec![0x0a, 0x01, 0x43, 0x00]));
        assert!(LedProtoSpec {
            template: vec![0x0a, 0x01, 0x42]
        }
        .validate()
        .is_err());
        assert!(LedProtoSpec {
            template: vec![LEVEL_MARK, LEVEL_MARK]
        }
        .validate()
        .is_err());
    }

    #[test]
    fn pressure_gate_needs_frames_and_rearms() {
        let mut gate = PressureGate::new(PressureSwitch {
            enabled: true,
            percent: 80,
            frames: 3,
            cooldown_ms: 500.0,
        });
        let thr = 800; // max 1000 * 80%
        assert!(!gate.feed(thr + 10, 1000, true, 0.0));
        assert!(!gate.feed(thr + 10, 1000, true, 8.0));
        assert!(gate.feed(thr + 10, 1000, true, 16.0));
        // 按住不放不会再触发
        assert!(!gate.feed(thr + 10, 1000, true, 24.0));
        assert!(!gate.feed(thr + 10, 1000, true, 1000.0));
        // 松开压力后重新武装，冷却后再触发
        assert!(!gate.feed(0, 1000, true, 1010.0));
        assert!(!gate.feed(thr + 10, 1000, true, 1018.0));
        assert!(!gate.feed(thr + 10, 1000, true, 1026.0));
        assert!(gate.feed(thr + 10, 1000, true, 1034.0));
        assert_eq!(gate.fired(), 2);
    }

    #[test]
    fn pressure_gate_ignores_pressureless_panels() {
        let mut gate = PressureGate::new(PressureSwitch {
            enabled: true,
            ..Default::default()
        });
        assert!(!gate.feed(9999, 0, true, 0.0));
        let mut off = PressureGate::new(PressureSwitch::default());
        assert!(!off.feed(9999, 1000, true, 0.0));
    }

    #[test]
    fn hotkey_parse_and_match() {
        let hk = Hotkey::parse("ctrl+alt+n").unwrap();
        assert_eq!(hk.mods, MOD_CTRL | MOD_ALT);
        assert_eq!(hk.code, kc::KEY_N);
        assert_eq!(hk.label(), "ctrl+alt+n");
        let (mut state, notes) = HotkeyState::new(&[HotkeySpec {
            gesture: "ctrl+alt+n".to_string(),
            action: "toggle".to_string(),
        }]);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(state.feed(kc::KEY_LEFTCTRL, 1), None);
        assert_eq!(state.feed(kc::KEY_LEFTALT, 1), None);
        assert_eq!(state.feed(kc::KEY_N, 1), Some(HotkeyAction::Toggle));
        // 按住不放不重复触发
        assert_eq!(state.feed(kc::KEY_N, 2), None);
        assert_eq!(state.feed(kc::KEY_N, 1), Some(HotkeyAction::Toggle));
        state.feed(kc::KEY_N, 0);
        state.feed(kc::KEY_LEFTALT, 0);
        assert_eq!(state.feed(kc::KEY_N, 1), None); // 缺 ctrl
    }

    #[test]
    fn hotkey_rejects_bad_specs() {
        assert!(Hotkey::parse("ctrl+").is_err());
        assert!(Hotkey::parse("ctrl+alt").is_err());
        assert!(Hotkey::parse("n+m").is_err());
        let (state, notes) = HotkeyState::new(&[HotkeySpec {
            gesture: "bogus+x".to_string(),
            action: "toggle".to_string(),
        }]);
        assert!(state.is_empty());
        assert_eq!(notes.len(), 1);
    }
}
