//! `dump`：打印生效配置。

use anyhow::Result;
use asus_numpad::banner;
use asus_numpad::config::Config;
use asus_numpad::i18n::{self};

pub(crate) fn cmd_dump(cfg: &Config) -> Result<i32> {
    println!("{}", banner());
    println!("{}", i18n::t("dump.mode"));
    println!("{}", cfg.summary());

    #[cfg(target_os = "linux")]
    {
        asus_numpad::linux::dump_events(cfg)?;
        Ok(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        println!("{}", i18n::t("dump.nonlinux"));
        Ok(1)
    }
}
