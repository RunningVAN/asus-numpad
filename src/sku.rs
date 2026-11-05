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
    /// 该机型号对应的触摸板模组名（`ASUE1416` / `ASCP1400` …），与
    /// [`crate::layout::HID_LAYOUTS`] 的键同源。硬件层面的第二判据，见 [`HardwareProbe`]。
    pub modules: &'static [&'static str],
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
/// * `g614` 吸收 `g634`：同一块 ELAN 触摸板（04F3:3275 / ASUE1416），键表 GH3XLNDZ；
/// * `g615` 吸收 `g635`、`h7606`：PixArt ASCP1400 / ASCF1400 → GH4XLNDZ 同族；
/// * `gx550` 吸收 `gx650`：两代冰刃双屏同属 gx551 键位族。
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
        modules: &["ASUE1403", "ELAN1403"],
        note: "模组 ASUE1403 / ELAN1403 → 键表 GH2XLNDZ（同族）",
    },
    Sku {
        id: "g614",
        display: "ASUS 枪神7 / 魔霸新锐 2023 / 枪神7 超竞版 16 英寸（G614 / G634）",
        models: &["G614", "G634"],
        modules: &["ASUE1416", "ASUF1416"],
        note: "合并包：G614 与 G634 同一块触摸板（ASUE1416 / ASUF1416 → GH3XLNDZ）；低配 EACX 无此硬件，靠设备缺失自然不动键盘",
    },
    Sku {
        id: "g615",
        display: "ASUS 枪神9 / 枪神10 16 英寸、魔霸新锐 2025、超竞版、ProArt P16（G615 / G635 / H7606）",
        models: &["G615", "G635", "H7606"],
        modules: &["ASCP1400", "ASCF1400"],
        note: "合并包：PixArt ASCP1400 / ASCF1400 → GH4XLNDZ（同族）",
    },
    Sku {
        id: "gx550",
        display: "ASUS 冰刃双屏 15 / 16 英寸（GX550 / GX551 / GX650）",
        models: &["GX550", "GX551", "GX650"],
        modules: &["ASUE1408", "ASUE1404"],
        note: "合并包：GX550 / GX551 / GX650 同属 gx551 键位族；GX550 模组 ASUE1408 → GVM_A",
    },
    Sku {
        id: "ux582",
        display: "灵耀双屏（UX582 / UX482）",
        models: &["UX582", "UX482"],
        modules: &["ASUE1406", "ELAN1406"],
        note: "模组 ASUE1406 / ELAN1406 → 键表 NVM",
    },
    Sku {
        id: "universal",
        display: "全机型兼容包（不做机型校验）",
        models: &[],
        modules: &[],
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
    SKUS.iter().find(|s| s.id.eq_ignore_ascii_case(id.trim()))
}

/// DMI 身份快照：厂商 + 产品名 + 主板名 + 产品家族。
///
/// 单看 `product_name` 不够用，实际机器上这几种情况都真实存在：
///
/// * 部分机型 `product_name` 只有营销名（`ROG Strix SCAR 16`），机型编号 `G614` 只落在
///   `board_name` 或 `product_family` 里；
/// * 部分机型 `product_name` 是 `G614JY_G614JY`，编号重复两遍（老前缀匹配能吃下，
///   但 `G614JY` 里的 `G614` 也可能被 `G614X` 的规则误吃，这里用整串多字段比对更稳）；
/// * 白牌 / 虚拟机会写 `System Product Name`、`To Be Filled By O.E.M.` 这类占位串，
///   当机型处理会误判成「不是 ASUS 机器」而拦下正常安装。
///
/// 所以判定改成「多字段取并集 + 占位串归一 + 厂商字段定性质」。
#[derive(Debug, Clone, Default)]
pub struct DmiIdentity {
    /// `/sys/class/dmi/id/sys_vendor`，正常为 `ASUSTeK COMPUTER INC.`
    pub sys_vendor: Option<String>,
    /// `/sys/class/dmi/id/board_vendor`
    pub board_vendor: Option<String>,
    /// `/sys/class/dmi/id/product_name`，如 `ROG Strix SCAR 16 G614JY_G614JY`
    pub product_name: Option<String>,
    /// `/sys/class/dmi/id/board_name`，部分机型只有这里有编号
    pub board_name: Option<String>,
    /// `/sys/class/dmi/id/product_family`
    pub product_family: Option<String>,
}

