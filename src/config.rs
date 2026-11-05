//! 配置：默认值即官方 AlgoParameters 原值，改动即偏离官方。

use crate::keycodes as kc;
use crate::layout::KeyMode;
use serde::de::{self, Deserializer, Visitor};
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

// --------------------------------------------------------------------------- //
// 官方常量（AsusNUMPADDriver V17.0.0.27 / AsusNUMPADFilter.inf AlgoParameters）
// --------------------------------------------------------------------------- //

/// 官方注释 "1 frame 8ms" → 125 Hz
pub const FRAME_MS: u32 = 8;
pub const FRAMES_PER_SEC: u32 = 1000 / FRAME_MS;

/// NUMPAD_00 ContSendKey_Trig_FC = 25 (200 ms)
pub const NUM_KEY_CONT_TRIGGER_FRAMES: u32 = 25;
/// NUMPAD_01 NumLock_Trigger_FC = 40 (320 ms, 通用)
pub const NUMLOCK_TRIGGER_FRAMES: u32 = 40;
/// NUMPAD_GH3XLNDZ_01 NumLock_Trigger_FC = 62 (496 ms, 本机)
pub const NUMLOCK_TRIGGER_FRAMES_GH3XLNDZ: u32 = 62;
/// NUMPAD_GH2XLNDZ_01 同族取值
pub const NUMLOCK_TRIGGER_FRAMES_GH2XLNDZ: u32 = 62;
/// NUMPAD_06 NumAdv_Trigger_FC = 40 (320 ms)
pub const NUMADV_TRIGGER_FRAMES: u32 = 40;
/// NUMPAD_02 Max_Numpad_Work_Time_T = 120000 ms
pub const MAX_WORK_TIME_MS: u32 = 120_000;
/// NUMPAD_03 iUserEnableNumpadAlgo = 1
pub const ENABLE_NUMPAD_ALGO: u8 = 1;
/// NUMPAD_04 iNumpad_FeatureSwitch = 255
pub const NUMPAD_FEATURE_SWITCH: u8 = 0xFF;
/// NUMPAD_07 EnAppGesture_Trig_UM = 7000 µm
pub const APP_GESTURE_TRIG_UM: f64 = 7000.0;
/// NUMPAD_08 iNumLockMaxFailDeltaDist_Ratio = 3
pub const NUMLOCK_MAX_FAIL_DELTA_RATIO: f64 = 3.0;
/// NUMPAD_09 iNumLockMaxVaildDist = 2741 µm
pub const NUMLOCK_MAX_VALID_DIST_UM: f64 = 2741.0;
/// NUMPAD_10 iNumLockMaxFailNum = 3
pub const NUMLOCK_MAX_FAIL_NUM: u32 = 3;

/// GESTURE_00 dwSupport_MixedMode = 1
pub const GESTURE_SUPPORT_MIXED: u8 = 1;
/// GESTURE_01 Gesture_TouchSendKey_Trig_FC = 10 (80 ms)
pub const GESTURE_TOUCH_SENDKEY_FRAMES: u32 = 10;
/// GESTURE_02 Gesture_DelayTouchSendKey_Ratio = 3
pub const GESTURE_DELAY_TOUCH_SENDKEY_RATIO: u32 = 3;
/// GESTURE_03/04 Min_Fir/Sec_Det_TAP_Interval_T = 350 ms
pub const TAP_INTERVAL_MIN_MS: f64 = 350.0;
/// GESTURE_05 Gesture_Trig_UM = 4838 µm
pub const GESTURE_TRIG_UM: f64 = 4838.0;
/// GESTURE_06 iTAPPING_THRESHOLD_MM = 10 mm
pub const TAP_THRESHOLD_MM: f64 = 10.0;
/// GESTURE_07 iGestureEdgeTempDataLen = 30 帧
pub const GESTURE_EDGE_TEMP_FRAMES: u32 = 30;
/// GESTURE_08 iscroll_finger_dist_MM = 40 mm
pub const SCROLL_FINGER_DIST_MM: f64 = 40.0;
/// GESTURE_09 iMixedMode_FeatureSwitch = 255
pub const MIXED_MODE_FEATURE_SWITCH: u8 = 0xFF;

/// LED_00 iLEDLevel = 1（无亮度档位）
pub const LED_LEVEL: u8 = 1;
/// LED_03 SupportLED = 1
pub const SUPPORT_LED: u8 = 1;
/// APSettingParameters COMMONFEATURE_00
pub const CALCULATOR_FEATURE: u8 = 1;
/// FORCE_07 Normal = 1 / FORCE_08 Particular（小键盘态）= 2
pub const FORCE_LEVEL_NORMAL: u8 = 1;
pub const FORCE_LEVEL_NUMPAD: u8 = 2;

/// 点灯命令取值（官方仅两态，另加一个解锁值）
///
/// - `0x01` / `0x00`：亮 / 灭，hidraw FEATURE 0x0d 与 i2c 两条通道通用
/// - `0x60`：i2c 直写 EC 的解锁前置字，hidraw FEATURE 通道**不需要**；
///   亮度档 `0x41..=0x48` 对应 1..8 档
pub const VAL_UNLOCK: u8 = 0x60;
pub const VAL_ON: u8 = 0x01;
pub const VAL_OFF: u8 = 0x00;
/// 亮度档起点：`0x41` 为 1 档，`0x48` 为 8 档
pub const VAL_BRIGHTNESS_BASE: u8 = 0x40;
pub const VAL_BRIGHTNESS_MAX: u8 = 8;

/// 手动调光的默认档数（`asus-numpad bright 1|2|3|4`，另有 0 = 熄灭）。
///
/// 四档在「熄灭 + 硬件全档」这条轴上**线性等分**（`0x00` 与 `0x41..=0x48`，共 9 点），
/// 首档就是熄灭、末档是最亮：默认 8 档表 → `0x00 / 0x43 / 0x45 / 0x48`。
///
/// 这只是默认值：`manual_brightness_levels` 给了自定义表时，档数以该表长度为准，
/// 亮度直接取表内写死的值（见 [`Config::manual_steps`]）。
pub const MANUAL_BRIGHTNESS_STEPS: u8 = 4;

/// 自定义手动档位表的长度上限（受硬件 0x41..=0x48 八个物理档限制）。
pub const MANUAL_BRIGHTNESS_MAX_STEPS: usize = 8;

/// 自定义档位是否合法：`0`（熄灭）或硬件亮度档 `0x41..=0x48`。
///
/// 允许 0 是为了让「从 0 到最大线性四档」能写成 `[0, 0x43, 0x46, 0x48]`，
/// 与 `bright 0` 的熄灭语义一致；其余值一律视为手写配置错误。
pub fn manual_level_allowed(v: u8) -> bool {
    v == VAL_OFF
        || (VAL_BRIGHTNESS_BASE + 1..=VAL_BRIGHTNESS_BASE + VAL_BRIGHTNESS_MAX).contains(&v)
}

