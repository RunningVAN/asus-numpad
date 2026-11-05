//! 机型配置：一个包，运行期自动选择本机对应的机型参数。
//!
//! ## 为什么不再按机型分包
//!
//! 早期做法是按机型出多个包（`asus-numpad-g614` 之类），每个包在
//! 编译期把机型写死，装错包就退出码 3 拒绝启动。这带来三个问题：
//!
//! * 用户必须先查清自己是哪台机器才能装——查 DMI、对表、挑包，全是多余步骤；
//! * 同一台机器换主板 / 刷 BIOS / 装到虚拟机后包名立刻变成「错的」；
//! * 机型表每加一行就要多发一轮包，九个包维护成本远高于收益。
//!
//! 现在只有一个包，机型表 [`SKUS`] 编译进二进制，启动时按**本机实际硬件**选配置：
//!
//! 1. 配置文件 / 命令行显式指定（`"sku": "g614"` 或 `--sku g614`）——最高优先，
//!    给「DMI 读不出型号」的机器留的备用通道（[`SkuSource::Manual`]）；
//! 2. DMI 型号字段命中（[`SkuSource::Dmi`]）；
//! 3. 触摸板模组名命中（[`SkuSource::Hardware`]）——DMI 写成占位串时的第二判据；
//! 4. 都没命中 → 通用配置（[`SkuSource::Fallback`]），照常运行，键表由
//!    [`crate::layout`] 按实际 HID 模组决定。
//!
//! ## 仍然会拦的两种情况
//!
//! 自动识别之后「装错包」不再是问题，但有两类机器**装了也没用**，必须直说：
//! 厂商明确不是 ASUS（[`DmiVerdict::NonAsus`]），以及 DMI 命中「结构上没有
//! NumberPad」的机型（[`NON_NUMPAD_MODELS`]，如 ProArt 的 DialPad 旋钮、
//! 18 英寸的物理数字键）→ [`DmiVerdict::NotNumberPad`]。
//!
//! ## 不做的事
//!
//! 这里不做布局/键表判定，那仍然由 [`crate::layout`] 负责。本模块只回答一个问题：
//! **这台机器该用哪套机型参数。**

use std::path::Path;