/// DMI 里常见的占位串（BIOS 没填真值时就写这些），一律当「读不到」处理。
const DMI_PLACEHOLDERS: &[&str] = &[
    "SYSTEM PRODUCT NAME",
    "SYSTEM MANUFACTURER",
    "SYSTEM VERSION",
    "TO BE FILLED BY O.E.M.",
    "TO BE FILLED BY OEM",
    "DEFAULT STRING",
    "NOT SPECIFIED",
    "NOT APPLICABLE",
    "INVALID",
    "UNKNOWN",
    "OEM",
    "O.E.M.",
];

impl DmiIdentity {
    /// 有效字段：去掉空值与占位串，统一大写，去重。
    ///
    /// 返回顺序固定为「产品名 → 主板名 → 产品家族」，便于输出与测试稳定。
    pub fn haystacks(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for field in [&self.product_name, &self.board_name, &self.product_family] {
            let Some(raw) = field.as_deref() else {
                continue;
            };
            let up = raw.trim().to_ascii_uppercase();
            if up.is_empty() || DMI_PLACEHOLDERS.contains(&up.as_str()) {
                continue;
            }
            if !out.contains(&up) {
                out.push(up);
            }
        }
        out
    }

    /// 厂商字段的结论：`Some(true)` 明确是 ASUS、`Some(false)` 明确不是、`None` 判断不了。
    ///
    /// 只在两个厂商字段都读到实值、且都不含 ASUS 时才敢说「不是」；
    /// 任一字段缺失（容器、非 Linux）都返回 `None`，交给后续按「未知」放行。
    pub fn is_asus(&self) -> Option<bool> {
        let mut seen = false;
        for field in [&self.sys_vendor, &self.board_vendor] {
            let Some(raw) = field.as_deref() else {
                continue;
            };
            let up = raw.trim().to_ascii_uppercase();
            if up.is_empty() || DMI_PLACEHOLDERS.contains(&up.as_str()) {
                continue;
            }
            seen = true;
            if up.contains("ASUS") {
                return Some(true);
            }
        }
        if seen {
            Some(false)
        } else {
            None
        }
    }

    /// 供日志输出的一行身份摘要（只列有效字段）。
    pub fn label(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(v) = &self.sys_vendor {
            if !v.trim().is_empty() {
                parts.push(format!("vendor={}", v.trim()));
            }
        }
        if let Some(v) = &self.product_name {
            if !v.trim().is_empty() {
                parts.push(format!("product={}", v.trim()));
            }
        }
        if let Some(v) = &self.board_name {
            let t = v.trim();
            if !t.is_empty() && self.product_name.as_deref().map(str::trim) != Some(t) {
                parts.push(format!("board={t}"));
            }
        }
        if let Some(v) = &self.product_family {
            if !v.trim().is_empty() {
                parts.push(format!("family={}", v.trim()));
            }
        }
        if parts.is_empty() {
            "unknown".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// 机型校验结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DmiVerdict {
    /// DMI 命中本包机型
    Match,
    /// DMI 没给结论，但触摸板模组命中本包（比 DMI 更贴近真实硬件）
    MatchHardware(String),
    /// DMI 明确是别的（ASUS）机型
    Mismatch(String),
    /// DMI 厂商字段表明这不是 ASUS 机器
    NonAsus(String),
    /// 读不到 DMI（非 Linux、容器、权限受限、BIOS 只给了占位串）
    Unknown,
    /// 本包不做机型校验（`universal`）
    NotEnforced,
}

impl DmiVerdict {
    /// 是否允许继续启动。
    ///
    /// `Unknown` 放行的理由：读不到 DMI 的场合（容器、缺权限、BIOS 占位串）本来就
    /// 无法判断，此时拦下会误伤正常机器；真正运行起来后还有布局闸门兜底。
    pub fn allows_run(&self) -> bool {
        matches!(
            self,
            DmiVerdict::Match
                | DmiVerdict::MatchHardware(_)
                | DmiVerdict::Unknown
                | DmiVerdict::NotEnforced
        )
    }

    /// 供日志/自检输出的短标签。
    pub fn as_str(&self) -> &'static str {
        match self {
            DmiVerdict::Match => "match",
            DmiVerdict::MatchHardware(_) => "match-hw",
            DmiVerdict::Mismatch(_) => "mismatch",
            DmiVerdict::NonAsus(_) => "non-asus",
            DmiVerdict::Unknown => "unknown",
            DmiVerdict::NotEnforced => "not-enforced",
        }
    }
}

/// 纯逻辑：给定机型表与单个 DMI 型号，判定是否对得上（只比对产品名字段）。
///
/// 保留这个入口是为了兼容只想知道「这一串型号属不属于本包」的调用方与用例；
/// 真正跑在机器上的判定请用 [`verdict_for_identity`]（多字段 + 厂商定性）。
pub fn verdict_for(sku: &Sku, dmi: Option<&str>) -> DmiVerdict {
    let identity = DmiIdentity {
        product_name: dmi.map(str::to_string),
        ..Default::default()
    };
    verdict_for_identity(sku, &identity)
}

/// 纯逻辑：机型表 × DMI 身份 → 结论。
///
/// 判定顺序（越靠前越优先，保证不会被后面的宽松规则覆盖）：
///
/// 1. 通用包 → `NotEnforced`；
/// 2. 三个型号字段都取不到有效值（缺失 / 占位串）→ `Unknown`（放行）；
/// 3. 任一机型前缀出现在任一有效字段里 → `Match`；
/// 4. 厂商字段明确不是 ASUS → `NonAsus`（拦下，装错机器的经典形态）；
/// 5. 其余 → `Mismatch`，附上身份摘要方便用户对照。
pub fn verdict_for_identity(sku: &Sku, identity: &DmiIdentity) -> DmiVerdict {
    if sku.models.is_empty() {
        return DmiVerdict::NotEnforced;
    }
    let pools = identity.haystacks();
    if pools.is_empty() {
        return DmiVerdict::Unknown;
    }
    if sku.models.iter().any(|m| matches_model(m, &pools)) {
        return DmiVerdict::Match;
    }
    if identity.is_asus() == Some(false) {
        let vendor = identity
            .sys_vendor
            .as_deref()
            .or(identity.board_vendor.as_deref())
            .unwrap_or("unknown")
            .trim()
            .to_string();
        return DmiVerdict::NonAsus(vendor);
    }
    DmiVerdict::Mismatch(identity.label())
}

/// 硬件探测结果：这台机器上真实存在的**触摸板模组名**。
///
/// DMI 是 BIOS 写的字符串，会写错、会写成占位串、会因为主板换装而对不上；
/// 触摸板模组名（`ASUE1416:00` 这种）来自 i2c-hid / HID 子系统，直接反映硬件本身，
/// 因此作为 DMI 之外的第二判据——DMI 说不上来时靠它放行，DMI 与实物冲突时也以它为准。
#[derive(Debug, Clone, Default)]
pub struct HardwareProbe {
    /// 归一化后的模组名（大写、去掉 `:00` 之类的实例后缀），去重
    pub modules: Vec<String>,
}

impl HardwareProbe {
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// 本机硬件是否命中该机型的模组名单。
    pub fn matches(&self, sku: &Sku) -> bool {
        self.first_hit(sku).is_some()
    }

