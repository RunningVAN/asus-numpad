//! 机型（SKU）闸门：每个发行包只服务一组机型。
//!
//! ## 为什么按机型拆包
//!
//! 驱动真正依赖的是**触摸板 HID 模组**（`ASUE1416` 等），不是整机型号；但发行层面
//! 若只出一个「宣称支持全部机型」的包，用户既看不出自己这台有没有被适配，也无法在
//! 装错时被拦住。所以按机型拆成多个包，每个包在编译期把机型注入进来
//! （环境变量 [`SKU_ENV`]），运行期读 DMI 的 `product_name` 比对：
//!
//! * 机型对上 → 正常走布局闸门（`verified` / `family` / `unsupported` 三档不变）；
//! * 机型对不上 → 直接拒绝启动，并提示该装哪个包。
//!
//! 判定只看「DMI 里是否出现本包机型前缀」。同型号的高低配（如魔霸新锐 2023 的
//! `EAGX` 有 NumberPad、`EACX` 没有）DMI 前缀相同，闸门不会误伤——那种情况本来
//! 就由「有没有这个触摸板硬件」决定，驱动找不到设备自然不会动键盘。
//!
//! ## 不做的事
//!
//! 这里不做布局/键表判定，那仍然由 [`crate::layout`] 负责。机型闸门只回答一个问题：
//! **这个包是不是给这台机器的。**

use std::path::Path;

/// 打包脚本用来注入机型的环境变量（编译期读取）。
pub const SKU_ENV: &str = "ASUS_NUMPAD_SKU";

/// 一个发行包对应的机型。
#[derive(Debug)]
pub struct Sku {
    /// 包名后缀，如 `g614` → `asus-numpad-g614`
    pub id: &'static str,
    /// 给用户看的机型说明
    pub display: &'static str,
    /// DMI `product_name` 里出现的机型前缀；为空表示不做机型校验
    pub models: &'static [&'static str],
    /// 该机型官方规格里的 NumberPad 结论，用于输出自证
    pub note: &'static str,
}

/// 机型表。新增机型只加一行，并同步 `packaging/skus.tsv`。
///
/// ## 合并规则（1.0.0-14 起）
///
/// 一个包服务一个**触摸板模组族**，而不是一个整机 SKU。同族的依据是
/// `layout.rs` 的 [`crate::layout::HID_LAYOUTS`]（官方 INF 的 HID 名 → 键表）：
/// 模组名相同（如 `ASUE1416` / `ASUF1416`）就一定是同一份键表、同一套几何。
///
/// * `g614` 吸收 `g634`：同一块 ELAN 触摸板（04F3:3275 / ASUE1416），真机 G634JY 已验证；
/// * `g615` 吸收 `g635`、`h7606`：PixArt ASCP1400 / ASCF1400 → GH4XLNDZ 同族；
/// * `gx550` 吸收 `gx650`：两代冰刃双屏上游实测同属 gx551 键位族（模组未真机验证）。
///
/// 合并只影响「机型闸门」的 DMI 白名单，运行期选哪套键表仍由实际 HID 模组名决定，
/// 所以合并不改变闸门的保守性：模组不在 `HID_LAYOUTS` 里，照样拒绝启动。
///
/// 被吸收的旧包名写在 `packaging/skus.tsv` 第 5 列，出包时由本包 Obsoletes 掉，
/// 老用户 `dnf upgrade` 会自动接管，不需要先手工卸载旧机型包。
pub const SKUS: &[Sku] = &[
    Sku {
        id: "g533",
        display: "ASUS 枪神6（G533）",
        models: &["G533"],
        note: "模组 ASUE1403/ELAN1403 → 键表 GH2XLNDZ（同族，未逐键实测）",
    },
    Sku {
        id: "g614",
        display: "ASUS 枪神7 / 魔霸新锐 2023 / 枪神7 超竞版 16 英寸（G614 / G634）",
        models: &["G614", "G634"],
        note: "合并包：G614 与 G634 同一块触摸板（ASUE1416 / ASUF1416 → GH3XLNDZ，真机 G634JY 已验证）；低配 EACX 无此硬件，靠设备缺失自然不动键盘",
    },
    Sku {
        id: "g615",
        display: "ASUS 枪神9 / 枪神10 16 英寸、魔霸新锐 2025、超竞版、ProArt P16（G615 / G635 / H7606）",
        models: &["G615", "G635", "H7606"],
        note: "合并包：PixArt ASCP1400 / ASCF1400 → GH4XLNDZ（同族，需逐键核对）",
    },
    Sku {
        id: "gx550",
        display: "ASUS 冰刃双屏 15 / 16 英寸（GX550 / GX551 / GX650）",
        models: &["GX550", "GX551", "GX650"],
        note: "合并包：两代双屏上游实测同属 gx551 键位族；GX550 模组 ASUE1408 → GVM_A，未真机验证",
    },
    Sku {
        id: "ux582",
        display: "灵耀双屏（UX582 / UX482）",
        models: &["UX582", "UX482"],
        note: "模组 ASUE1406 / ELAN1406 → 键表 NVM（未验证）；UX482 待规格页复核",
    },
    Sku {
        id: "universal",
        display: "全机型兼容包（不做机型校验）",
        models: &[],
        note: "仅在不确定机型时兜底使用",
    },
];