/// 一个机型配置条目。
#[derive(Debug)]
pub struct Sku {
    /// 配置 id（也是 `--sku` / 配置文件的取值），如 `g614`
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

/// 机型配置表。新增机型只加一行——这张表是唯一源，出包脚本不再复制一份。
///
/// ## 只收跑过的机型
///
/// 表里只有 `g614`（实测机型 `G634JY` 所在的一族）与兜底的 `universal`。
/// 其他机型（`G533` / `G513`、`G615` / `G635`、`GX550` / `GX551` / `GX650`、
/// `UX582` / `UX482`）不再单独建条目：键表本来就不跟整机型号走，而是由
/// [`crate::layout::HID_LAYOUTS`] 按实际触摸板模组选择（模组名相同即同一份键表、
/// 同一套几何），这些机器落 [`SkuSource::Fallback`] 照样能跑。写进表里只是没有
/// 实测支撑的声明，真出问题也没人能复现。
///
/// 所以本表只回答一件事：是不是实测机型、要不要走实测参数。「认不出」不是错误，
/// 通用配置就是默认路径。
///
/// 历史机型包名（`asus-numpad-g614` 等）由统一包 Obsoletes 掉，老用户
/// `dnf upgrade` 会自动接管，不需要先手工卸载旧机型包——那只关乎包名迁移，与机型表无关。
pub const SKUS: &[Sku] = &[
    Sku {
        id: "g614",
        display: "ASUS 枪神7 / 枪神8 / 魔霸新锐 2023 / 枪神7 超竞版 16 英寸（G614 / G634）",
        models: &["G614", "G634"],
        modules: &["ASUE1416", "ASUF1416"],
        note: "唯一上过真机的配置（G634JY）：同一块 ELAN 触摸板（ASUE1416 / ASUF1416 → GH3XLNDZ）；低配 EACX 无此硬件，靠设备缺失自然不动键盘",
    },
    Sku {
        id: "universal",
        display: "通用配置（默认路径：非实测机型都走这里）",
        models: &[],
        modules: &[],
        note: "参数取官方默认，键表按实际触摸板模组自动选择；也可用 asus-numpad sku <id> 手动指定",
    },
];

/// 明确**不是** NumberPad 触摸板的机型：识别到就直说「这台不需要本驱动」。
///
/// 只靠 [`SKUS`] 的白名单做不到这件事——不在白名单里只会得到一句「机型不匹配，
/// 建议改装 asus-numpad-xxx」，而这几类机器**装哪个包都没用**，必须把原因讲清楚：
///
/// * ProArt 系（`H7606` = ProArt P16）：触控板是 **ASUS DialPad 智能旋钮**，官方规格
///   写的是 `support DialPad`，走旋钮的 HID 用法，与 NumberPad 的 `FEATURE 0x0d`
///   数字区背光完全无关；
/// * 18 英寸 Plus（`G834` / `G835` / `G814` / `G815` / `G813`）：机身自带物理数字键，
///   触控板里没有虚拟数字键盘。
///
/// 判定与 [`SKUS`] 用同一套多字段 token 匹配，所以 `product_name` 写营销名、型号只落在
/// `board_name` 的机器同样能认出来。
pub const NON_NUMPAD_MODELS: &[(&str, &str)] = &[
    (
        "H7606",
        "ProArt P16 的触控板是 ASUS DialPad 智能旋钮（官方规格 support DialPad），不是 NumberPad 虚拟数字键盘",
    ),
    (
        "G713",
        "17 英寸 Strix G17 机身自带物理数字键，触控板没有虚拟数字键盘（官方规格 NumberPad 一栏为 N/A）",
    ),
    (
        "G834",
        "18 英寸 Plus 机身自带物理数字键，触控板没有虚拟数字键盘",
    ),
    (
        "G835",
        "18 英寸 Plus 机身自带物理数字键，触控板没有虚拟数字键盘",
    ),
    (
        "G814",
        "18 英寸 Plus 机身自带物理数字键，触控板没有虚拟数字键盘",
    ),
    (
        "G815",
        "18 英寸 Plus 机身自带物理数字键，触控板没有虚拟数字键盘",
    ),
    (
        "G813",
        "18 英寸 Plus 机身自带物理数字键，触控板没有虚拟数字键盘",
    ),
];

/// 该 DMI 身份是否命中「明确没有 NumberPad」的机型；命中返回 `(型号, 原因)`。
pub fn non_numpad_hit(identity: &DmiIdentity) -> Option<(&'static str, &'static str)> {
    let pools = identity.haystacks();
    if pools.is_empty() {
        return None;
    }
    NON_NUMPAD_MODELS
        .iter()
        .find(|(model, _)| matches_model(model, &pools))
        .map(|(model, why)| (*model, *why))
}

/// 机型配置是怎么选出来的——输出里要说清「凭什么判定是这台」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkuSource {
    /// 配置文件 `sku` 字段 / 命令行 `--sku` 显式指定
    Manual,
    /// DMI 型号字段命中
    Dmi,
    /// 触摸板模组名命中（DMI 读不出型号时的第二判据）
    Hardware,
    /// 都没命中，用通用配置
    Fallback,
}

impl SkuSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkuSource::Manual => "manual",
            SkuSource::Dmi => "dmi",
            SkuSource::Hardware => "hardware",
            SkuSource::Fallback => "fallback",
        }
    }

    /// 对应 `sku.source.*` 这组 i18n 键。
    pub fn i18n_key(&self) -> &'static str {
        match self {
            SkuSource::Manual => "sku.source.manual",
            SkuSource::Dmi => "sku.source.dmi",
            SkuSource::Hardware => "sku.source.hardware",
            SkuSource::Fallback => "sku.source.fallback",
        }
    }
}