    /// 命中的第一个模组名（用于日志输出「凭什么判定是它」）。
    pub fn first_hit(&self, sku: &Sku) -> Option<&str> {
        let hit = self.modules.iter().find(|m| {
            sku.modules.iter().any(|want| {
                let want = want.to_ascii_uppercase();
                // 完全相等，或本机串是「模组名 + 尾巴」（`ASUE1416` vs `ASUE1416-PIXA`）
                m.as_str() == want.as_str() || m.starts_with(&want)
            })
        })?;
        Some(hit.as_str())
    }

    /// 本机挂着的触摸板模组里，第一个被 `HID_LAYOUTS` 收录的（即「确定的触摸板模组」）。
    pub fn known_module(&self) -> Option<&str> {
        self.modules
            .iter()
            .find(|m| {
                crate::layout::HID_LAYOUTS
                    .iter()
                    .any(|(k, _)| k.eq_ignore_ascii_case(m.as_str()))
            })
            .map(String::as_str)
    }

    /// 供日志输出的一行硬件摘要。
    pub fn label(&self) -> String {
        if self.modules.is_empty() {
            "unknown".to_string()
        } else {
            self.modules.join(", ")
        }
    }
}

/// 已知触摸板模组的厂前缀。
///
/// 归一化时只认这些前缀，避免把 i2c/HID 总线上顺手读到的 `PNP0C50`（ACPI ID）、
/// `HID-SENSOR` 之类当成模组名——宁可少数认不出，也不让无关串把机型判定带偏。
const MODULE_PREFIXES: &[&str] = &[
    "ASUE", "ASUF", "ASUP", "ASCP", "ASCF", "ELAN", "ATML", "SYNA", "FTS", "GTCH",
];

/// 归一化模组名：大写、去空白、砍掉 `:00` / `.0001` 这类实例后缀。
fn normalize_module(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let cut = trimmed
        .find(|c: char| c == ':' || c == '.' || c.is_whitespace())
        .unwrap_or(trimmed.len());
    let head = &trimmed[..cut];
    if head.len() < 4 {
        return None;
    }
    // 只留「字母开头 + 含数字」的标识（ASUE1416 / ELAN1403 / ASCP1400），
    // 滤掉 i2c 总线上那些 `SynPS/2`、`PNP0C50` 之类的无关名字
    let bytes = head.as_bytes();
    if !bytes[0].is_ascii_alphabetic() {
        return None;
    }
    if !head.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if !head.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let up = head.to_ascii_uppercase();
    if !MODULE_PREFIXES.iter().any(|p| up.starts_with(p)) {
        return None;
    }
    Some(up)
}

/// 从给定 sysfs 根与 input 设备表读硬件标识（便于测试）。
///
/// 三个来源互补：i2c-hid 触摸板在 `bus/i2c/devices/*/name`，HID 层在
/// `bus/hid/devices/*/HID_NAME`，老内核/部分模组只在 `/proc/bus/input/devices` 里露面。
pub fn probe_hardware_at(sys_root: &Path, input_devices: &Path) -> HardwareProbe {
    let mut modules: Vec<String> = Vec::new();
    let mut push = |raw: &str| {
        if let Some(m) = normalize_module(raw) {
            if !modules.contains(&m) {
                modules.push(m);
            }
        }
    };

    for bus in ["bus/i2c/devices", "bus/hid/devices"] {
        let dir = sys_root.join(bus);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            for name in ["name", "HID_NAME"] {
                if let Ok(text) = std::fs::read_to_string(entry.path().join(name)) {
                    push(&text);
                }
            }
        }
    }

