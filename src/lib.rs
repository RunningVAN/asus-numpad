//! asus-numpad —— ASUS 笔记本触摸板数字小键盘驱动的 Linux 用户态实现。
//!
//! 结构与官方驱动一一对应：
//!
//! | 本工程 | 官方对应 | 职责 |
//! |---|---|---|
//! | `keycodes` | `include/uapi/linux/input-event-codes.h` | 事件码常量 |
//! | `layout` | `NUMPADLAYOUT_GH3XLNDZ_00..14` | 0.1 mm 布局 → 绝对坐标矩形 → 键码 |
//! | `config` | `AsusNUMPADFilter.inf` 的 `AlgoParameters` | 帧数 / 阈值 / 点灯取值 |
//! | `engine` | `AsusNUMPADFilter` 的 Algo 状态机 | 触点状态机、点灯联动、混合模式 |
//! | `linux` | — | evdev 抓取、uinput 虚拟设备、hidraw/i2c 点灯、热插拔 |
//!
//! 纯逻辑部分（`layout` / `engine`）不依赖 Linux，可在任意平台跑单测；
//! 硬件相关代码全部用 trait 注入，真机接上即用。
//!
//! ## 真机验证状态
//!
//! 逻辑层已通过完整离线用例，但**尚未在真机（ROG Strix SCAR 16 G634JY）上验证**。
//! 其中一处来源差异必须如实说明：官方 INF 的 `NUMPAD_09 iNumLockMaxVaildDist = 2741`
//! 在 `iUnit = 100 µm` 下只有 0.27 mm，与「长按按住不动」的物理语义不符，
//! 本实现按字面值使用，真机上若出现长按被判为滑动、NumLock 切不动，请先排查此处。

pub mod brightness;
pub mod config;
pub mod engine;
pub mod i18n;
pub mod keycodes;
pub mod layout;
pub mod led_policy;
pub mod sku;

#[cfg(target_os = "linux")]
pub mod linux;

pub use config::{Config, VERSION};
pub use engine::Engine;
pub use i18n::Lang;
pub use layout::{Geometry, KeyMode, Layout, GH3XLNDZ};

/// 与上游一致的启动横幅（随界面语言变化）。
pub fn banner() -> String {
    i18n::tf("app.banner", &[("version", VERSION)])
}

/// 真机验证免责声明（`--check` / `--dump` 启动时输出）。
pub fn verification_notice() -> String {
    i18n::t("app.notice")
}