/// 自动识别的结果：用哪套配置 + 凭什么。
#[derive(Debug, Clone)]
pub struct Resolved {
    pub sku: &'static Sku,
    pub source: SkuSource,
    /// 选中依据的细节：命中的机型代号 / 触摸板模组 / 手动指定的 id
    pub detail: Option<String>,
    /// 这台机器该不该跑（`NotNumberPad` / `NonAsus` 会被启动闸门拦下）
    pub verdict: DmiVerdict,
}

impl Resolved {
    pub fn id(&self) -> &'static str {
        self.sku.id
    }

    pub fn allows_run(&self) -> bool {
        self.verdict.allows_run()
    }

    /// 是否真的认出了机型（`false` = 落到通用配置）
    pub fn is_detected(&self) -> bool {
        self.source != SkuSource::Fallback
    }
}

/// 手动指定的机型 id 不在表里。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSku(pub String);

impl std::fmt::Display for UnknownSku {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown model id: {}", self.0)
    }
}

impl std::error::Error for UnknownSku {}

/// 机型表里所有可选 id（含兜底的 `universal`）。
pub fn known_ids() -> Vec<&'static str> {
    SKUS.iter().map(|s| s.id).collect()
}

fn universal() -> &'static Sku {
    by_id("universal").expect("SKUS 必须含 universal")
}

/// 纯逻辑：DMI 身份 × 硬件探测 × 手动指定 → 该用哪套机型配置。
///
/// 判定顺序（越靠前越优先）：
///
/// 1. 手动指定（配置文件 `sku` / `--sku`）→ `Manual`，认了这个 id 就不再质疑；
/// 2. DMI 型号字段命中某条机的 `models` → `Dmi`；
/// 3. 触摸板模组命中某条机的 `modules` → `Hardware`；
/// 4. DMI 命中「结构上没有 NumberPad」的机型 → 通用配置 + `NotNumberPad`；
/// 5. 厂商字段明确不是 ASUS → 通用配置 + `NonAsus`；
/// 6. 其余（读不到 DMI、型号不在表里）→ 通用配置 + `Unknown`，照常运行。
///
/// 第 6 步刻意放行：读不到 DMI 不等于装错机器，真正跑起来后还有布局闸门按实际
/// HID 模组兜底。手动指定是给这类机器准备的备用通道。
pub fn resolve(
    identity: &DmiIdentity,
    hw: &HardwareProbe,
    manual: Option<&str>,
) -> Result<Resolved, UnknownSku> {
    if let Some(id) = manual {
        if !id.trim().is_empty() {
            let sku = by_id(id).ok_or_else(|| UnknownSku(id.trim().to_string()))?;
            // 手动指定等于「我确认这台就是它」，不再让 DMI 反过来否掉用户的选择
            let verdict = if sku.models.is_empty() {
                DmiVerdict::NotEnforced
            } else {
                DmiVerdict::Match
            };
            return Ok(Resolved {
                sku,
                source: SkuSource::Manual,
                detail: Some(sku.id.to_string()),
                verdict,
            });
        }
    }

    if let Some((sku, model)) = dmi_hit(identity) {
        return Ok(Resolved {
            sku,
            source: SkuSource::Dmi,
            detail: Some(model.to_string()),
            verdict: DmiVerdict::Match,
        });
    }

    for sku in SKUS.iter().filter(|s| !s.modules.is_empty()) {
        if let Some(module) = hw.first_hit(sku) {
            return Ok(Resolved {
                sku,
                source: SkuSource::Hardware,
                detail: Some(module.to_string()),
                verdict: DmiVerdict::MatchHardware(module.to_string()),
            });
        }
    }

    if let Some((model, reason)) = non_numpad_hit(identity) {
        return Ok(Resolved {
            sku: universal(),
            source: SkuSource::Fallback,
            detail: Some(model.to_string()),
            verdict: DmiVerdict::NotNumberPad {
                model: model.to_string(),
                reason,
            },
        });
    }

    if identity.is_asus() == Some(false) {
        let vendor = identity
            .sys_vendor
            .as_deref()
            .or(identity.board_vendor.as_deref())
            .unwrap_or("unknown")
            .trim()
            .to_string();
        return Ok(Resolved {
            sku: universal(),
            source: SkuSource::Fallback,
            detail: None,
            verdict: DmiVerdict::NonAsus(vendor),
        });
    }

    Ok(Resolved {
        sku: universal(),
        source: SkuSource::Fallback,
        detail: None,
        verdict: DmiVerdict::Unknown,
    })
}