    if let Ok(text) = std::fs::read_to_string(input_devices) {
        for line in text.lines() {
            // `N: Name="ASUE1416:00 04F3:3275 Touchpad"` 与 `I: ...` 两类行都扫一遍
            for field in ["Name=\"", "Name: "] {
                if let Some(pos) = line.find(field) {
                    let rest = &line[pos + field.len()..];
                    let end = rest.find('"').unwrap_or(rest.len());
                    for token in rest[..end].split_whitespace() {
                        push(token);
                    }
                }
            }
        }
    }

    HardwareProbe { modules }
}

/// 读本机硬件标识（非 Linux 环境返回空，判定退回 DMI 路径）。
pub fn probe_hardware() -> HardwareProbe {
    probe_hardware_at(Path::new("/sys"), Path::new("/proc/bus/input/devices"))
}

/// 纯逻辑：机型表 × DMI 身份 × 硬件探测 → 结论。
///
/// 与 [`verdict_for_identity`] 相比多了硬件层，判定顺序：
///
/// 1. 通用包 → `NotEnforced`；
/// 2. DMI 命中机型 → `Match`；
/// 3. 触摸板模组命中机型 → `MatchHardware`（DMI 写错/占位时的救命路径）；
/// 4. DMI 三个型号字段全无效 → `Unknown`（放行）；
/// 5. 厂商字段明确不是 ASUS → `NonAsus`；
/// 6. 其余 → `Mismatch`。
///
/// 硬件判据放在 DMI「无效」之前、厂商判定之前：模组名是实物证据，
/// 比 BIOS 字符串可信；但排在 DMI 命中之后，避免把同族不同代的机器错认成一台。
pub fn verdict_full(sku: &Sku, identity: &DmiIdentity, hw: &HardwareProbe) -> DmiVerdict {
    if sku.models.is_empty() {
        return DmiVerdict::NotEnforced;
    }
    let pools = identity.haystacks();
    if !pools.is_empty() && sku.models.iter().any(|m| matches_model(m, &pools)) {
        return DmiVerdict::Match;
    }
    // DMI 里读出了明确的型号代号（`G615JY` 这种）却没对上本包：这是「明确不是这台」，
    // 不允许一块同族触摸板把它翻案——换了主板/刷了 BIOS 的机器也会出现这种组合。
    let dmi_has_code = pools.iter().any(|p| !model_tokens(p).is_empty());
    if !dmi_has_code {
        if let Some(module) = hw.first_hit(sku) {
            return DmiVerdict::MatchHardware(module.to_string());
        }
        // DMI 说不出型号，但硬件上挂着一块**别的**已知触摸板模组 → 确实是别的机型
        if let Some(other) = hw.known_module() {
            return DmiVerdict::Mismatch(format!("module {}", other));
        }
    }
    if pools.is_empty() {
        return DmiVerdict::Unknown;
    }
    if identity.is_asus() == Some(false) {
        let vendor = identity
            .sys_vendor
            .as_deref()
            .or(identity.board_vendor.as_deref())
            .unwrap_or("unknown")
            .trim()
            .to_string();
        return DmiVerdict::NonAsus(vendor);
    }
    DmiVerdict::Mismatch(identity.label())
}