/// 编译期由 `build.rs` 生成的机型常量（见仓库根 `build.rs`）。
pub mod build {
    include!(concat!(env!("OUT_DIR"), "/build_sku.rs"));
}

/// 当前二进制所属机型。未注入时 `build.rs` 会写成 `universal`。
pub fn build_sku() -> &'static Sku {
    by_id(build::BUILD_SKU_ID).unwrap_or_else(|| by_id("universal").expect("SKUS 必须含 universal"))
}

/// 本二进制的包名（如 `asus-numpad-g614`），由 `build.rs` 在编译期固定。
///
/// 出包脚本用它 grep 二进制自证机型，避免「包名对了、二进制还是上一台」的错包。
pub fn build_package_name() -> &'static str {
    build::BUILD_PACKAGE_NAME
}

/// 按 id 查机型（大小写不敏感）。
pub fn by_id(id: &str) -> Option<&'static Sku> {
    SKUS.iter()
        .find(|s| s.id.eq_ignore_ascii_case(id.trim()))
}

/// 机型校验结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DmiVerdict {
    /// DMI 命中本包机型
    Match,
    /// DMI 明确是别的机型
    Mismatch(String),
    /// 读不到 DMI（非 Linux、容器、权限受限）
    Unknown,
    /// 本包不做机型校验（`universal`）
    NotEnforced,
}

impl DmiVerdict {
    /// 是否允许继续启动。
    pub fn allows_run(&self) -> bool {
        matches!(self, DmiVerdict::Match | DmiVerdict::Unknown | DmiVerdict::NotEnforced)
    }

    /// 供日志/自检输出的短标签。
    pub fn as_str(&self) -> &'static str {
        match self {
            DmiVerdict::Match => "match",
            DmiVerdict::Mismatch(_) => "mismatch",
            DmiVerdict::Unknown => "unknown",
            DmiVerdict::NotEnforced => "not-enforced",
        }
    }
}

/// 纯逻辑：给定机型表与 DMI 型号，判定是否对得上。
pub fn verdict_for(sku: &Sku, dmi: Option<&str>) -> DmiVerdict {
    if sku.models.is_empty() {
        return DmiVerdict::NotEnforced;
    }
    let Some(dmi) = dmi.map(str::trim).filter(|s| !s.is_empty()) else {
        return DmiVerdict::Unknown;
    };
    let upper = dmi.to_ascii_uppercase();
    if sku.models.iter().any(|m| upper.contains(&m.to_ascii_uppercase())) {
        DmiVerdict::Match
    } else {
        DmiVerdict::Mismatch(dmi.to_string())
    }
}