/// 本机该用哪套机型配置（读真实 DMI 与触摸板）；`manual` 来自配置 / 命令行。
pub fn current(manual: Option<&str>) -> Result<Resolved, UnknownSku> {
    resolve(&dmi_identity(), &probe_hardware(), manual)
}

/// 纯逻辑：DMI 型号 → 命中的配置条目与命中的那个代号。
///
/// 多条同时命中时取 `model` 最长的那个：`G614` 与 `G614JY` 都在表里时，
/// 更长的代号更精确。
fn dmi_hit(identity: &DmiIdentity) -> Option<(&'static Sku, &'static str)> {
    let pools = identity.haystacks();
    if pools.is_empty() {
        return None;
    }
    let mut best: Option<(&'static Sku, &'static str)> = None;
    for sku in SKUS.iter().filter(|s| !s.models.is_empty()) {
        for model in sku.models {
            if !matches_model(model, &pools) {
                continue;
            }
            let better = match best {
                Some((_, cur)) => model.len() > cur.len(),
                None => true,
            };
            if better {
                best = Some((sku, model));
            }
        }
    }
    best
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
    /// DMI 命中「结构上没有 NumberPad」的机型（ProArt 的 DialPad 旋钮、18 英寸物理数字键），
    /// 附型号与原因，用于直接告知「这台不用装」，而不是含糊地报「不匹配」
    NotNumberPad { model: String, reason: &'static str },
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
            DmiVerdict::NotNumberPad { .. } => "not-numpad",
            DmiVerdict::NonAsus(_) => "non-asus",
            DmiVerdict::Unknown => "unknown",
            DmiVerdict::NotEnforced => "not-enforced",
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(product: Option<&str>, board: Option<&str>, family: Option<&str>) -> DmiIdentity {
        DmiIdentity {
            sys_vendor: Some("ASUSTeK COMPUTER INC.".to_string()),
            board_vendor: Some("ASUSTeK COMPUTER INC.".to_string()),
            product_name: product.map(str::to_string),
            board_name: board.map(str::to_string),
            product_family: family.map(str::to_string),
        }
    }

    /// 只有 product_name 有值的身份——绝大多数真机就是这种形态。
    fn dmi(product: &str) -> DmiIdentity {
        ident(Some(product), None, None)
    }

    fn hw(modules: &[&str]) -> HardwareProbe {
        HardwareProbe {
            modules: modules.iter().map(|m| m.to_ascii_uppercase()).collect(),
        }
    }

    fn no_hw() -> HardwareProbe {
        HardwareProbe::default()
    }

    #[test]
    fn sku_ids_are_unique_and_complete() {
        let mut ids: Vec<&str> = SKUS.iter().map(|s| s.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "SKU id 必须唯一");
        assert!(by_id("universal").is_some(), "必须保留 universal 兜底");
        assert!(by_id("G614").is_some(), "机型查询应忽略大小写");
        // 只收跑过的机型：表里除 g614 与兜底的 universal 之外不应再有条目
        let mut ids: Vec<&str> = SKUS.iter().map(|s| s.id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["g614", "universal"]);
        for gone in [
            "g533", "g634", "g635", "gx550", "gx650", "ux582", "g713", "h7606",
        ] {
            assert!(by_id(gone).is_none(), "{gone} 不应单列");
        }
        for s in SKUS {
            assert!(!s.display.is_empty(), "{} 缺少展示名", s.id);
            assert!(!s.note.is_empty(), "{} 缺少说明", s.id);
        }
        assert!(known_ids().contains(&"universal"));
    }

    #[test]
    fn dmi_auto_selects_the_matching_config() {
        let cases = [
            ("ROG Strix SCAR 16 G634JY_G634JY", "g614"),
            ("ROG Strix SCAR 16 G614JZ_G614JZ", "g614"),
        ];
        for (product, want) in cases {
            let r = resolve(&dmi(product), &no_hw(), None).unwrap();
            assert_eq!(r.id(), want, "{product}");
            assert_eq!(r.source, SkuSource::Dmi, "{product}");
            assert!(r.allows_run(), "{product} 应放行");
            assert!(r.is_detected(), "{product} 应算认出机型");
            assert!(r.detail.is_some(), "{product} 应说明命中的代号");
        }
    }

    #[test]
    fn untested_models_fall_back_instead_of_being_listed() {
        // 不在实测机型表里的机器不做机型声明：落通用配置，照常放行。
        // 键表由 layout 按实际触摸板模组选，与机型无关，功能不受影响。
        for product in [
            "ROG Strix SCAR 15 G533ZW_G533ZW",
            "ROG Strix G15 G513RM_G513RM",
            "ROG Strix SCAR 16 G615LR_G615LR",
            "ROG Strix SCAR 16 G635LX_G635LX",
            "ROG Zephyrus Duo 16 GX650PZ_GX650PZ",
            "ROG Zephyrus Duo 15 GX550LXS_GX550LXS",
            "ASUS Zenbook Duo UX582LR_UX582LR",
            "ASUS Zenbook Duo UX482EG_UX482EG",
        ] {
            let r = resolve(&dmi(product), &no_hw(), None).unwrap();
            assert_eq!(r.id(), "universal", "{product}");
            assert_eq!(r.source, SkuSource::Fallback, "{product}");
            assert_eq!(r.verdict, DmiVerdict::Unknown, "{product}");
            assert!(r.allows_run(), "{product} 应放行");
            assert!(!r.is_detected(), "{product} 不应算认出机型");
        }
    }

    #[test]
    fn model_id_found_in_board_or_family_field() {
        // 产品名只有营销名，编号在主板名里
        let r = resolve(
            &ident(Some("ROG Strix SCAR 16"), Some("G614JY"), None),
            &no_hw(),
            None,
        )
        .unwrap();
        assert_eq!(r.id(), "g614");
        assert_eq!(r.source, SkuSource::Dmi);
        // 编号只在产品家族里
        let r = resolve(
            &ident(Some("ROG Strix SCAR 16"), None, Some("G614JZ")),
            &no_hw(),
            None,
        )
        .unwrap();
        assert_eq!(r.id(), "g614");
        // 只有营销名、没有编号：认不出就落通用配置，不拦
        let r = resolve(
            &ident(Some("ROG Strix SCAR 16"), None, None),
            &no_hw(),
            None,
        )
        .unwrap();
        assert_eq!(r.id(), "universal");
        assert_eq!(r.source, SkuSource::Fallback);
        assert!(r.allows_run());
        assert!(!r.is_detected());
    }

    #[test]
    fn hardware_module_is_the_second_judgement() {
        // DMI 里没有机型编号，但触摸板模组对得上 → 按硬件选配置
        let r = resolve(&dmi("ROG Strix SCAR 16"), &hw(&["ASUE1416:00"]), None).unwrap();
        assert_eq!(r.id(), "g614");
        assert_eq!(r.source, SkuSource::Hardware);
        assert!(matches!(&r.verdict, DmiVerdict::MatchHardware(m) if m.starts_with("ASUE1416")));
        assert!(r.allows_run());
        // 模组不认识 → 通用配置
        let r = resolve(
            &dmi("ROG Strix SCAR 16"),
            &hw(&["PNP0C50", "HID-SENSOR-2000"]),
            None,
        )
        .unwrap();
        assert_eq!(r.source, SkuSource::Fallback);
        assert!(r.allows_run());
        // 模组属于其他机型族（不在机型表里）：照样放行，键表交给 layout 按模组选
        let r = resolve(
            &dmi("ROG Strix G15 G513RM_G513RM"),
            &hw(&["ASUE1403:00"]),
            None,
        )
        .unwrap();
        assert_eq!(r.id(), "universal");
        assert_eq!(r.source, SkuSource::Fallback);
        assert!(r.allows_run());
    }

    #[test]
    fn manual_selection_overrides_detection() {
        // 手动指定优先于 DMI：读错硬件信息时这是唯一的逃生通道
        let r = resolve(&no_identity(), &no_hw(), Some("g614")).unwrap();
        assert_eq!(r.id(), "g614");
        assert_eq!(r.source, SkuSource::Manual);
        assert!(r.allows_run());
        assert_eq!(r.detail.as_deref(), Some("g614"));
        // 大小写与空白不敏感
        let r = resolve(&no_identity(), &no_hw(), Some("  G614 ")).unwrap();
        assert_eq!(r.id(), "g614");
        // 已下架的机型 id 现在按未知处理（退出码 2 并提示可用 id），不再静默接受
        assert!(resolve(&no_identity(), &no_hw(), Some("gx550")).is_err());
        // 手动指定 universal = 明确要通用配置
        let r = resolve(&no_identity(), &no_hw(), Some("universal")).unwrap();
        assert_eq!(r.id(), "universal");
        assert_eq!(r.verdict, DmiVerdict::NotEnforced);
        assert!(r.allows_run());
        // 空字符串等同没指定
        let r = resolve(&dmi("ROG Strix SCAR 16 G634JY"), &no_hw(), Some("   ")).unwrap();
        assert_eq!(r.source, SkuSource::Dmi);
    }

    #[test]
    fn manual_unknown_id_is_an_error() {
        let err = resolve(&no_identity(), &no_hw(), Some("g999")).unwrap_err();
        assert_eq!(err, UnknownSku("g999".to_string()));
    }

    fn no_identity() -> DmiIdentity {
        DmiIdentity::default()
    }

    #[test]
    fn proart_p16_is_not_numpad() {
        // ProArt P16（H7606）触控板是 ASUS DialPad 智能旋钮，官方规格 support DialPad，
        // 不是 NumberPad：自动识别不该把它挑成任何机型配置，且要给出准确原因。
        let r = resolve(&dmi("ASUS ProArt P16 H7606WI_H7606WI"), &no_hw(), None).unwrap();
        assert!(
            matches!(&r.verdict, DmiVerdict::NotNumberPad { model, reason }
            if model == "H7606" && reason.contains("DialPad"))
        );
        assert!(!r.allows_run());
        // 型号只落在 board_name 上也能认出来
        let r = resolve(
            &ident(Some("ASUS ProArt P16"), Some("H7606WI"), None),
            &no_hw(),
            None,
        )
        .unwrap();
        assert!(matches!(r.verdict, DmiVerdict::NotNumberPad { .. }));
    }

    #[test]
    fn physical_numpad_models_are_not_numberpad() {
        // 18 英寸 Plus（G834 / G814）与 17 英寸 G17（G713）机身自带物理数字键，
        // 触控板没有虚拟数字键盘，官方规格 NumberPad 一栏为 N/A。
        for product in [
            "ROG Strix SCAR 18 G834JY_G834JY",
            "ROG Strix G18 G814JZ_G814JZ",
            "ROG Strix G17 G713RC_G713RC",
            "ROG Strix G17 G713RM_G713RM",
            "ROG Strix G17 G713QR_G713QR",
        ] {
            let r = resolve(&dmi(product), &no_hw(), None).unwrap();
            assert!(
                matches!(r.verdict, DmiVerdict::NotNumberPad { .. }),
                "{product}: {:?}",
                r.verdict
            );
            assert!(!r.allows_run(), "{product} 不应放行");
        }
    }

    #[test]
    fn real_gaming_models_are_not_caught_by_reject_rules() {
        for product in [
            "ROG Strix SCAR 16 G615LR_G615LR",
            "ROG Strix SCAR 16 G635LX_G635LX",
            "ROG Strix SCAR 16 G634JY_G634JY",
        ] {
            let r = resolve(&dmi(product), &no_hw(), None).unwrap();
            assert!(
                !matches!(r.verdict, DmiVerdict::NotNumberPad { .. }),
                "{product}: {:?}",
                r.verdict
            );
            assert!(r.allows_run(), "{product} 应放行");
        }
        // 实测机型才走具体参数，其余走通用配置
        assert_eq!(
            resolve(&dmi("ROG Strix SCAR 16 G634JY_G634JY"), &no_hw(), None)
                .unwrap()
                .id(),
            "g614"
        );
        assert_eq!(
            resolve(&dmi("ROG Strix SCAR 16 G635LX_G635LX"), &no_hw(), None)
                .unwrap()
                .id(),
            "universal"
        );
    }

    #[test]
    fn non_asus_vendor_is_reported_as_non_asus() {
        let id = DmiIdentity {
            sys_vendor: Some("LENOVO".to_string()),
            board_vendor: Some("LENOVO".to_string()),
            product_name: Some("Legion 7 16IRX8H".to_string()),
            board_name: Some("LNVNB161216".to_string()),
            product_family: None,
        };
        let r = resolve(&id, &no_hw(), None).unwrap();
        assert_eq!(r.id(), "universal");
        assert!(matches!(&r.verdict, DmiVerdict::NonAsus(v) if v.contains("LENOVO")));
        assert!(!r.allows_run());
    }

    #[test]
    fn unreadable_dmi_falls_back_instead_of_blocking() {
        // 读不到 DMI 不等于装错机器：落通用配置照常跑，运行期还有布局闸门兜底
        let r = resolve(&no_identity(), &no_hw(), None).unwrap();
        assert_eq!(r.id(), "universal");
        assert_eq!(r.source, SkuSource::Fallback);
        assert_eq!(r.verdict, DmiVerdict::Unknown);
        assert!(r.allows_run());
        // 型号不在机型表里（新机型）同样不拦
        let r = resolve(&dmi("ROG Strix SCAR 16 G999ZZ"), &no_hw(), None).unwrap();
        assert_eq!(r.id(), "universal");
        assert!(r.allows_run());
    }

    #[test]
    fn placeholders_count_as_unreadable() {
        // BIOS 没填真值：占位串不能被当成「别的机型」
        let id = ident(
            Some("System Product Name"),
            Some("To Be Filled By O.E.M."),
            Some("Default string"),
        );
        assert!(id.haystacks().is_empty());
        let r = resolve(&id, &no_hw(), None).unwrap();
        assert_eq!(r.verdict, DmiVerdict::Unknown);
        assert!(r.allows_run());
        assert!(DmiIdentity::default().haystacks().is_empty());
    }

    #[test]
    fn dmi_file_read_and_auto_selection() {
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
        // 只有 product_name 时也能自动选中对应配置
        let r = resolve(&dmi("ROG Strix SCAR 16 G634JY_G634JY"), &no_hw(), None).unwrap();
        assert_eq!(r.id(), "g614");

        // 空文件视为未知，不拦
        std::fs::write(dir.join("product_name"), "  \n").unwrap();
        assert_eq!(dmi_product_name_at(&root), None);
        let r = resolve(&no_identity(), &no_hw(), None).unwrap();
        assert!(r.allows_run());

        let _ = std::fs::remove_dir_all(&root);
    }
}