/// 给「装错包」的用户找最可能的正确包：按机型前缀的前 3 位做粗匹配。
///
/// 例如本机是 `G615JY`，装的是 `g614` 包 → `G61` 前缀同时命中 `g614`/`g615`，
/// 这里会把 `g615` 找出来建议安装。命中多个时返回第一个（机型表按代次排序）。
pub fn suggest_for(identity: &DmiIdentity) -> Option<&'static Sku> {
    suggest_all_for(identity).into_iter().next()
}

/// 同 [`suggest_for`]，但把**全部**候选按匹配强度从高到低返回。
///
/// 同一代机型经常只差一位（`G614` / `G615` / `G635`），只给一个候选容易指错方向；
/// 自检输出里把候选全列出来，用户按自己机器的完整型号挑就行。
///
/// 强度排序：`model` 代号与 DMI 代号 token **完全相等**的最强（权重 100），
/// 其次是「同族前缀」——取 `model` 与 token 的公共前缀长度作为权重。
pub fn suggest_all_for(identity: &DmiIdentity) -> Vec<&'static Sku> {
    let pools = identity.haystacks();
    if pools.is_empty() {
        return Vec::new();
    }
    let tokens: Vec<String> = pools.iter().flat_map(|p| model_tokens(p)).collect();

    let mut scored: Vec<(&'static Sku, usize)> = Vec::new();
    for sku in SKUS {
        if sku.models.is_empty() {
            continue;
        }
        let mut best = 0usize;
        for model in sku.models {
            let up = model.to_ascii_uppercase();
            if tokens.iter().any(|t| t == &up) {
                best = best.max(100);
            }
            for pool in &pools {
                let shared = shared_prefix_in(&up, pool);
                if shared >= 3 {
                    best = best.max(shared);
                }
            }
        }
        if best > 0 {
            scored.push((sku, best));
        }
    }
    // 权重降序；同权重保持 SKUS 表顺序（代次先后），结果稳定可测
    scored.sort_by_key(|&(_, weight)| std::cmp::Reverse(weight));
    let mut out: Vec<&'static Sku> = Vec::new();
    for (sku, _) in scored {
        if !out.iter().any(|s| s.id == sku.id) {
            out.push(sku);
        }
    }
    out
}

/// 从任意 DMI 字符串里抽出「型号代号」token（`G614` / `GX650` / `H7606` / `G614JY`）。
///
/// 判定规则：按非字母数字切分，段长 ≥ 4、以 ASCII 字母开头、且至少含 2 位数字。
///
/// 这样串里的营销词（`ROG` / `STRIX` / `SCAR`）和纯数字（`16`）会被自然滤掉，
/// 剩下的才是可用于比对的代号；像 `G614JY_G614JY` 这种重复写法会被切成两个相同的
/// `G614JY`，不影响判定。
pub fn model_tokens(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for seg in text.split(|c: char| !c.is_ascii_alphanumeric()) {
        if seg.len() < 4 {
            continue;
        }
        let bytes = seg.as_bytes();
        if !bytes[0].is_ascii_alphabetic() {
            continue;
        }
        if seg.chars().filter(|c| c.is_ascii_digit()).count() < 2 {
            continue;
        }
        let up = seg.to_ascii_uppercase();
        if !out.contains(&up) {
            out.push(up);
        }
    }
    out
}

/// 机型表里的 `model` 是否能对上这批 DMI 字段。
///
/// 优先按**型号代号 token** 比对，消除「子串匹配吃进邻近型号」的老问题：
///
/// * `model == token` → 命中；
/// * `token` 以 `model` 开头、且 `model` 之后的第一位**不是数字** → 命中
///   （`G614JY` / `G614JVR` 归 `G614`；`G6140` 不会被 `G614` 吃）；
/// * 某个字段整串抽不出 token（纯营销名，如 `ROG STRIX SCAR 16`）→ 退回旧的
///   整体子串匹配，宁可宽松也不误判这种读不出代号的机器。
fn matches_model(model: &str, pools: &[String]) -> bool {
    let up = model.to_ascii_uppercase();
    if !up.is_ascii() || up.is_empty() {
        return pools.iter().any(|p| p.to_ascii_uppercase().contains(&up));
    }
    for pool in pools {
        let tokens = model_tokens(pool);
        if tokens.is_empty() {
            if pool.to_ascii_uppercase().contains(&up) {
                return true;
            }
            continue;
        }
        for tok in &tokens {
            if tok == &up {
                return true;
            }
            if tok.len() > up.len() && tok.starts_with(&up) {
                // up 全是 ASCII，按字节取界安全
                if !tok.as_bytes()[up.len()].is_ascii_digit() {
                    return true;
                }
            }
        }
    }
    false
}

