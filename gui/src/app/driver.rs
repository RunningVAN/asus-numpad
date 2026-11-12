//! 驱动配置文件（`asus-numpad.json`）在界面里的编辑草稿。
//!
//! 界面直接读写驱动自己那份配置，而不是另存一份影子副本：文件格式、字段名、默认值
//! 全部来自 [`asus_numpad::config`]，和 `asus-numpad config` / 手改文件看到的是同一份东西。
//!
//! 写路径沿用驱动自己的规则（[`Config::writable_path`]）：普通用户落
//! `~/.config/asus-numpad/asus-numpad.json`，root 才写 `/etc`。界面不 sudo，所以
//! 正常情况永远不需要提权。

use std::path::PathBuf;

use asus_numpad::config::Config;

/// 一份可编辑的驱动配置。
pub struct Draft {
    /// 读取时的来源路径（可能是 `/etc` 下的历史配置）
    pub path: PathBuf,
    /// 实际落盘路径（普通用户即用户级路径）
    pub write_path: PathBuf,
    /// 读取时文件是否已存在；不存在说明界面里保存的是首次创建
    pub existed: bool,
    pub cfg: Config,
    /// 读取过程中的提示（解析失败、字段被夹回合法区间等）
    pub notes: Vec<String>,
    /// 界面改过但还没落盘
    pub dirty: bool,
}

impl Draft {
    pub fn load() -> Self {
        let path = Config::default_path();
        let existed = path.exists();
        // 非 Linux 只用于开发机预览 Windows/Linux 两套界面：一律写用户级路径，
        // 免得因为没有 geteuid 语义而把「可写」误判成 /etc。
        let write_path = if cfg!(target_os = "linux") {
            Config::writable_path(&path)
        } else {
            Config::user_config_path()
        };
        let (cfg, notes) = Config::load(&path);
        Self {
            path,
            write_path,
            existed,
            cfg,
            notes,
            dirty: false,
        }
    }

    /// 丢掉界面上的改动，按磁盘重新读一遍。
    pub fn reload(&mut self) {
        *self = Self::load();
    }

    /// 恢复成驱动默认值（不落盘，需要再点保存）。
    pub fn reset(&mut self) {
        self.cfg = Config::default();
        self.notes.clear();
        self.dirty = true;
    }

    /// 落盘。返回给状态栏用的文案。
    pub fn save(&mut self) -> anyhow::Result<String> {
        self.cfg.sanitize();
        let text = format!("{}\n", self.cfg.to_json());
        // 复用设置的原子写：临时文件 + rename，写一半崩了也不会把配置文件截断
        crate::settings::write_atomic(&self.write_path, &text)
            .map_err(|err| anyhow::anyhow!("写入 {} 失败：{err}", self.write_path.display()))?;
        self.notes.clear();
        self.dirty = false;
        self.existed = true;
        Ok(format!("驱动配置已写入 {}", self.write_path.display()))
    }

    /// 当前编辑内容的 JSON 预览（与 `asus-numpad --dump-config` 同一份序列化）。
    pub fn json(&self) -> String {
        self.cfg.to_json()
    }
}