pub const PROC_INPUT_DEVICES: &str = "/proc/bus/input/devices";
/// 配置文件正式路径（与 rpm 包内安装路径一致）。
pub const CONFIG_PATH: &str = "/etc/asus-numpad/asus-numpad.json";
/// 历史版本的配置路径（都平铺在 `/etc` 下），按新→旧顺序探测。
///
/// * `/etc/asus-numpad.json` —— 改名后中间版本的路径；
/// * `/etc/rog-numpad.json`   —— 更早 rog-numpad 时代的路径。
///
/// 仅用于一次性迁移与只读兼容，新写入一律走 [`CONFIG_PATH`]。
pub const LEGACY_CONFIG_PATHS: &[&str] = &["/etc/asus-numpad.json", "/etc/rog-numpad.json"];
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// --------------------------------------------------------------------------- //
// 枚举
// --------------------------------------------------------------------------- //

/// 背光与系统 NumLock 的联动方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum NumLockMode {
    /// 只在 NumLock 已开时点灯；关闭时补按一次 NumLock
    #[default]
    Sync,
    /// 无条件切换 NumLock
    Toggle,
    /// 不动 NumLock
    Off,
}

impl NumLockMode {
    pub fn as_str(self) -> &'static str {
        match self {
            NumLockMode::Sync => "sync",
            NumLockMode::Toggle => "toggle",
            NumLockMode::Off => "off",
        }
    }
}

/// 计算器联动模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum CalcMode {
    /// 长按左上图标区（NUMADV_Trigger_FC = 40 帧）
    #[default]
    Long,
    /// 关闭
    Off,
}

/// 混合模式开关：官方 dwSupport_MixedMode = 1 的落地开关。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MixedMode {
    /// 有虚拟触摸板就开启（等同官方默认）
    #[default]
    Auto,
    /// 强制开启
    On,
    /// 纯数字键盘（光标不可用）
    Off,
}

impl MixedMode {
    pub fn is_enabled(self) -> bool {
        !matches!(self, MixedMode::Off)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            MixedMode::Auto => "auto",
            MixedMode::On => "true",
            MixedMode::Off => "false",
        }
    }
}

/// 兼容上游配置写法：`"auto"` / `true` / `false` / `"on"` / `"off"`。
impl<'de> Deserialize<'de> for MixedMode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = MixedMode;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("auto / true / false")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<MixedMode, E> {
                Ok(if v { MixedMode::On } else { MixedMode::Off })
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<MixedMode, E> {
                match v.to_ascii_lowercase().as_str() {
                    "auto" => Ok(MixedMode::Auto),
                    "on" | "true" | "yes" | "1" => Ok(MixedMode::On),
                    "off" | "false" | "no" | "0" => Ok(MixedMode::Off),
                    other => Err(E::custom(crate::i18n::tf(
                        "cfg.err.mixed_mode",
                        &[("v", other)],
                    ))),
                }
            }
        }
        d.deserialize_any(V)
    }
}

/// 键位风格（无效值回落 keypad，与上游一致）。
impl<'de> Deserialize<'de> for KeyMode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(s.parse().unwrap_or(KeyMode::Keypad))
    }
}

// --------------------------------------------------------------------------- //
// 配置结构
// --------------------------------------------------------------------------- //

/// 全部字段均可缺省；缺省即官方值。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    // ---- 官方参数覆盖 ----
    pub frame_ms: u32,
    pub numlock_trigger_frames: u32,
    pub cont_sendkey_trigger_frames: u32,
    pub numadv_trigger_frames: u32,
    pub max_work_time_ms: u32,
    pub app_gesture_trig_um: f64,
    pub numlock_max_valid_dist_um: f64,
    pub numlock_max_fail_num: u32,
    pub gesture_trig_um: f64,
    pub tap_threshold_mm: f64,
    pub scroll_finger_dist_mm: f64,
    // ---- 混合模式手势（NUMPADGESTURE_00..09 官方原值） ----
    /// GESTURE_01 = 10 帧 (80 ms)：手势态触摸后延迟出键，区分「想按键」与「想移动光标」
    pub touch_sendkey_frames: u32,
    /// GESTURE_02 = 3：已在手势中的再次按下，延迟放大为 3 倍（240 ms）
    pub touch_sendkey_ratio: u32,
    /// GESTURE_03/04 = 350 ms：首/次 tap 间隔，用于区分点击与手势
    pub tap_interval_min_ms: f64,
    /// GESTURE_07 = 30 帧：手势边缘临时数据长度
    pub gesture_edge_frames: u32,
    /// 官方未给重复间隔，推断值
    pub repeat_interval_ms: f64,
    // ---- 布局 ----
    /// 官方布局名：`auto` = 按触摸板 HID 硬件 ID 自动识别（推荐）；
    /// 也可写死某套布局（`GH3XLNDZ` / `GH2XLNDZ` / `GH4XLNDZ` 等，见 `asus-numpad layout`）。
    pub layout_name: String,
    /// 允许使用「尚未收录键表」的同族布局出键。
    ///
    /// 默认 `false`：`auto` 识别到非 GH3XLNDZ 布局时只报告不跑，避免键位表不同
    /// 导致按错键；确认可用后再置 `true`（或命令行加 `--allow-unverified`）。
    pub allow_unverified_layout: bool,
    /// 仅支持 cell；icon 子区语义未闭合已移除（读入后强制回落）
    pub numlock_zone: String,
    // ---- 按压力度（FORCE_07 = 1 / FORCE_08 = 2 档位） ----
    /// 官方只给档位号，未在 INF 暴露原始压力值；0 = 不做压力过滤
    pub pressure_numpad_min: i32,
    // ---- 行为 ----
    pub key_mode: KeyMode,
    pub numlock: NumLockMode,
    pub calc_gesture: bool,
    pub calc_gesture_mode: CalcMode,
    pub mixed_mode: MixedMode,
    pub grab: bool,
    // ---- 几何兜底（resolution 缺失时使用） ----
    pub touchpad_width_mm: f64,
    pub touchpad_height_mm: f64,
    // ---- 界面语言 ----
    /// 界面语言：`auto`（跟随系统 locale）/ `zh-CN` / `zh-TW` / `en`。
    /// 优先级：环境变量 `ASUS_NUMPAD_LANG` > 命令行 `--lang` > 本字段 > 系统 locale。
    pub language: String,
    // ---- 背光亮度（跟随键盘背光） ----
    /// 是否让数字区亮度跟随键盘背光（读 asus::kbd_backlight 换算成 0x41..0x48）
    pub follow_keyboard_brightness: bool,
    /// 键盘背光 sysfs 目录，默认 `/sys/class/leds/asus::kbd_backlight`
    pub keyboard_brightness_dir: String,
    /// 数字区亮度档命令字，从暗到亮；留空 = 不跟随（退回 0x01 点亮）。
    ///
    /// 接受三种写法：整数（`65`）、十六进制字符串（`"0x41"`）、十进制字符串（`"65"`）；
    /// 写回时统一成整数数组——早期版本写的是 `0x41` 字符串，读不回来会让整份配置
    /// 悄悄退回默认值（跟随开关、手动档一起丢失），这里必须兼容。
    #[serde(
        default = "default_brightness_levels",
        deserialize_with = "de_brightness_levels"
    )]
    pub brightness_levels: Vec<u8>,
    /// 键盘背光轮询间隔（ms）
    pub brightness_poll_ms: u32,
    /// 手动调光的固定档（0 = 熄灭；1..= [`Config::manual_steps`] 从最暗到最亮）。
    /// 仅在「不跟随键盘背光」时作为常亮档使用；`asus-numpad bright N` 会写入这里。
    pub manual_brightness_step: u8,
    /// 自定义手动档位表（从暗到亮），留空 = 按 `manual_brightness_steps` 在
    /// `brightness_levels` 上线性等分。
    ///
    /// 允许的取值只有硬件的 `0x41..=0x48` 八档，长度 1..=8；档数由长度决定，
    /// 例如 `[65, 68, 72]` → `bright 1|2|3`。这一项让「四档」不必是等分——
    /// 想把 90% 亮度压在最后一档，直接写 `[65, 66, 67, 72]` 即可。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_opt_brightness_levels"
    )]
    pub manual_brightness_levels: Option<Vec<u8>>,
    // ---- 通道 ----
    pub i2c_transfer: String,
    pub i2c_bus: Option<i32>,
    pub i2c_addr: Option<u8>,
    pub hidraw: Option<String>,
    pub poll_interval_ms: u32,
    // ---- 运行时（一般由命令行 / systemd 单元给出，不在默认配置文件里） ----
    /// 触摸板事件设备覆盖，如 /dev/input/event7
    pub device_override: Option<String>,
    /// 键盘事件设备覆盖
    pub keyboard_override: Option<String>,
    /// 背光通道：hidraw / i2c / none
    pub led_kind: String,
    /// 历史字段：hidraw 曾用 `write()` 自发拼报告前缀，现已改为
    /// FEATURE 报告 `0x0d` 定长报文，本字段不再生效，仅为兼容旧配置文件保留。
    pub hidraw_prefix: Vec<u8>,
    /// i2c 载荷首字节（与 LED 命令字构成 w2 写入）
    pub i2c_payload: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            frame_ms: FRAME_MS,
            numlock_trigger_frames: NUMLOCK_TRIGGER_FRAMES,
            cont_sendkey_trigger_frames: NUM_KEY_CONT_TRIGGER_FRAMES,
            numadv_trigger_frames: NUMADV_TRIGGER_FRAMES,
            max_work_time_ms: MAX_WORK_TIME_MS,
            app_gesture_trig_um: APP_GESTURE_TRIG_UM,
            numlock_max_valid_dist_um: NUMLOCK_MAX_VALID_DIST_UM,
            numlock_max_fail_num: NUMLOCK_MAX_FAIL_NUM,
            gesture_trig_um: GESTURE_TRIG_UM,
            tap_threshold_mm: TAP_THRESHOLD_MM,
            scroll_finger_dist_mm: SCROLL_FINGER_DIST_MM,
            touch_sendkey_frames: GESTURE_TOUCH_SENDKEY_FRAMES,
            touch_sendkey_ratio: GESTURE_DELAY_TOUCH_SENDKEY_RATIO,
            tap_interval_min_ms: TAP_INTERVAL_MIN_MS,
            gesture_edge_frames: GESTURE_EDGE_TEMP_FRAMES,
            repeat_interval_ms: 40.0,
            layout_name: "auto".to_string(),
            allow_unverified_layout: false,
            numlock_zone: "cell".to_string(),
            pressure_numpad_min: 0,
            key_mode: KeyMode::Keypad,
            numlock: NumLockMode::Sync,
            calc_gesture: true,
            calc_gesture_mode: CalcMode::Long,
            mixed_mode: MixedMode::Auto,
            grab: true,
            touchpad_width_mm: 130.0,
            touchpad_height_mm: 80.0,
            language: "auto".to_string(),
            follow_keyboard_brightness: true,
            keyboard_brightness_dir: crate::brightness::DEFAULT_DIR.to_string(),
            brightness_levels: default_brightness_levels(),
            brightness_poll_ms: 500,
            manual_brightness_step: MANUAL_BRIGHTNESS_STEPS,
            manual_brightness_levels: None,
            i2c_transfer: "i2ctransfer".to_string(),
            i2c_bus: None,
            i2c_addr: None,
            hidraw: None,
            poll_interval_ms: FRAME_MS,
            device_override: None,
            keyboard_override: None,
            led_kind: "auto".to_string(),
            hidraw_prefix: vec![0x00, 0x03, 0x01],
            i2c_payload: "0x00".to_string(),
        }
    }
}