/// 读本机 DMI 型号（Linux `/sys/class/dmi/id/product_name`，其它平台返回 `None`）。
pub fn dmi_product_name() -> Option<String> {
    let text = std::fs::read_to_string("/sys/class/dmi/id/product_name").ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// 从指定 sysfs 根目录读 DMI 型号（便于测试与容器内排查）。
pub fn dmi_product_name_at(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("class/dmi/id/product_name")).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// 当前二进制的机型判定。
pub fn current_verdict() -> DmiVerdict {
    verdict_for(build_sku(), dmi_product_name().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sku_ids_are_unique_and_complete() {
        let mut ids: Vec<&str> = SKUS.iter().map(|s| s.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "SKU id 必须唯一");
        assert!(by_id("universal").is_some(), "必须保留 universal 兜底");
        assert!(by_id("G614").is_some(), "机型查询应忽略大小写");
        for merged in ["g634", "g635", "gx650", "h7606"] {
            assert!(by_id(merged).is_none(), "{merged} 已并入同族包，不应再单独存在");
        }
        for s in SKUS {
            assert!(!s.display.is_empty(), "{} 缺少展示名", s.id);
            assert!(!s.note.is_empty(), "{} 缺少说明", s.id);
        }
    }

    #[test]
    fn g614_pack_covers_g634_real_dmi() {
        let sku = by_id("g614").unwrap();
        assert_eq!(
            verdict_for(sku, Some("ROG Strix SCAR 16 G634JY_G634JY")),
            DmiVerdict::Match
        );
    }

    #[test]
    fn plus_18inch_is_mismatch_for_16inch_pack() {
        let sku = by_id("g614").unwrap();
        let v = verdict_for(sku, Some("ROG Strix SCAR 18 G834JY_G834JY"));
        assert!(matches!(v, DmiVerdict::Mismatch(_)));
        assert!(!v.allows_run());
    }

    #[test]
    fn missing_dmi_does_not_block() {
        let sku = by_id("g615").unwrap();
        assert_eq!(verdict_for(sku, None), DmiVerdict::Unknown);
        assert_eq!(verdict_for(sku, Some("   ")), DmiVerdict::Unknown);
        assert!(verdict_for(sku, None).allows_run());
    }

    #[test]
    fn universal_pack_never_blocks() {
        let sku = by_id("universal").unwrap();
        assert_eq!(verdict_for(sku, Some("Anything G999ZZ")), DmiVerdict::NotEnforced);
        assert!(verdict_for(sku, Some("Anything G999ZZ")).allows_run());
    }

    #[test]
    fn dmi_file_read_and_mismatch_path() {
        let root = std::env::temp_dir().join("asus-numpad-dmi-selftest");
        let dir = root.join("class/dmi/id");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("product_name"), "ROG Strix SCAR 16 G634JY_G634JY\n").unwrap();

        // 读到并去掉换行
        assert_eq!(
            dmi_product_name_at(&root).as_deref(),
            Some("ROG Strix SCAR 16 G634JY_G634JY")
        );
        // 同一台机器，合并后的 g614 包放行，g615 包拒绝
        let dmi = dmi_product_name_at(&root);
        assert_eq!(
            verdict_for(by_id("g614").unwrap(), dmi.as_deref()),
            DmiVerdict::Match
        );
        assert!(!verdict_for(by_id("g615").unwrap(), dmi.as_deref()).allows_run());

        // 空文件视为未知，不拦
        std::fs::write(dir.join("product_name"), "  \n").unwrap();
        assert_eq!(dmi_product_name_at(&root), None);
        assert!(verdict_for(by_id("g614").unwrap(), None).allows_run());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dual_screen_and_proart_packs_match_their_dmi() {
        // 冰刃双屏 15 英寸（GX550 / GX551）
        let gx550 = by_id("gx550").unwrap();
        for dmi in [
            "ROG Zephyrus Duo 15 GX550LXS_GX550LXS",
            "ROG Zephyrus Duo 15 SE GX551QS_GX551QS",
        ] {
            assert_eq!(verdict_for(gx550, Some(dmi)), DmiVerdict::Match, "{dmi}");
        }
        // 冰刃 6 / 7 双屏 16 英寸（GX650）已并入同一包
        assert_eq!(
            verdict_for(gx550, Some("ROG Zephyrus Duo 16 GX650PZ_GX650PZ")),
            DmiVerdict::Match
        );
        // 灵耀双屏（UX582 / UX482）
        let ux582 = by_id("ux582").unwrap();
        for dmi in ["ASUS Zenbook Duo UX582LR_UX582LR", "ASUS Zenbook Duo UX482EG_UX482EG"] {
            assert_eq!(verdict_for(ux582, Some(dmi)), DmiVerdict::Match, "{dmi}");
        }
        // ProArt P16（H7606）已并入 g615（同为 PixArt ASCP/ASCF 模组族）
        let g615 = by_id("g615").unwrap();
        for dmi in [
            "ASUS ProArt P16 H7606WI_H7606WI",
            "ROG Strix G16 G615LR_G615LR",
            "ROG Strix SCAR 16 G635LX_G635LX",
        ] {
            assert_eq!(verdict_for(g615, Some(dmi)), DmiVerdict::Match, "{dmi}");
        }
        // 跨包互不误放：冰刃双屏包不认枪神，枪神包不认冰刃
        assert!(!verdict_for(gx550, Some("ROG Strix SCAR 16 G634JY_G634JY")).allows_run());
        assert!(!verdict_for(by_id("g614").unwrap(), Some("ROG Zephyrus Duo 16 GX650PZ_GX650PZ")).allows_run());
    }

    #[test]
    fn unknown_build_sku_falls_back_to_universal() {
        let sku = build_sku();
        assert!(by_id(sku.id).is_some());
    }
}