/// `model` 有多大一段前缀出现在 `pool` 里（从长到短试，命中即返回长度）。
///
/// DMI 产品名里型号编号前面还挂着营销名（`ROG STRIX G16 G615LR_G615LR`），
/// 所以不能按整串做前缀比较，只能问「model 的前 n 位在 pool 里出现过吗」。
fn shared_prefix_in(model: &str, pool: &str) -> usize {
    let m = model.to_ascii_uppercase();
    let p = pool.to_ascii_uppercase();
    let mut n = m.len();
    while n >= 3 {
        // m 只含 ASCII（机型编号），按字节切片安全
        if p.contains(&m[..n]) {
            return n;
        }
        n -= 1;
    }
    0
}

/// 读单个 DMI 字段，空串按「读不到」处理。
fn read_dmi_field(root: &Path, name: &str) -> Option<String> {
    let text = std::fs::read_to_string(root.join(name)).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn identity_at(root: &Path) -> DmiIdentity {
    let id_dir = root.join("class/dmi/id");
    DmiIdentity {
        sys_vendor: read_dmi_field(&id_dir, "sys_vendor"),
        board_vendor: read_dmi_field(&id_dir, "board_vendor"),
        product_name: read_dmi_field(&id_dir, "product_name"),
        board_name: read_dmi_field(&id_dir, "board_name"),
        product_family: read_dmi_field(&id_dir, "product_family"),
    }
}

/// 读本机 DMI 身份（Linux `/sys/class/dmi/id/`，其它平台全部字段为 `None`）。
///
/// 任一字段读不到都不影响其余字段——权限受限的容器里常见只有 `product_name` 可读。
pub fn dmi_identity() -> DmiIdentity {
    identity_at(Path::new("/sys"))
}

/// 从指定 sysfs 根目录读 DMI 身份（便于测试与容器内排查）。
pub fn dmi_identity_at(root: &Path) -> DmiIdentity {
    identity_at(root)
}

/// 读本机 DMI 型号（Linux `/sys/class/dmi/id/product_name`，其它平台返回 `None`）。
pub fn dmi_product_name() -> Option<String> {
    dmi_identity().product_name
}

/// 从指定 sysfs 根目录读 DMI 型号（便于测试与容器内排查）。
pub fn dmi_product_name_at(root: &Path) -> Option<String> {
    dmi_identity_at(root).product_name
}

/// 当前二进制的机型判定。
pub fn current_verdict() -> DmiVerdict {
    verdict_full(build_sku(), &dmi_identity(), &probe_hardware())
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
            assert!(
                by_id(merged).is_none(),
                "{merged} 已并入同族包，不应再单独存在"
            );
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
        assert_eq!(
            verdict_for(sku, Some("Anything G999ZZ")),
            DmiVerdict::NotEnforced
        );
        assert!(verdict_for(sku, Some("Anything G999ZZ")).allows_run());
    }

    #[test]
    fn dmi_file_read_and_mismatch_path() {
        let root = std::env::temp_dir().join("asus-numpad-dmi-selftest");
        let dir = root.join("class/dmi/id");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("product_name"),
            "ROG Strix SCAR 16 G634JY_G634JY\n",
        )
        .unwrap();

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
        for dmi in [
            "ASUS Zenbook Duo UX582LR_UX582LR",
            "ASUS Zenbook Duo UX482EG_UX482EG",
        ] {
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
        assert!(!verdict_for(
            by_id("g614").unwrap(),
            Some("ROG Zephyrus Duo 16 GX650PZ_GX650PZ")
        )
        .allows_run());
    }

    fn ident(product: Option<&str>, board: Option<&str>, family: Option<&str>) -> DmiIdentity {
        DmiIdentity {
            sys_vendor: Some("ASUSTeK COMPUTER INC.".to_string()),
            board_vendor: Some("ASUSTeK COMPUTER INC.".to_string()),
            product_name: product.map(str::to_string),
            board_name: board.map(str::to_string),
            product_family: family.map(str::to_string),
        }
    }

    #[test]
    fn placeholders_count_as_unreadable() {
        // BIOS 没填真值：占位串不能被当成「别的机型」拦下
        let id = ident(
            Some("System Product Name"),
            Some("To Be Filled By O.E.M."),
            Some("Default string"),
        );
        assert!(id.haystacks().is_empty());
        assert_eq!(
            verdict_for_identity(by_id("g614").unwrap(), &id),
            DmiVerdict::Unknown
        );
        assert!(DmiIdentity::default().haystacks().is_empty());
    }

    #[test]
    fn model_id_found_in_board_or_family_field() {
        let g614 = by_id("g614").unwrap();
        // 产品名只有营销名，编号在主板名里
        let id = ident(Some("ROG Strix SCAR 16"), Some("G614JY"), None);
        assert_eq!(
            verdict_for_identity(g614, &id),
            DmiVerdict::Match,
            "{:?}",
            id.label()
        );
        // 编号只在产品家族里
        let id = ident(Some("ROG Strix SCAR 16"), None, Some("G614JZ"));
        assert_eq!(
            verdict_for_identity(g614, &id),
            DmiVerdict::Match,
            "{:?}",
            id.label()
        );
        // 只有营销名、没有编号 → 认不出，按「别的机型」拦下（有值但认不出就不该放行）
        let id = ident(Some("ROG Strix SCAR 16"), None, None);
        assert!(matches!(
            verdict_for_identity(g614, &id),
            DmiVerdict::Mismatch(_)
        ));
    }

    #[test]
    fn non_asus_vendor_is_reported_as_non_asus() {
        let id = DmiIdentity {
            sys_vendor: Some("LENOVO".to_string()),
            board_vendor: Some("LENOVO".to_string()),
            product_name: Some("21KVCTO1WW".to_string()),
            ..Default::default()
        };
        match verdict_for_identity(by_id("g614").unwrap(), &id) {
            DmiVerdict::NonAsus(vendor) => assert!(vendor.contains("LENOVO"), "{vendor}"),
            other => panic!("expected NonAsus, got {other:?}"),
        }
        assert!(!verdict_for_identity(by_id("g614").unwrap(), &id).allows_run());
        // 厂商字段读不全时不硬判「不是 ASUS」
        let id = DmiIdentity {
            sys_vendor: None,
            board_vendor: None,
            product_name: Some("21KVCTO1WW".to_string()),
            ..Default::default()
        };
        assert!(matches!(
            verdict_for_identity(by_id("g614").unwrap(), &id),
            DmiVerdict::Mismatch(_)
        ));
    }

    #[test]
    fn suggest_picks_the_neighbouring_generation() {
        // 本机是 G615，装的是 g614 包 → 建议 g615
        let id = ident(Some("ROG Strix G16 G615LR_G615LR"), None, None);
        let hit = suggest_for(&id).expect("should suggest a package");
        assert_eq!(hit.id, "g615", "suggested {}", hit.id);
        // 读不到 DMI 时没有建议
        assert!(suggest_for(&DmiIdentity::default()).is_none());
    }

    #[test]
    fn normalize_module_strips_instance_suffix() {
        assert_eq!(
            normalize_module("ASUE1416:00\n").as_deref(),
            Some("ASUE1416")
        );
        assert_eq!(
            normalize_module(" ASUE1416:00 04F3:3275 Touchpad ").as_deref(),
            Some("ASUE1416")
        );
        assert_eq!(
            normalize_module("ROG STRIX SCAR 16"),
            None,
            "营销名不是模组"
        );
        assert_eq!(normalize_module("PNP0C50"), None, "ACPI 码不是模组");
        assert_eq!(normalize_module(""), None);
    }

    #[test]
    fn hardware_rescues_placeholder_dmi() {
        // DMI 全是占位串（白牌 / 刷过 BIOS 的机器），过去只能放行到 Unknown；
        // 现在触摸板模组能直接把机型认出来。
        let identity = DmiIdentity {
            sys_vendor: Some("System manufacturer".to_string()),
            product_name: Some("System Product Name".to_string()),
            ..Default::default()
        };
        let hw = HardwareProbe {
            modules: vec!["ASUE1416".to_string()],
        };
        assert!(identity.haystacks().is_empty(), "占位串不算有效型号");
        assert_eq!(
            verdict_full(by_id("g614").unwrap(), &identity, &hw),
            DmiVerdict::MatchHardware("ASUE1416".to_string())
        );
        // 换了机型包就不该被同一块触摸板救活
        assert!(!verdict_full(by_id("g615").unwrap(), &identity, &hw).allows_run());
    }

    #[test]
    fn hardware_does_not_override_explicit_dmi_mismatch() {
        // DMI 明确是 G615，硬件也没挂错：装 g614 包仍然拒绝
        let identity = DmiIdentity {
            sys_vendor: Some("ASUSTeK COMPUTER INC.".to_string()),
            product_name: Some("ROG Strix SCAR 16 G615JY_G615JY".to_string()),
            ..Default::default()
        };
        let hw = HardwareProbe {
            modules: vec!["ASUE1416".to_string()],
        };
        let v = verdict_full(by_id("g614").unwrap(), &identity, &hw);
        assert!(!v.allows_run(), "{v:?}");
        let v = verdict_full(by_id("g615").unwrap(), &identity, &hw);
        assert_eq!(v, DmiVerdict::Match);
    }

    #[test]
    fn hardware_probe_reads_i2c_and_input_tables() {
        let root = std::env::temp_dir().join("asus-numpad-hw-selftest");
        let i2c = root.join("bus/i2c/devices/i2c-ASUE1416:00");
        std::fs::create_dir_all(&i2c).unwrap();
        std::fs::write(i2c.join("name"), "ASUE1416:00\n").unwrap();
        let hid = root.join("bus/hid/devices/0003:04F3:3275.0001");
        std::fs::create_dir_all(&hid).unwrap();
        std::fs::write(hid.join("HID_NAME"), "ASUE1416:00 04F3:3275 Touchpad\n").unwrap();
        let input = root.join("proc-input-devices");
        std::fs::write(
            &input,
            "I: Bus=0018\nN: Name=\"ASUE1416:00 04F3:3275 Touchpad\"\n",
        )
        .unwrap();

        let hw = probe_hardware_at(&root, &input);
        assert!(hw.matches(by_id("g614").unwrap()), "{:?}", hw);
        assert!(!hw.matches(by_id("g615").unwrap()));
        assert_eq!(hw.label(), "ASUE1416");

        // 目录不存在时不 panic，返回空探测
        let empty = probe_hardware_at(&root.join("nope"), &root.join("nope-input"));
        assert!(empty.is_empty());
        assert_eq!(empty.label(), "unknown");
    }

    #[test]
    fn unknown_build_sku_falls_back_to_universal() {
        let sku = build_sku();
        assert!(by_id(sku.id).is_some());
    }

    #[test]
    fn model_tokens_drop_marketing_words() {
        assert_eq!(
            model_tokens("ROG Strix SCAR 16 G614JY_G614JY"),
            vec!["G614JY".to_string()]
        );
        assert_eq!(model_tokens("ROG STRIX SCAR 16"), Vec::<String>::new());
        assert_eq!(
            model_tokens("ASUS ProArt P16 H7606WI"),
            vec!["H7606WI".to_string()]
        );
    }

    #[test]
    fn token_prefix_does_not_swallow_longer_numbers() {
        // G614 包不能把 G6140 之类更长的编号当成自家机型
        let id = ident(Some("Generic Chassis G6140XY"), None, None);
        assert!(matches!(
            verdict_for_identity(by_id("g614").unwrap(), &id),
            DmiVerdict::Mismatch(_)
        ));
        // 但同代后缀（G614JY / G614JVR）要能认
        for name in ["G614JY", "G614JVR", "G614JV_G614JV"] {
            let id = ident(Some(&format!("ROG Strix SCAR 16 {name}")), None, None);
            assert_eq!(
                verdict_for_identity(by_id("g614").unwrap(), &id),
                DmiVerdict::Match,
                "{name} 应归 g614"
            );
        }
    }

    #[test]
    fn marketing_only_pool_falls_back_to_substring() {
        // 抽不出型号代号的字段沿用整体子串匹配，不能因为令牌化就判死
        let id = ident(Some("ASUS G615"), None, None);
        assert_eq!(
            verdict_for_identity(by_id("g615").unwrap(), &id),
            DmiVerdict::Match
        );
    }

    #[test]
    fn board_name_rescues_marketing_product_name() {
        // 产品名只有营销名时，主板名 / 家族里的编号要能兜住
        let id = ident(Some("ROG Strix SCAR 16"), Some("G614JY"), None);
        assert_eq!(
            verdict_for_identity(by_id("g614").unwrap(), &id),
            DmiVerdict::Match
        );
    }

    #[test]
    fn suggest_all_lists_strongest_first() {
        let id = ident(Some("ROG Strix G16 G615LR_G615LR"), None, None);
        let all = suggest_all_for(&id);
        assert!(!all.is_empty(), "应有候选");
        assert_eq!(all[0].id, "g615", "最强的候选应排第一");
        // 候选去重
        let mut ids: Vec<&str> = all.iter().map(|s| s.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n);
        // 读不到 DMI 时无候选
        assert!(suggest_all_for(&DmiIdentity::default()).is_empty());
    }
}