/// 默认档位表：0x41..=0x48（8 档）
fn default_brightness_levels() -> Vec<u8> {
    (1..=VAL_BRIGHTNESS_MAX)
        .map(|i| VAL_BRIGHTNESS_BASE + i)
        .collect()
}

/// 宽松解析亮度档位表：兼容整数与 `"0x41"` / `"65"` 字符串。
/// 任何一项解析不了都直接报错（不要静默丢档位），错误信息带下标，方便定位手改错的配置。
fn de_brightness_levels<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<serde_json::Value>::deserialize(deserializer)?;
    parse_level_list::<D::Error>(&raw)
}

/// 同上，但允许整项缺失 / 显式 `null`（`manual_brightness_levels` 用）。
fn de_opt_brightness_levels<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<Vec<serde_json::Value>>::deserialize(deserializer)?;
    match raw {
        None => Ok(None),
        Some(list) => parse_level_list::<D::Error>(&list).map(Some),
    }
}

/// 档位数组解析的共用实现（与 serde 解耦，方便两处复用）。
fn parse_level_list<E: de::Error>(raw: &[serde_json::Value]) -> Result<Vec<u8>, E> {
    let mut out = Vec::with_capacity(raw.len());
    for (idx, item) in raw.iter().enumerate() {
        let parsed = match item {
            serde_json::Value::Number(n) => {
                n.as_u64().filter(|v| *v <= u8::MAX as u64).map(|v| v as u8)
            }
            serde_json::Value::String(text) => {
                let t = text.trim();
                match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    Some(hex) => u8::from_str_radix(hex, 16).ok(),
                    None => t.parse::<u8>().ok(),
                }
            }
            _ => None,
        };
        match parsed {
            Some(v) => out.push(v),
            None => {
                return Err(de::Error::custom(crate::i18n::tf(
                    "cfg.err.brightness_levels",
                    &[("idx", &idx.to_string()), ("item", &item.to_string())],
                )))
            }
        }
    }
    Ok(out)
}

impl Config {
    /// 实际生效的数字区亮度档；返回空 = 不跟随键盘背光、退回 0x01 点亮。
    pub fn effective_levels(&self) -> Vec<u8> {
        if self.follow_keyboard_brightness {
            self.brightness_levels.clone()
        } else {
            Vec::new()
        }
    }

    /// 手动调光档数：有自定义表时以表长为准，否则用默认 [`MANUAL_BRIGHTNESS_STEPS`]。
    pub fn manual_steps(&self) -> u8 {
        match &self.manual_brightness_levels {
            Some(v) if !v.is_empty() => v.len().min(MANUAL_BRIGHTNESS_MAX_STEPS) as u8,
            _ => MANUAL_BRIGHTNESS_STEPS,
        }
    }

    /// 手动调光用的档位表（从暗到亮）。
    ///
    /// * 配了 `manual_brightness_levels` → 原样使用（就是用户写的「自定义」）；
    /// * 没配 → 在「熄灭 + 硬件全档」这条轴（`0x00` 与 `0x41..=0x48`，共 9 点）上按
    ///   `manual_steps()` **线性等分**，首档落 `0x00`（灭）、末档落最亮。
    ///   默认 8 档 + 4 档 → `0x00 / 0x43 / 0x45 / 0x48`。
    ///
    /// 与「是否跟随键盘背光」无关：关掉跟随之后 `bright N` 依然可用。
    pub fn manual_levels(&self) -> Vec<u8> {
        if let Some(v) = &self.manual_brightness_levels {
            if !v.is_empty() {
                return v
                    .iter()
                    .take(MANUAL_BRIGHTNESS_MAX_STEPS)
                    .copied()
                    .collect();
            }
        }
        crate::brightness::linear_levels_from_off(
            &self.brightness_levels,
            MANUAL_BRIGHTNESS_STEPS as usize,
        )
    }

