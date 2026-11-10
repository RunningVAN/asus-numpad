//! 界面设置的持久化。
//!
//! 只存「用户的选择」，不存硬件状态：档位、联动开关、上次选中的接口。
//! Windows 下放 `%APPDATA%\asus-numpad-gui\settings.json`，其他平台放
//! `~/.config/asus-numpad-gui/settings.json`。
//!
//! 界面设置与驱动的配置文件（`asus-numpad.toml`）是两份东西：驱动那份写机型与
//! 布局，这里只写「这个界面自己记的事」，改这里不会影响驱动行为。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use asus_numpad::config::VAL_BRIGHTNESS_MAX;

const APP_DIR: &str = "asus-numpad-gui";
const FILE: &str = "settings.json";

/// 界面设置。字段全部带默认值，旧版本配置缺项也能读。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 当前亮度档（1..=`steps`）
    pub step: u8,
    /// 键盘侧档数：与键盘背光对齐时通常是 4 档，单独控制可用 8 档
    pub steps: u8,
    /// 键盘 4 档各自对应的触摸板亮度 level（`0x00` 或 `0x41..=0x48`）。
    ///
    /// 只在 `steps == 4`（对齐键盘四档）时生效；8 档组仍按线性等分。表结构与驱动
    /// 配置里的 `manual_brightness_levels` 相同，可以整表写过去。
    pub kbd_map: Vec<u8>,
    /// 当前是否点亮
    pub on: bool,
    /// NumLock 关闭时自动熄灯（Windows 没有键盘背光 sysfs，用它替代跟随逻辑）
    pub follow_numlock: bool,
    /// 联动轮询间隔（毫秒）
    pub poll_ms: u64,
    /// 上次选中的设备接口路径
    pub device_path: Option<String>,
    /// 启动时把上次的档位重新下发一次（休眠/热插拔后 EC 会丢状态）
    pub apply_on_start: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            step: crate::kbdmap::KBD_STEPS as u8,
            steps: crate::kbdmap::KBD_STEPS as u8,
            kbd_map: crate::kbdmap::default_map(),
            on: true,
            follow_numlock: false,
            poll_ms: 500,
            device_path: None,
            apply_on_start: true,
        }
    }
}

impl Settings {
    /// 把外部写入的越界值夹回合法区间，避免配置被手改成 0 或 99 后直接下发。
    pub fn sanitize(&mut self) {
        if !matches!(self.steps, 4 | 8) {
            self.steps = VAL_BRIGHTNESS_MAX;
        }
        if self.step == 0 || self.step > self.steps {
            self.step = self.steps.clamp(1, self.steps);
        }
        crate::kbdmap::sanitize(&mut self.kbd_map);
        self.poll_ms = self.poll_ms.clamp(100, 5000);
    }
}

/// 配置文件路径。
pub fn config_path() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join(APP_DIR).join(FILE);
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config").join(APP_DIR).join(FILE);
        }
    }
    PathBuf::from(APP_DIR).join(FILE)
}

/// 读配置；文件不存在或损坏时返回默认值（不阻断界面启动）。
pub fn load() -> Settings {
    let path = config_path();
    let mut settings = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<Settings>(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    };
    settings.sanitize();
    settings
}

/// 写配置。
pub fn save(settings: &Settings) -> Result<()> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("创建配置目录失败：{}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(settings).context("序列化设置失败")?;
    std::fs::write(&path, text).with_context(|| format!("写入设置失败：{}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_consistent() {
        let s = Settings::default();
        assert!(s.step >= 1 && s.step <= s.steps);
        assert!((4..=8).contains(&s.steps));
    }

    #[test]
    fn sanitize_clamps_bad_values() {
        let mut s = Settings {
            step: 99,
            steps: 5,
            poll_ms: 5,
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(s.steps, 8);
        assert_eq!(s.step, 8);
        assert_eq!(s.poll_ms, 100);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"step": 2}"#).unwrap();
        assert_eq!(s.step, 2);
        assert_eq!(s.steps, crate::kbdmap::KBD_STEPS as u8);
        assert_eq!(s.kbd_map, crate::kbdmap::default_map());
        assert!(s.apply_on_start);
    }

    #[test]
    fn old_settings_without_kbd_map_get_defaults() {
        // 旧版设置文件没有 kbd_map 字段，读出来必须补齐成默认四档，而不是空表
        let s: Settings = serde_json::from_str(r#"{"step": 3, "steps": 4}"#).unwrap();
        assert_eq!(s.kbd_map.len(), crate::kbdmap::KBD_STEPS);
        assert_eq!(s.kbd_map, crate::kbdmap::default_map());
    }

    #[test]
    fn sanitize_fixes_kbd_map() {
        let mut s = Settings {
            kbd_map: vec![0x48, 0x10],
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(s.kbd_map, crate::kbdmap::default_map());

        let mut s = Settings {
            kbd_map: vec![0x10, 0x41, 0x45, 0x99],
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(s.kbd_map, vec![0x41, 0x41, 0x45, 0x48]);
    }
}