    /// 手动档位 → 报文亮度字节。
    ///
    /// - `step = 0` → `0x00`（熄灭）
    /// - `step = 1..=manual_steps()` → 自定义 / 线性表里对应那一档（1 = 灭到最暗端，N = 最亮）
    /// - 越界返回 `None`；档位表为空时退化为 `VAL_ON`
    ///
    /// 默认档表首档就是 `0x00`（用户要求「0 到最大线性四档」），所以 `bright 1` 与
    /// `bright 0` 效果相同——这是硬件只有 8 级造成的，不是 bug。
    /// 想让首档变成「最暗但仍有光」，写 `manual_brightness_levels: [0x41, 0x43, 0x46, 0x48]`。
    pub fn level_for_step(&self, step: u8) -> Option<u8> {
        if step == 0 {
            return Some(VAL_OFF);
        }
        let levels = self.manual_levels();
        if levels.is_empty() {
            return Some(VAL_ON);
        }
        if step as usize > levels.len() {
            return None;
        }
        Some(levels[step as usize - 1])
    }

    /// 不跟随键盘背光时点灯用的常亮值：取手动档，命中 `0x00` 档时退回 `0x01` 点亮。
    ///
    /// 「默认四档首档就是熄灭」之后，若这里原样返回 `0x00`，关掉跟随就会让数字区一直黑着，
    /// 与「常亮」的语义相悖；真正的熄灭仍由 `bright 0` / `off` 负责。
    pub fn static_on_value(&self) -> u8 {
        // 档位 0 是用户显式要求「不点灯」，与「四档里的第 1 档恰好是熄灭」要分开处理
        if self.manual_brightness_step == 0 {
            return VAL_OFF;
        }
        match self.level_for_step(self.manual_brightness_step) {
            Some(VAL_OFF) | None => VAL_ON,
            Some(v) => v,
        }
    }

    /// 本机布局对应的 NumLock 长按帧数（NUMPAD_GH3XLNDZ_01 = 62）。
    pub fn numlock_frames(&self) -> u32 {
        // 按「实际生效的布局」取长按帧数：auto 会解析成具体布局名，
        // 解析不到时回落 GH3XLNDZ 的 62 帧（≈ 496 ms）。
        match self.layout_entry().name {
            "GH3XLNDZ" => NUMLOCK_TRIGGER_FRAMES_GH3XLNDZ,
            "GH2XLNDZ" | "GH4XLNDZ" => NUMLOCK_TRIGGER_FRAMES_GH2XLNDZ,
            _ => self.numlock_trigger_frames,
        }
    }

    /// 与上游 `load_config` 相同的收敛逻辑：非法值回落官方默认，并说明原因。
    pub fn sanitize(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        // 界面语言只接受 auto / zh-CN / zh-TW / en（大小写不敏感），其余回落 auto
        let lang_raw = self.language.trim().to_string();
        if lang_raw.is_empty() {
            self.language = "auto".to_string();
        } else if !lang_raw.eq_ignore_ascii_case("auto") {
            match crate::i18n::Lang::parse(&lang_raw) {
                Some(l) => self.language = l.code().to_string(),
                None => {
                    notes.push(crate::i18n::tf(
                        "note.invalid_language",
                        &[("v", &lang_raw)],
                    ));
                    self.language = "auto".to_string();
                }
            }
        } else {
            self.language = "auto".to_string();
        }
        self.frame_ms = self.frame_ms.max(1);
        self.numlock_trigger_frames = self.numlock_trigger_frames.max(1);
        self.poll_interval_ms = self.poll_interval_ms.max(2);
        self.brightness_poll_ms = self.brightness_poll_ms.max(50);
        if self.keyboard_brightness_dir.is_empty() {
            self.keyboard_brightness_dir = crate::brightness::DEFAULT_DIR.to_string();
        }
        // 亮度档只允许落在 0x41..=0x48，越界值直接剔除（防手写配置把灯发到哨位上）
        let lo = VAL_BRIGHTNESS_BASE + 1;
        let hi = VAL_BRIGHTNESS_BASE + VAL_BRIGHTNESS_MAX;
        let before = self.brightness_levels.len();
        self.brightness_levels.retain(|v| (lo..=hi).contains(v));
        if self.brightness_levels.len() != before {
            let dropped = (before - self.brightness_levels.len()).to_string();
            notes.push(crate::i18n::tf(
                "note.levels_dropped",
                &[
                    ("n", &dropped),
                    ("lo", &format!("{lo:02x}")),
                    ("hi", &format!("{hi:02x}")),
                ],
            ));
        }
        // 自定义手动档表：非法值剔除、非递增表/空表/超长表按规则收敛，
        // 免得手改配置写成 `[0, 255]` 把灯发到哨位上，或写 20 档却没那么多物理档。
        //
        // 合法取值是 `0`（熄灭）或 `0x41..=0x48`：允许 0 是为了支持「从 0 到最大
        // 线性自定义四档」这种写法（如 `[0, 0x43, 0x46, 0x48]`），硬件侧 0 就是
        // 熄灭，与 `bright 0` 同义。
        if let Some(custom) = self.manual_brightness_levels.take() {
            let mut kept: Vec<u8> = custom
                .iter()
                .copied()
                .filter(|v| manual_level_allowed(*v))
                .collect();
            if kept.len() != custom.len() {
                let dropped = (custom.len() - kept.len()).to_string();
                notes.push(crate::i18n::tf(
                    "note.manual_levels_dropped",
                    &[
                        ("n", &dropped),
                        ("lo", &format!("{lo:02x}")),
                        ("hi", &format!("{hi:02x}")),
                    ],
                ));
            }
            if kept.len() > MANUAL_BRIGHTNESS_MAX_STEPS {
                let n = kept.len().to_string();
                let max = MANUAL_BRIGHTNESS_MAX_STEPS.to_string();
                notes.push(crate::i18n::tf(
                    "note.manual_levels_truncated",
                    &[("n", &n), ("max", &max)],
                ));
                kept.truncate(MANUAL_BRIGHTNESS_MAX_STEPS);
            }
            // 档位必须从暗到亮严格递增：否则 `bright 2` 可能比 `bright 1` 更暗，
            // 与「档位越大越亮」的直觉相反。写错就整表丢弃，回落线性等分。
            if kept.is_empty() {
                notes.push(crate::i18n::t("note.manual_levels_reset"));
            } else if !kept.windows(2).all(|w| w[0] < w[1]) {
                notes.push(crate::i18n::t("note.manual_levels_not_increasing"));
            } else {
                self.manual_brightness_levels = Some(kept);
            }
        }
        // 手动档上限取决于实际档数（自定义表长度可能不是 4）
        let steps = self.manual_steps();
        if self.manual_brightness_step > steps {
            let step = self.manual_brightness_step.to_string();
            let max = steps.to_string();
            notes.push(crate::i18n::tf(
                "note.manual_step_clamped",
                &[("step", &step), ("max", &max)],
            ));
            self.manual_brightness_step = steps;
        }
        if self.numlock_zone != "cell" {
            let value = self.numlock_zone.clone();
            notes.push(crate::i18n::tf("note.numlock_zone", &[("v", &value)]));
            self.numlock_zone = "cell".to_string();
        }
        if self.pressure_numpad_min < 0 {
            self.pressure_numpad_min = 0;
        }
        let layout_raw = self.layout_name.trim().to_string();
        if layout_raw.is_empty() {
            // 空值按 auto 处理：跨机型通用靠的是按 HID 自动识别，而不是写死默认布局
            self.layout_name = "auto".to_string();
        } else if !layout_raw.eq_ignore_ascii_case("auto") {
            match crate::layout::by_name(&layout_raw) {
                Some(e) => self.layout_name = e.name.to_string(),
                None => {
                    notes.push(crate::i18n::tf(
                        "note.layout_unknown",
                        &[("v", &layout_raw)],
                    ));
                    self.layout_name = "auto".to_string();
                }
            }
        } else {
            self.layout_name = "auto".to_string();
        }
        notes
    }

    /// 读取配置；文件不存在或解析失败时用官方默认值（与上游行为一致）。
    pub fn load(path: &Path) -> (Self, Vec<String>) {
        let mut notes = Vec::new();
        let mut cfg = match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Config>(&text) {
                Ok(c) => c,
                Err(err) => {
                    notes.push(crate::i18n::tf(
                        "note.read_failed",
                        &[
                            ("path", &path.display().to_string()),
                            ("err", &err.to_string()),
                        ],
                    ));
                    Config::default()
                }
            },
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    notes.push(crate::i18n::tf(
                        "note.read_failed",
                        &[
                            ("path", &path.display().to_string()),
                            ("err", &err.to_string()),
                        ],
                    ));
                }
                Config::default()
            }
        };
        notes.extend(cfg.sanitize());
        (cfg, notes)
    }

    /// 用户级配置路径（免 root 可写）：`$XDG_CONFIG_HOME/asus-numpad/asus-numpad.json`，
    /// 未设 `XDG_CONFIG_HOME` 时回落 `~/.config/asus-numpad/asus-numpad.json`。
    ///
    /// 服务与 CLI 都以同一个登录用户身份运行，这一份就是唯一事实来源；
    /// 只有拿不到 `HOME` 的异常环境（chroot / initramfs）才退回系统路径。
    pub fn user_config_path() -> PathBuf {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            let dir = PathBuf::from(xdg);
            if !dir.as_os_str().is_empty() {
                return dir.join("asus-numpad").join("asus-numpad.json");
            }
        }
        match std::env::var_os("HOME") {
            Some(home) if !home.as_os_str().is_empty() => PathBuf::from(home)
                .join(".config")
                .join("asus-numpad")
                .join("asus-numpad.json"),
            _ => PathBuf::from(CONFIG_PATH),
        }
    }

    /// 当前进程是否持有写 `/etc` 的特权（root）。
    ///
    /// 这是「不用 sudo」的分支点：普通用户所有写入都落到 [`user_config_path`]，
    /// root（含有人手工 `sudo asus-numpad bright 2`）仍写系统路径，保持管理员习惯。
    pub fn is_privileged() -> bool {
        #[cfg(target_os = "linux")]
        {
            // SAFETY: geteuid 是纯查询，无副作用
            unsafe { libc::geteuid() == 0 }
        }
        #[cfg(not(target_os = "linux"))]
        {
            // 非 Linux 只做交叉编译与单测，保持历史行为（写 /etc）
            true
        }
    }

    /// 落盘目标：普通用户 → 用户级路径；特权进程 → 传入路径（通常是 `/etc`）。
    pub fn writable_path(read_path: &Path) -> PathBuf {
        if Self::is_privileged() {
            read_path.to_path_buf()
        } else {
            Self::user_config_path()
        }
    }

    /// 默认配置路径（读取用）。
    ///
    /// 顺序：用户级 → `/etc` 正式路径 → 历史平铺路径；都不存在时返回用户级路径，
    /// 也就是「首次写入默认落在自己的家目录」，不再因为写不了 `/etc` 而失败。
    pub fn default_path() -> PathBuf {
        let user = Self::user_config_path();
        if user.exists() {
            return user;
        }
        let current = PathBuf::from(CONFIG_PATH);
        if current.exists() {
            return current;
        }
        for legacy in LEGACY_CONFIG_PATHS {
            let path = PathBuf::from(legacy);
            if path.exists() {
                return path;
            }
        }
        user
    }

    /// 迁移源：`/etc` 正式路径或历史路径中第一个存在的。
    fn legacy_config_path() -> Option<&'static Path> {
        std::iter::once(CONFIG_PATH)
            .chain(LEGACY_CONFIG_PATHS.iter().copied())
            .map(Path::new)
            .find(|p| p.exists())
    }

    /// 把 `/etc` 下的既有配置搬到用户级路径（源文件保留不动，避免误删系统默认）。
    ///
    /// 从 1.0.0-15 起配置归用户所有：普通用户改档位 / 语言不再需要 sudo。
    /// 只在「用户级还不存在」时迁移；已存在的用户级配置视为最新意图，不覆盖。
    /// 迁移失败（例如无 `HOME`）静默跳过，读取侧仍有 [`default_path`] 的回落。
    pub fn migrate_legacy_config() -> Option<String> {
        let target = Self::user_config_path();
        if target.exists() {
            return None;
        }
        let source = Self::legacy_config_path()?;
        if source == target.as_path() {
            return None;
        }
        let text = std::fs::read_to_string(source).ok()?;
        // 只搬合法配置，避免把半截文件带进用户目录
        serde_json::from_str::<Config>(&text).ok()?;
        std::fs::create_dir_all(target.parent()?).ok()?;
        std::fs::write(&target, text.as_bytes()).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644));
        }
        let to = target.display().to_string();
        Some(crate::i18n::tf(
            "note.config_migrated",
            &[("from", &source.display().to_string()), ("to", &to)],
        ))
    }

    /// 生成示例配置（安装脚本与 `--dump-config` 使用）。
    pub fn to_json(&self) -> String {
        let value = serde_json::json!({
            "frame_ms": self.frame_ms,
            "numlock_trigger_frames": self.numlock_trigger_frames,
            "cont_sendkey_trigger_frames": self.cont_sendkey_trigger_frames,
            "numadv_trigger_frames": self.numadv_trigger_frames,
            "max_work_time_ms": self.max_work_time_ms,
            "app_gesture_trig_um": self.app_gesture_trig_um,
            "numlock_max_valid_dist_um": self.numlock_max_valid_dist_um,
            "numlock_max_fail_num": self.numlock_max_fail_num,
            "gesture_trig_um": self.gesture_trig_um,
            "tap_threshold_mm": self.tap_threshold_mm,
            "scroll_finger_dist_mm": self.scroll_finger_dist_mm,
            "touch_sendkey_frames": self.touch_sendkey_frames,
            "touch_sendkey_ratio": self.touch_sendkey_ratio,
            "tap_interval_min_ms": self.tap_interval_min_ms,
            "gesture_edge_frames": self.gesture_edge_frames,
            "repeat_interval_ms": self.repeat_interval_ms,
            "layout_name": self.layout_name,
            "allow_unverified_layout": self.allow_unverified_layout,
            "numlock_zone": self.numlock_zone,
            "pressure_numpad_min": self.pressure_numpad_min,
            "key_mode": self.key_mode.as_str(),
            "numlock": self.numlock.as_str(),
            "calc_gesture": self.calc_gesture,
            "calc_gesture_mode": match self.calc_gesture_mode {
                CalcMode::Long => "long",
                CalcMode::Off => "off",
            },
            "mixed_mode": self.mixed_mode.as_str(),
            "grab": self.grab,
            "touchpad_width_mm": self.touchpad_width_mm,
            "touchpad_height_mm": self.touchpad_height_mm,
            "i2c_transfer": self.i2c_transfer,
            "i2c_bus": self.i2c_bus,
            "i2c_addr": self.i2c_addr,
            "hidraw": self.hidraw,
            "poll_interval_ms": self.poll_interval_ms,
            "language": self.language,
            "follow_keyboard_brightness": self.follow_keyboard_brightness,
            "keyboard_brightness_dir": self.keyboard_brightness_dir,
            // 整数数组：既保证自己读得回来，也不给手改配置添麻烦
            "brightness_levels": self.brightness_levels,
            "brightness_poll_ms": self.brightness_poll_ms,
            "manual_brightness_step": self.manual_brightness_step,
            "manual_brightness_levels": self.manual_brightness_levels,
            "led_kind": self.led_kind,
        });

        let mut text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
        text.push('\n');
        text
    }

    /// 供日志使用的一行摘要。
    pub fn summary(&self) -> String {
        crate::i18n::tf(
            "cfg.summary",
            &[
                ("layout", &self.layout_name),
                ("mode", self.key_mode.as_str()),
                ("frames", &self.numlock_frames().to_string()),
                ("ms", &(self.numlock_frames() * self.frame_ms).to_string()),
                ("idle", &(self.max_work_time_ms / 1000).to_string()),
                ("mixed", self.mixed_mode.as_str()),
                ("numlock", self.numlock.as_str()),
            ],
        )
    }

    /// 布局记录：`auto` 或未知名字都回落到 GH3XLNDZ（键表基准）。
    ///
    /// 实际识别发生在运行时：`linux` 模块会先用触摸板设备名里的 HID 硬件 ID
    /// 查官方映射表，命中后把 `layout_name` 定成具体布局名再调用本函数。
    pub fn layout_entry(&self) -> &'static crate::layout::LayoutEntry {
        crate::layout::by_name(&self.layout_name)
            .or_else(|| crate::layout::by_name(crate::layout::DEFAULT_LAYOUT_NAME))
            .expect("内置布局表必须包含默认布局")
    }

    /// 官方布局参数。
    pub fn layout(&self) -> crate::layout::Layout {
        self.layout_entry().layout
    }
}

/// 官方参数一览（`--check` 输出用）；键名与说明随界面语言变化。
pub fn official_parameters() -> Vec<(String, String)> {
    let frames = |n: u32, ms_key: &str| {
        crate::i18n::tf(
            ms_key,
            &[
                ("frames", &n.to_string()),
                ("ms", &(n * FRAME_MS).to_string()),
            ],
        )
    };
    vec![
        (
            crate::i18n::t("param.frame_model"),
            format!("1 frame {FRAME_MS}ms = {FRAMES_PER_SEC} Hz"),
        ),
        (
            "NUMPAD_00 ContSendKey_Trig_FC".to_string(),
            frames(NUM_KEY_CONT_TRIGGER_FRAMES, "param.cont_sendkey"),
        ),
        (
            "NUMPAD_01 NumLock_Trigger_FC".to_string(),
            frames(NUMLOCK_TRIGGER_FRAMES, "param.numlock"),
        ),
        (
            "NUMPAD_GH3XLNDZ_01".to_string(),
            frames(NUMLOCK_TRIGGER_FRAMES_GH3XLNDZ, "param.numlock_gh3"),
        ),
        (
            "NUMPAD_02 Max_Numpad_Work_Time_T".to_string(),
            format!("{MAX_WORK_TIME_MS} ms"),
        ),
        (
            "NUMPAD_06 NumAdv_Trigger_FC".to_string(),
            frames(NUMADV_TRIGGER_FRAMES, "param.cont_sendkey"),
        ),
        (
            "NUMPAD_07 EnAppGesture_Trig_UM".to_string(),
            format!("{APP_GESTURE_TRIG_UM:.0} µm"),
        ),
        (
            "NUMPAD_09 NumLockMaxVaildDist".to_string(),
            format!("{NUMLOCK_MAX_VALID_DIST_UM:.0} µm"),
        ),
        (
            "GESTURE_01 TouchSendKey_Trig_FC".to_string(),
            frames(GESTURE_TOUCH_SENDKEY_FRAMES, "param.cont_sendkey"),
        ),
        (
            "GESTURE_05 Gesture_Trig_UM".to_string(),
            format!("{GESTURE_TRIG_UM:.0} µm"),
        ),
        (
            "GESTURE_06 iTAPPING_THRESHOLD_MM".to_string(),
            format!("{TAP_THRESHOLD_MM:.0} mm"),
        ),
        (
            "GESTURE_08 iscroll_finger_dist_MM".to_string(),
            format!("{SCROLL_FINGER_DIST_MM:.0} mm"),
        ),
        (
            crate::i18n::t("param.led_values_name"),
            crate::i18n::tf(
                "param.led_values",
                &[
                    ("off", &format!("{VAL_OFF:02x}")),
                    ("on", &format!("{VAL_ON:02x}")),
                ],
            ),
        ),
        (
            crate::i18n::t("param.i2c_unlock_name"),
            crate::i18n::tf(
                "param.i2c_unlock",
                &[("unlock", &format!("{VAL_UNLOCK:02x}"))],
            ),
        ),
        (
            crate::i18n::t("param.key_count"),
            format!("{}", kc::ALL_KEYS.len()),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_config_path_uses_xdg_layout() {
        // 不论 HOME/XDG 是否设置，末尾必须是 <dir>/asus-numpad/asus-numpad.json
        let p = Config::user_config_path();
        assert!(
            p.ends_with("asus-numpad/asus-numpad.json"),
            "got {}",
            p.display()
        );
    }

    #[test]
    fn writable_path_follows_privilege() {
        // 契约：root 写传入路径（/etc），普通用户写自己的家目录——这是「免 sudo」的根本
        let sys = Path::new(CONFIG_PATH);
        let got = Config::writable_path(sys);
        if Config::is_privileged() {
            assert_eq!(got, sys);
        } else {
            assert_eq!(got, Config::user_config_path());
            assert!(!got.starts_with("/etc"));
        }
    }

    #[test]
    fn defaults_match_official_inf() {
        let c = Config::default();
        assert_eq!(c.frame_ms, 8);
        assert_eq!(c.tap_threshold_mm, 10.0);
        assert_eq!(c.gesture_trig_um, 4838.0);
        assert_eq!(c.numlock_max_valid_dist_um, 2741.0);
        assert_eq!(c.touch_sendkey_frames, 10);
        assert_eq!(c.touch_sendkey_ratio, 3);
        assert_eq!(c.tap_interval_min_ms, 350.0);
        assert_eq!(c.scroll_finger_dist_mm, 40.0);
        assert_eq!(c.app_gesture_trig_um, 7000.0);
        assert_eq!(c.max_work_time_ms, 120_000);
        assert_eq!(c.cont_sendkey_trigger_frames, 25);
        assert_eq!(c.numadv_trigger_frames, 40);
        assert_eq!(c.numlock_zone, "cell");
        assert_eq!(c.pressure_numpad_min, 0);
        assert!(c.calc_gesture);
        assert!(c.grab);
        assert_eq!(c.key_mode, KeyMode::Keypad);
        assert_eq!(c.numlock, NumLockMode::Sync);
        assert_eq!(c.mixed_mode, MixedMode::Auto);
    }

    #[test]
    fn manual_four_steps_span_off_to_brightest() {
        let c = Config::default();
        // 默认：在「熄灭 + 8 档」共 9 点上线性四等分 → 0 / 0x43 / 0x45 / 0x48
        assert_eq!(c.manual_levels(), vec![VAL_OFF, 0x43, 0x45, 0x48]);
        assert_eq!(c.level_for_step(0), Some(VAL_OFF));
        assert_eq!(c.level_for_step(1), Some(VAL_OFF));
        assert_eq!(c.level_for_step(2), Some(0x43));
        assert_eq!(c.level_for_step(3), Some(0x45));
        assert_eq!(c.level_for_step(4), Some(0x48));
        // 越界档位不接受（由 CLI 拦成用法提示）
        assert_eq!(c.level_for_step(5), None);
    }

    #[test]
    fn linear_steps_are_monotonic_and_cover_ends() {
        let c = Config::default();
        let levels = c.manual_levels();
        assert_eq!(levels.len(), MANUAL_BRIGHTNESS_STEPS as usize);
        // 首档 = 熄灭，末档 = 最亮档（「从 0 到最大」的直觉）
        assert_eq!(levels.first().copied(), Some(VAL_OFF));
        assert_eq!(levels.last().copied(), c.brightness_levels.last().copied());
        // 单调不减，中间档必须严格递增
        for w in levels.windows(2) {
            assert!(w[0] <= w[1], "档位必须从暗到亮: {levels:?}");
        }
        assert!(
            levels.windows(2).skip(1).all(|w| w[0] < w[1]),
            "中间档需严格递增: {levels:?}"
        );
    }

    #[test]
    fn linear_from_off_is_stable_across_step_counts() {
        use crate::brightness::linear_levels_from_off;
        let eight: Vec<u8> = (1..=8).map(|i| 0x40 + i).collect();
        // 两端固定：灭 → 最亮
        for n in 2..=8usize {
            let got = linear_levels_from_off(&eight, n);
            assert_eq!(got.len(), n);
            assert_eq!(got[0], VAL_OFF);
            assert_eq!(*got.last().unwrap(), 0x48);
        }
        // 只有一档时没有两端之分，取最亮
        assert_eq!(linear_levels_from_off(&eight, 1), vec![0x48]);
        // 自定义表已含 0 时不重复插入零点
        assert_eq!(
            linear_levels_from_off(&[0x00, 0x43, 0x48], 3),
            vec![0x00, 0x43, 0x48]
        );
        assert!(linear_levels_from_off(&[], 4).is_empty());
    }

    #[test]
    fn custom_levels_may_start_from_off() {
        // 「从 0 到最大线性四档」：0 是熄灭，其余三档线性铺到最亮
        let mut c = Config {
            manual_brightness_levels: Some(vec![0x00, 0x43, 0x46, 0x48]),
            ..Config::default()
        };
        let notes = c.sanitize();
        assert!(
            c.manual_brightness_levels.is_some(),
            "含 0 的自定义表应被保留，却回落到线性等分: {notes:?}"
        );
        assert_eq!(c.manual_steps(), 4);
        assert_eq!(c.level_for_step(1), Some(VAL_OFF));
        assert_eq!(c.level_for_step(2), Some(0x43));
        assert_eq!(c.level_for_step(3), Some(0x46));
        assert_eq!(c.level_for_step(4), Some(0x48));
    }

    #[test]
    fn custom_levels_must_be_increasing() {
        let mut c = Config {
            manual_brightness_levels: Some(vec![0x48, 0x46, 0x43, 0x41]),
            ..Config::default()
        };
        let notes = c.sanitize();
        assert!(!notes.is_empty(), "倒序表应给出告警");
        // 丢弃自定义后回落到默认线性四档（首档为熄灭）
        assert!(c.manual_brightness_levels.is_none());
        assert_eq!(c.manual_steps(), MANUAL_BRIGHTNESS_STEPS);
        assert_eq!(c.level_for_step(1), Some(VAL_OFF));
        assert_eq!(c.level_for_step(4), Some(0x48));
    }

    #[test]
    fn custom_levels_still_drop_out_of_range() {
        let mut c = Config {
            manual_brightness_levels: Some(vec![0x00, 0x30, 0x43, 0x48]),
            ..Config::default()
        };
        let notes = c.sanitize();
        assert!(!notes.is_empty(), "越界档位应被剔除并告警");
        assert_eq!(c.manual_levels(), vec![0x00, 0x43, 0x48]);
    }

    #[test]
    fn linear_levels_helper_handles_edge_cases() {
        use crate::brightness::linear_levels;
        // 空表 / 0 档 → 空
        assert!(linear_levels(&[], 4).is_empty());
        assert!(linear_levels(&[0x41, 0x48], 0).is_empty());
        // 单档恒定把最亮当唯一档（不会因为「只有一档」变暗）
        assert_eq!(linear_levels(&[0x41, 0x48], 1), vec![0x48]);
        // 档数与表长相同 → 原样返回
        assert_eq!(
            linear_levels(&[0x41, 0x44, 0x48], 3),
            vec![0x41, 0x44, 0x48]
        );
        // 8 档取 2 点 → 两端
        assert_eq!(
            linear_levels(&default_brightness_levels(), 2),
            vec![0x41, 0x48]
        );
    }

    #[test]
    fn manual_step_ignored_when_following() {
        let mut c = Config {
            manual_brightness_step: 1,
            ..Config::default()
        };
        assert!(c.follow_keyboard_brightness);
        // 跟随开：effective_levels 用整表，固定档不参与点灯
        assert_eq!(c.effective_levels().len(), 8);
        c.follow_keyboard_brightness = false;
        assert!(c.effective_levels().is_empty());
        // 跟随关：按手动档常亮（1 档 = 熄灭档，常亮退化为 0x01；3 档 = 0x45）
        assert_eq!(c.static_on_value(), 0x01);
        c.manual_brightness_step = 3;
        assert_eq!(c.static_on_value(), 0x45);
        c.manual_brightness_step = 4;
        assert_eq!(c.static_on_value(), 0x48);
        c.manual_brightness_step = 0;
        assert_eq!(c.static_on_value(), VAL_OFF);
    }

    #[test]
    fn manual_step_clamped_on_load() {
        let mut c: Config = serde_json::from_str(r#"{"manual_brightness_step":9}"#).unwrap();
        let notes = c.sanitize();
        assert_eq!(c.manual_brightness_step, MANUAL_BRIGHTNESS_STEPS);
        assert!(notes.iter().any(|n| n.contains("manual_brightness_step")));
    }

    #[test]
    fn manual_levels_use_custom_table() {
        let mut c = Config {
            // 自定义档表：档数由长度决定（这里 3 档），亮度逐档照写，不做等分
            manual_brightness_levels: Some(vec![0x41, 0x44, 0x48]),
            ..Config::default()
        };
        c.sanitize();
        assert_eq!(c.manual_steps(), 3);
        assert_eq!(c.level_for_step(0), Some(VAL_OFF));
        assert_eq!(c.level_for_step(1), Some(0x41));
        assert_eq!(c.level_for_step(2), Some(0x44));
        assert_eq!(c.level_for_step(3), Some(0x48));
        assert_eq!(c.level_for_step(4), None);
        // 空档表：退化为 0x01 点亮
        c.manual_brightness_levels = None;
        c.brightness_levels.clear();
        assert_eq!(c.level_for_step(2), Some(VAL_ON));
    }

    #[test]
    fn custom_levels_are_sanitized() {
        // 越界值剔除 → 表变短，档数跟着变（0 是合法值，代表熄灭）
        let mut c: Config =
            serde_json::from_str(r#"{"manual_brightness_levels":["0x41","0x30","0x46","0x48"]}"#)
                .unwrap();
        let notes = c.sanitize();
        assert_eq!(c.manual_brightness_levels, Some(vec![0x41, 0x46, 0x48]));
        assert_eq!(c.manual_steps(), 3);
        // 提示文案随语言变化，这里只要求「有提示」且指向该配置项
        assert!(
            notes.iter().any(|n| n.contains("manual_brightness_levels")),
            "notes={notes:?}"
        );
        // 全越界 → 放弃自定义，回落线性等分
        let mut c: Config =
            serde_json::from_str(r#"{"manual_brightness_levels":["0x30","0xff"]}"#).unwrap();
        let notes = c.sanitize();
        assert_eq!(c.manual_brightness_levels, None);
        assert_eq!(c.manual_levels().len(), MANUAL_BRIGHTNESS_STEPS as usize);
        assert_eq!(notes.len(), 2, "越界剔除 + 回落各一条提示: {notes:?}");
        // 超长表：硬件只有 8 个物理档，10 档截断后必然出现重复值 → 判非递增 → 回落
        let mut c: Config =
            serde_json::from_str(r#"{"manual_brightness_levels":[65,65,65,65,65,65,65,65,65,65]}"#)
                .unwrap();
        let notes = c.sanitize();
        assert!(c.manual_brightness_levels.is_none());
        assert_eq!(c.manual_levels().len(), MANUAL_BRIGHTNESS_STEPS as usize);
        assert!(notes.len() >= 2, "截断 + 非递增各一条提示: {notes:?}");
    }

    #[test]
    fn custom_table_len_drives_step_clamp() {
        let mut c: Config = serde_json::from_str(
            r#"{"manual_brightness_levels":[65,68],"manual_brightness_step":4}"#,
        )
        .unwrap();
        let notes = c.sanitize();
        assert_eq!(c.manual_steps(), 2);
        assert_eq!(c.manual_brightness_step, 2);
        assert!(notes.iter().any(|n| n.contains("manual_brightness_step")));
    }

    #[test]
    fn gh3xlndz_numlock_frames_is_62() {
        let c = Config::default();
        assert_eq!(c.numlock_frames(), 62);
        assert_eq!(c.numlock_frames() * c.frame_ms, 496);
    }

    #[test]
    fn partial_json_keeps_official_defaults() {
        let c: Config = serde_json::from_str(r#"{"key_mode":"digits","grab":false}"#).unwrap();
        assert_eq!(c.key_mode, KeyMode::Digits);
        assert!(!c.grab);
        // 未给出的字段必须是官方默认
        assert_eq!(c.gesture_trig_um, 4838.0);
        assert_eq!(c.frame_ms, 8);
    }

    #[test]
    fn mixed_mode_accepts_bool_and_string() {
        let a: MixedMode = serde_json::from_str("true").unwrap();
        let b: MixedMode = serde_json::from_str("\"auto\"").unwrap();
        let c: MixedMode = serde_json::from_str("false").unwrap();
        assert_eq!(a, MixedMode::On);
        assert_eq!(b, MixedMode::Auto);
        assert_eq!(c, MixedMode::Off);
    }

    #[test]
    fn invalid_key_mode_falls_back_to_keypad() {
        let c: Config = serde_json::from_str(r#"{"key_mode":"numpad"}"#).unwrap();
        assert_eq!(c.key_mode, KeyMode::Keypad);
    }

    #[test]
    fn icon_numlock_zone_is_rejected() {
        let mut c: Config = serde_json::from_str(r#"{"numlock_zone":"icon"}"#).unwrap();
        let notes = c.sanitize();
        assert_eq!(c.numlock_zone, "cell");
        assert!(notes.iter().any(|n| n.contains("icon")));
    }

    #[test]
    fn brightness_levels_accept_int_and_hex_string() {
        let ints: Config = serde_json::from_str(r#"{"brightness_levels":[65,72]}"#).unwrap();
        assert_eq!(ints.brightness_levels, vec![0x41, 0x48]);
        // 旧版本写出的 "0x41" 字符串必须还能读（否则整份配置会悄悄退回默认）
        let hex: Config =
            serde_json::from_str(r#"{"brightness_levels":["0x41","0x45","0x48"]}"#).unwrap();
        assert_eq!(hex.brightness_levels, vec![0x41, 0x45, 0x48]);
        let dec: Config = serde_json::from_str(r#"{"brightness_levels":["65"]}"#).unwrap();
        assert_eq!(dec.brightness_levels, vec![0x41]);
        let missing: Config = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(missing.brightness_levels.len(), 8);
    }

    #[test]
    fn dump_config_roundtrips_through_load() {
        // to_json 写出的东西必须能被 load 原样读回（v5 曾因 "0x41" 字符串读不回来，
        // 导致 bright 命令写入后 follow/手动档全部丢失，退回默认值）
        let dir = std::env::temp_dir();
        let path = dir.join(format!("asus-numpad-roundtrip-{}.json", std::process::id()));
        let c = Config {
            follow_keyboard_brightness: false,
            manual_brightness_step: 2,
            ..Config::default()
        };
        std::fs::write(&path, c.to_json()).unwrap();

        let (back, notes) = Config::load(&path);
        let _ = std::fs::remove_file(&path);
        assert!(notes.is_empty(), "notes={notes:?}");
        assert!(!back.follow_keyboard_brightness);
        assert_eq!(back.manual_brightness_step, 2);
        assert_eq!(back.brightness_levels, c.brightness_levels);
        assert_eq!(back.level_for_step(1), Some(VAL_OFF));
        assert_eq!(back.level_for_step(2), Some(0x43));
        assert_eq!(back.level_for_step(4), Some(0x48));
    }

    #[test]
    fn bad_brightness_levels_reports_index() {
        let err =
            serde_json::from_str::<Config>(r#"{"brightness_levels":[65,"oops"]}"#).unwrap_err();
        assert!(err.to_string().contains("brightness_levels[1]"), "{err}");
    }

    #[test]
    fn zero_frame_ms_is_clamped() {
        let mut c: Config =
            serde_json::from_str(r#"{"frame_ms":0,"numlock_trigger_frames":0}"#).unwrap();
        c.sanitize();
        assert_eq!(c.frame_ms, 1);
        assert_eq!(c.numlock_trigger_frames, 1);
    }
}
