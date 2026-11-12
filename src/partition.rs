//! 自定义分区与手动校准：给官方表里没有的机型准备的两条后路。
//!
//! 官方 15 个布局参数只覆盖「5 列均匀网格 + 4 行」这一种形态，且网格位置由
//! `index0/index4/level0/level3` 与有效区内缩推算。表外机型常见两类偏差：
//!
//! 1. **分区不同**：贴纸行列不是 5x4，列宽不是等分，NumLock 图标不在右上角；
//! 2. **面板参数不同**：`ABS_MT_POSITION_*` 的范围/分辨率报不准，或干脆没报。
//!
//! 对应两个配置段：
//!
//! * `partition`：直接用边界把网格切成任意行列（可非均匀），可整块平移缩放；
//! * `calibration`：覆盖轴的 `min` / `max` / `units_per_mm`。
//!
//! 两者都只描述**几何**，不碰键表——键表仍由 `custom_layouts[].keys` 给。
//!
//! 校准求解器 [`solve_calibration`] 是纯函数：给它「四角落点 + 两个格中心落点」的
//! 实测样本，它把面板范围与网格边界一并算出来。采集在 `linux` 模块，macOS 上也能
//! 用样本文件跑同一套算法做端到端验证。

use serde::{Deserialize, Serialize};

/// 触摸板面板参数（绝对坐标范围 + 两轴分辨率），来自 evdev 或校准。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub xmin: i32,
    pub xmax: i32,
    pub ymin: i32,
    pub ymax: i32,
    /// units per µm
    pub ux: f64,
    pub uy: f64,
}

impl Panel {
    /// 毫米 → 采样单位（相对面板左上角）。
    pub fn mm_to_x(&self, mm: f64) -> f64 {
        self.xmin as f64 + mm * 1000.0 * self.ux
    }

    pub fn mm_to_y(&self, mm: f64) -> f64 {
        self.ymin as f64 + mm * 1000.0 * self.uy
    }
}

// --------------------------------------------------------------------------- //
// 手动校准
// --------------------------------------------------------------------------- //

/// 单轴校准：留空的项保持内核上报值。
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AxisCal {
    pub min: Option<i32>,
    pub max: Option<i32>,
    /// units per mm（与 evdev `resolution` 同一量纲）。
    pub units_per_mm: Option<f64>,
}

impl AxisCal {
    pub fn is_unset(&self) -> bool {
        self.min.is_none() && self.max.is_none() && self.units_per_mm.is_none()
    }
}

/// 手动校准段。
///
/// 只做全局线性映射：改范围、改分辨率。**不做**轴交换与翻转——那要改事件流，
/// 且真实机型里极少见；宁可让用户换 `--device`，也不摆一个配了没反应的开关。
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalibrationSpec {
    pub x: AxisCal,
    pub y: AxisCal,
}

impl CalibrationSpec {
    pub fn is_unset(&self) -> bool {
        self.x.is_unset() && self.y.is_unset()
    }

    /// 只靠配置本身就能判定的问题（不依赖面板实际范围）。
    ///
    /// `sanitize` 阶段还没摸到设备，拿不到 `ABS_MT_POSITION_*`；这里只查「写的范围反了」
    /// 和「单位换算是垃圾值」，好让配置一加载就把问题说出来。
    pub fn notes(&self) -> Vec<CalNote> {
        fn one(cal: &AxisCal, axis: &'static str, out: &mut Vec<CalNote>) {
            if let (Some(a), Some(b)) = (cal.min, cal.max) {
                if a >= b {
                    out.push(CalNote::Inverted(axis));
                }
            }
            if let Some(u) = cal.units_per_mm {
                if !(u > 0.0 && u.is_finite()) {
                    out.push(CalNote::BadUnitsPerMm(axis));
                }
            }
        }
        let mut out = Vec::new();
        one(&self.x, "x", &mut out);
        one(&self.y, "y", &mut out);
        out
    }

    /// 校正到面板参数上；返回 `(panel, notes)`。
    ///
    /// `notes` 里是「写了但不生效」的项（例如只给了 `min` 没给 `max`、范围反了），
    /// 由上层转成提示语——静默丢弃是最难查的那类问题。
    pub fn apply(&self, base: Panel) -> (Panel, Vec<CalNote>) {
        let mut notes = Vec::new();
        let mut p = base;
        let (nx, xnotes) = apply_axis(base.xmin, base.xmax, &self.x, "x");
        p.xmin = nx.0;
        p.xmax = nx.1;
        notes.extend(xnotes);
        let (ny, ynotes) = apply_axis(base.ymin, base.ymax, &self.y, "y");
        p.ymin = ny.0;
        p.ymax = ny.1;
        notes.extend(ynotes);
        if let Some(upm) = self.x.units_per_mm {
            if upm > 0.0 && upm.is_finite() {
                p.ux = upm / 1000.0;
            } else {
                notes.push(CalNote::BadUnitsPerMm("x"));
            }
        }
        if let Some(upm) = self.y.units_per_mm {
            if upm > 0.0 && upm.is_finite() {
                p.uy = upm / 1000.0;
            } else {
                notes.push(CalNote::BadUnitsPerMm("y"));
            }
        }
        (p, notes)
    }
}

fn apply_axis(min: i32, max: i32, cal: &AxisCal, axis: &'static str) -> ((i32, i32), Vec<CalNote>) {
    let mut notes = Vec::new();
    let (mut lo, mut hi) = (min, max);
    match (cal.min, cal.max) {
        (Some(a), Some(b)) => {
            if a < b {
                lo = a;
                hi = b;
            } else {
                notes.push(CalNote::Inverted(axis));
            }
        }
        (Some(a), None) => {
            // 只给下界的常见意图是「左边多留一点」，上界保持内核值
            if a < hi {
                lo = a;
            } else {
                notes.push(CalNote::Inverted(axis));
            }
        }
        (None, Some(b)) => {
            if b > lo {
                hi = b;
            } else {
                notes.push(CalNote::Inverted(axis));
            }
        }
        (None, None) => {}
    }
    ((lo, hi), notes)
}

/// 校准段的提示项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalNote {
    /// 该轴写的范围是反的（下界不小于上界），整轴忽略
    Inverted(&'static str),
    /// 该轴的单位换算值非正/非有限，忽略
    BadUnitsPerMm(&'static str),
}

impl CalNote {
    /// i18n 键。
    pub fn key(self) -> &'static str {
        match self {
            CalNote::Inverted(_) => "note.calibration_inverted",
            CalNote::BadUnitsPerMm(_) => "note.calibration_bad_units",
        }
    }

    pub fn axis(self) -> &'static str {
        match self {
            CalNote::Inverted(a) | CalNote::BadUnitsPerMm(a) => a,
        }
    }
}

// --------------------------------------------------------------------------- //
// 自定义分区
// --------------------------------------------------------------------------- //

/// 自定义分区：把网格换成任意行列划分。
///
/// 四个边界来源按优先级：`rect`（绝对坐标）> `rect_mm`（相对面板左上角的毫米）>
/// 官方参数推算出的网格。只写 `cols` / `rows` / 比例而不写矩形，就只改划分、
/// 不动位置——「触摸板对、只是键位排布不一样」的机型走这条。
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartitionSpec {
    /// 对哪套布局生效：`auto`（当前生效布局）/ 具体布局名 / 自定义布局名。
    pub layout: String,
    /// 网格矩形 `[x0, y0, x1, y1]`，触摸板绝对坐标。
    pub rect: Option<[f64; 4]>,
    /// 网格矩形 `[x0, y0, x1, y1]`，相对面板左上角的毫米。
    pub rect_mm: Option<[f64; 4]>,
    /// 列数；留空跟随键表。
    pub cols: Option<usize>,
    /// 行数；留空跟随键表。
    pub rows: Option<usize>,
    /// 列宽权重（长度须等于列数，可不等分）；留空 = 等分。
    pub col_ratios: Vec<f64>,
    /// 行高权重；留空 = 等分。
    pub row_ratios: Vec<f64>,
    /// NumLock 图标所在格 `[row, col]`；留空 = 第 0 行最后一列。
    pub numlock_cell: Option<[usize; 2]>,
    /// 网格外落点吸附到最近一格（默认关闭，与官方「网格外属于光标区」一致）。
    pub snap_outside: bool,
}

/// 分区生效范围判定是否有意义——全空时整段不参与。
impl PartitionSpec {
    pub fn is_unset(&self) -> bool {
        self.rect.is_none()
            && self.rect_mm.is_none()
            && self.cols.is_none()
            && self.rows.is_none()
            && self.col_ratios.is_empty()
            && self.row_ratios.is_empty()
            && self.numlock_cell.is_none()
            && !self.snap_outside
    }

    /// 是否对这套布局生效（`auto` / 空 = 一律生效）。
    pub fn applies_to(&self, layout_name: &str) -> bool {
        let want = self.layout.trim();
        want.is_empty() || want.eq_ignore_ascii_case("auto") || want.eq_ignore_ascii_case(layout_name)
    }

    /// 解算成可直接喂给 [`crate::layout::Geometry::with_partition`] 的边界。
    ///
    /// `base_grid` 是官方参数推出来的网格矩形，`keys` 是键表的 `(列数, 行数)`。
    pub fn resolve(
        &self,
        panel: Panel,
        base_grid: (f64, f64, f64, f64),
        keys: (usize, usize),
    ) -> Result<ResolvedPartition, String> {
        let cols = self.cols.unwrap_or(keys.0.max(1));
        let rows = self.rows.unwrap_or(keys.1.max(1));
        if !(1..=8).contains(&cols) {
            return Err(format!("partition: cols {cols} outside 1..=8"));
        }
        if !(1..=8).contains(&rows) {
            return Err(format!("partition: rows {rows} outside 1..=8"));
        }
        let (x0, y0, x1, y1) = if let Some(r) = self.rect {
            (r[0], r[1], r[2], r[3])
        } else if let Some(m) = self.rect_mm {
            (
                panel.mm_to_x(m[0]),
                panel.mm_to_y(m[1]),
                panel.mm_to_x(m[2]),
                panel.mm_to_y(m[3]),
            )
        } else {
            base_grid
        };
        if x1 <= x0 || y1 <= y0 {
            return Err(format!(
                "partition: rect [{x0:.0}, {y0:.0}, {x1:.0}, {y1:.0}] is empty or inverted"
            ));
        }
        if x0 < panel.xmin as f64 || x1 > panel.xmax as f64 {
            return Err(format!(
                "partition: x span {x0:.0}..{x1:.0} outside panel {}..{}",
                panel.xmin, panel.xmax
            ));
        }
        if y0 < panel.ymin as f64 || y1 > panel.ymax as f64 {
            return Err(format!(
                "partition: y span {y0:.0}..{y1:.0} outside panel {}..{}",
                panel.ymin, panel.ymax
            ));
        }
        let x_edges = split(x0, x1, cols, &self.col_ratios, "col")?;
        let y_edges = split(y0, y1, rows, &self.row_ratios, "row")?;
        let nl_cell = self
            .numlock_cell
            .map(|c| (c[0], c[1]))
            .unwrap_or((0, cols - 1));
        if nl_cell.0 >= rows || nl_cell.1 >= cols {
            return Err(format!(
                "partition: numlock cell ({}, {}) outside {rows}x{cols} grid",
                nl_cell.0, nl_cell.1
            ));
        }
        Ok(ResolvedPartition {
            cols,
            rows,
            x_edges,
            y_edges,
            nl_cell: Some(nl_cell),
            snap_outside: self.snap_outside,
            notes: partition_notes(cols, rows, keys),
        })
    }
}

/// 分区与键表不匹配时给用户看的提醒（键表只覆盖重叠部分，多的那部分不出键）。
fn partition_notes(cols: usize, rows: usize, keys: (usize, usize)) -> Vec<PartitionNote> {
    let mut notes = Vec::new();
    if cols > keys.0 {
        notes.push(PartitionNote::ExtraCols(keys.0, cols));
    }
    if rows > keys.1 {
        notes.push(PartitionNote::ExtraRows(keys.1, rows));
    }
    notes
}

/// 分区提示项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionNote {
    /// 网格列数多于键表列数：多出来的列不出键
    ExtraCols(usize, usize),
    /// 网格行数多于键表行数：多出来的行不出键
    ExtraRows(usize, usize),
}

impl PartitionNote {
    pub fn key(self) -> &'static str {
        match self {
            PartitionNote::ExtraCols(..) => "note.partition_extra_cols",
            PartitionNote::ExtraRows(..) => "note.partition_extra_rows",
        }
    }

    pub fn args(self) -> Vec<(&'static str, String)> {
        match self {
            PartitionNote::ExtraCols(have, want) => {
                vec![("have", have.to_string()), ("want", want.to_string())]
            }
            PartitionNote::ExtraRows(have, want) => {
                vec![("have", have.to_string()), ("want", want.to_string())]
            }
        }
    }
}

/// 解算结果。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPartition {
    pub cols: usize,
    pub rows: usize,
    pub x_edges: Vec<f64>,
    pub y_edges: Vec<f64>,
    pub nl_cell: Option<(usize, usize)>,
    pub snap_outside: bool,
    pub notes: Vec<PartitionNote>,
}

/// 把 `[0,1]` 区间按权重切成 `n` 段的边界（长度 `n+1`）。
fn split(
    lo: f64,
    hi: f64,
    n: usize,
    ratios: &[f64],
    what: &str,
) -> Result<Vec<f64>, String> {
    let span = hi - lo;
    if ratios.is_empty() {
        return Ok((0..=n).map(|i| lo + span * i as f64 / n as f64).collect());
    }
    if ratios.len() != n {
        return Err(format!(
            "partition: {what}_ratios has {} entries, need {n}",
            ratios.len()
        ));
    }
    if ratios.iter().any(|r| !(r.is_finite() && *r > 0.0)) {
        return Err(format!("partition: {what}_ratios must all be positive"));
    }
    let total: f64 = ratios.iter().sum();
    let mut out = Vec::with_capacity(n + 1);
    out.push(lo);
    let mut acc = 0.0;
    for r in ratios {
        acc += r;
        out.push(lo + span * acc / total);
    }
    // 浮点累加的末项直接钉到 hi，避免 1 ulp 误差让最后一格比矩形窄
    if let Some(last) = out.last_mut() {
        *last = hi;
    }
    Ok(out)
}

// --------------------------------------------------------------------------- //
// 校准求解
// --------------------------------------------------------------------------- //

/// 四角实测样本（用户按住触摸板四角时读到的坐标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct CornerSamples {
    pub top_left: (i32, i32),
    pub top_right: (i32, i32),
    pub bottom_left: (i32, i32),
    pub bottom_right: (i32, i32),
}

/// 一次交互采集的原始样本：四角 + 两个对角格中心。
///
/// 只装坐标，不装「是哪一格」——格由参数决定（左上格与右下格），
/// 免得用户在采集途中还要回答跟坐标无关的问题。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSamples {
    pub corners: CornerSamples,
    pub first_point: (i32, i32),
    pub last_point: (i32, i32),
}

/// 格中心实测样本：按住两个已知格的中心，用来反解格宽与网格原点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridProbe {
    /// 第一格 `(row, col)` 的中心落点
    pub first_cell: (usize, usize),
    pub first_point: (i32, i32),
    /// 最后一格 `(row, col)` 的中心落点
    pub last_cell: (usize, usize),
    pub last_point: (i32, i32),
    /// 键表列数 / 行数
    pub cols: usize,
    pub rows: usize,
}

/// 校准输出：直接落盘的两段配置。
#[derive(Debug, Clone, PartialEq)]
pub struct Calibration {
    pub panel: Panel,
    pub partition: PartitionSpec,
}

/// 由实测样本解算面板范围与网格矩形。
///
/// 数学很简单，难的是「样本本身是否可信」，所以这里把能查的都查一遍：
///
/// * 四角必须构成一个正的矩形（左右、上下不能反）；
/// * 两个探针格必须不等且落在键表内；
/// * 由探针反解出的格宽格高必须为正，且拼出的网格不能翻出面板。
///
/// 反解公式：格中心 `c = x0 + (col + 0.5) * w`，两式相减即
/// `w = (c_last - c_first) / (col_last - col_first)`，再回代求 `x0`。
pub fn solve_calibration(
    corners: &CornerSamples,
    probe: &GridProbe,
) -> Result<Calibration, String> {
    if probe.first_cell == probe.last_cell {
        return Err("calibrate: the two probes must be different cells".to_string());
    }
    if probe.first_cell.0 >= probe.rows
        || probe.last_cell.0 >= probe.rows
        || probe.first_cell.1 >= probe.cols
        || probe.last_cell.1 >= probe.cols
    {
        return Err(format!(
            "calibrate: probe cell outside {}x{} key table",
            probe.rows, probe.cols
        ));
    }
    let xs = [
        corners.top_left.0,
        corners.top_right.0,
        corners.bottom_left.0,
        corners.bottom_right.0,
    ];
    let ys = [
        corners.top_left.1,
        corners.top_right.1,
        corners.bottom_left.1,
        corners.bottom_right.1,
    ];
    let xmin = *xs.iter().min().expect("4 corners");
    let xmax = *xs.iter().max().expect("4 corners");
    let ymin = *ys.iter().min().expect("4 corners");
    let ymax = *ys.iter().max().expect("4 corners");
    if xmax == xmin || ymax == ymin {
        return Err("calibrate: corners collapse to a line/point".to_string());
    }
    // 左上角必须真的在左上：否则说明用户按错了顺序，或该轴是反的
    if corners.top_left.0 > corners.top_right.0 || corners.top_left.1 > corners.bottom_left.1 {
        return Err("calibrate: corner order looks wrong (is the Y axis inverted?)".to_string());
    }

    let dcol = probe.last_cell.1 as f64 - probe.first_cell.1 as f64;
    let drow = probe.last_cell.0 as f64 - probe.first_cell.0 as f64;
    let col_w = if dcol.abs() > 0.5 {
        (probe.last_point.0 - probe.first_point.0) as f64 / dcol
    } else {
        (xmax - xmin) as f64 / probe.cols as f64
    };
    let row_h = if drow.abs() > 0.5 {
        (probe.last_point.1 - probe.first_point.1) as f64 / drow
    } else {
        (ymax - ymin) as f64 / probe.rows as f64
    };
    if col_w <= 0.0 {
        return Err(format!("calibrate: derived column width {col_w:.1} <= 0"));
    }
    if row_h <= 0.0 {
        return Err(format!("calibrate: derived row height {row_h:.1} <= 0"));
    }
    let grid_x0 = probe.first_point.0 as f64 - (probe.first_cell.1 as f64 + 0.5) * col_w;
    let grid_y0 = probe.first_point.1 as f64 - (probe.first_cell.0 as f64 + 0.5) * row_h;
    let grid_x1 = grid_x0 + probe.cols as f64 * col_w;
    let grid_y1 = grid_y0 + probe.rows as f64 * row_h;

    // 网格允许比四角落点略外一点（用户按不到物理边缘），但不能离谱
    let slack_x = col_w * 0.5;
    let slack_y = row_h * 0.5;
    if grid_x0 < xmin as f64 - slack_x || grid_x1 > xmax as f64 + slack_x {
        return Err(format!(
            "calibrate: derived grid x {grid_x0:.0}..{grid_x1:.0} outside panel {xmin}..{xmax}"
        ));
    }
    if grid_y0 < ymin as f64 - slack_y || grid_y1 > ymax as f64 + slack_y {
        return Err(format!(
            "calibrate: derived grid y {grid_y0:.0}..{grid_y1:.0} outside panel {ymin}..{ymax}"
        ));
    }

    let panel = Panel {
        xmin,
        xmax,
        ymin,
        ymax,
        // 分辨率未知：交给上层按标称物理尺寸换算（与内核没报 resolution 时同一路径）
        ux: 0.0,
        uy: 0.0,
    };
    let partition = PartitionSpec {
        layout: "auto".to_string(),
        rect: Some([
            grid_x0.round(),
            grid_y0.round(),
            grid_x1.round(),
            grid_y1.round(),
        ]),
        rect_mm: None,
        cols: Some(probe.cols),
        rows: Some(probe.rows),
        col_ratios: Vec::new(),
        row_ratios: Vec::new(),
        numlock_cell: None,
        snap_outside: false,
    };
    Ok(Calibration { panel, partition })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> Panel {
        Panel {
            xmin: 0,
            xmax: 4752,
            ymin: 0,
            ymax: 3008,
            ux: 45.0 / 1000.0,
            uy: 42.0 / 1000.0,
        }
    }

    #[test]
    fn split_even_and_weighted() {
        let e = split(0.0, 100.0, 4, &[], "col").unwrap();
        assert_eq!(e, vec![0.0, 25.0, 50.0, 75.0, 100.0]);
        let w = split(0.0, 100.0, 2, &[3.0, 1.0], "col").unwrap();
        assert_eq!(w, vec![0.0, 75.0, 100.0]);
        assert!(split(0.0, 100.0, 2, &[1.0], "col").is_err());
        assert!(split(0.0, 100.0, 2, &[1.0, 0.0], "col").is_err());
    }

    #[test]
    fn partition_resolves_uniform_over_base_grid() {
        let p = PartitionSpec {
            cols: Some(4),
            rows: Some(3),
            ..Default::default()
        };
        let r = p.resolve(panel(), (100.0, 100.0, 900.0, 600.0), (5, 4)).unwrap();
        assert_eq!(r.x_edges.len(), 5);
        assert_eq!(r.y_edges.len(), 4);
        assert_eq!(r.x_edges[0], 100.0);
        assert_eq!(r.x_edges[4], 900.0);
        assert_eq!(r.nl_cell, Some((0, 3)));
        assert!(r.notes.is_empty());
    }

    #[test]
    fn partition_rect_mm_converts_through_panel() {
        let p = PartitionSpec {
            rect_mm: Some([10.0, 10.0, 100.0, 70.0]),
            ..Default::default()
        };
        let r = p.resolve(panel(), (0.0, 0.0, 1.0, 1.0), (5, 4)).unwrap();
        // 10 mm × 45 units/mm = 450
        assert!((r.x_edges[0] - 450.0).abs() < 1.0, "{:?}", r.x_edges);
        // 70 mm × 42 units/mm = 2940
        assert!((r.y_edges[4] - 2940.0).abs() < 1.0, "{:?}", r.y_edges);
    }

    #[test]
    fn partition_rejects_bad_shapes() {
        let p = PartitionSpec {
            rect: Some([500.0, 100.0, 100.0, 600.0]),
            ..Default::default()
        };
        assert!(p.resolve(panel(), (0.0, 0.0, 1.0, 1.0), (5, 4)).is_err());
        let p = PartitionSpec {
            rect: Some([0.0, 0.0, 5000.0, 600.0]),
            ..Default::default()
        };
        assert!(p.resolve(panel(), (0.0, 0.0, 1.0, 1.0), (5, 4)).is_err());
        let p = PartitionSpec {
            cols: Some(9),
            ..Default::default()
        };
        assert!(p.resolve(panel(), (0.0, 0.0, 1.0, 1.0), (5, 4)).is_err());
        let p = PartitionSpec {
            numlock_cell: Some([9, 0]),
            ..Default::default()
        };
        assert!(p.resolve(panel(), (0.0, 0.0, 1.0, 1.0), (5, 4)).is_err());
    }

    #[test]
    fn partition_notes_when_grid_wider_than_keytable() {
        let p = PartitionSpec {
            cols: Some(7),
            ..Default::default()
        };
        let r = p.resolve(panel(), (100.0, 100.0, 900.0, 600.0), (5, 4)).unwrap();
        assert_eq!(r.notes, vec![PartitionNote::ExtraCols(5, 7)]);
    }

    #[test]
    fn partition_only_applies_to_its_layout() {
        let mut p = PartitionSpec {
            layout: "HANDMADE".to_string(),
            ..Default::default()
        };
        assert!(p.applies_to("handmade"));
        assert!(!p.applies_to("GH3XLNDZ"));
        p.layout = "auto".to_string();
        assert!(p.applies_to("GH3XLNDZ"));
    }

    #[test]
    fn calibration_overrides_only_written_items() {
        let cal = CalibrationSpec {
            x: AxisCal {
                min: Some(-10),
                max: Some(4800),
                units_per_mm: Some(40.0),
            },
            y: AxisCal::default(),
        };
        let (p, notes) = cal.apply(panel());
        assert_eq!(p.xmin, -10);
        assert_eq!(p.xmax, 4800);
        assert!((p.ux - 0.04).abs() < 1e-12);
        assert_eq!(p.ymin, 0);
        assert_eq!(p.ymax, 3008);
        assert!((p.uy - 42.0 / 1000.0).abs() < 1e-12);
        assert!(notes.is_empty());
    }

    #[test]
    fn calibration_notes_do_not_need_a_panel() {
        let cal = CalibrationSpec {
            x: AxisCal {
                min: Some(10),
                max: Some(10),
                units_per_mm: None,
            },
            y: AxisCal {
                min: Some(-5),
                max: None,
                units_per_mm: Some(0.0),
            },
        };
        assert_eq!(
            cal.notes(),
            vec![CalNote::Inverted("x"), CalNote::BadUnitsPerMm("y")]
        );
        assert!(CalibrationSpec::default().notes().is_empty());
    }

    #[test]
    fn calibration_flags_inverted_and_bad_units() {
        let cal = CalibrationSpec {
            x: AxisCal {
                min: Some(4800),
                max: Some(10),
                units_per_mm: Some(-1.0),
            },
            y: AxisCal::default(),
        };
        let (p, notes) = cal.apply(panel());
        // 反了的整轴忽略，保持内核值
        assert_eq!((p.xmin, p.xmax), (0, 4752));
        assert_eq!(
            notes,
            vec![
                CalNote::Inverted("x"),
                CalNote::BadUnitsPerMm("x"),
            ]
        );
    }

    #[test]
    fn solve_calibration_recovers_grid_from_probes() {
        // 真实面板 0..4752 / 0..3008，网格 x0=300 x1=4500（5 列，格宽 840）
        // y0=200 y1=2900（4 行，格高 675）
        let col_w = 840.0;
        let row_h = 675.0;
        let (gx0, gy0) = (300.0, 200.0);
        let center = |row: usize, col: usize| {
            (
                (gx0 + (col as f64 + 0.5) * col_w) as i32,
                (gy0 + (row as f64 + 0.5) * row_h) as i32,
            )
        };
        let corners = CornerSamples {
            top_left: (0, 0),
            top_right: (4752, 0),
            bottom_left: (0, 3008),
            bottom_right: (4752, 3008),
        };
        let probe = GridProbe {
            first_cell: (0, 0),
            first_point: center(0, 0),
            last_cell: (2, 4),
            last_point: center(2, 4),
            cols: 5,
            rows: 4,
        };
        let c = solve_calibration(&corners, &probe).unwrap();
        assert_eq!((c.panel.xmin, c.panel.xmax), (0, 4752));
        let rect = c.partition.rect.unwrap();
        assert!((rect[0] - gx0).abs() < 1.0, "{rect:?}");
        assert!((rect[1] - gy0).abs() < 1.0, "{rect:?}");
        assert!((rect[2] - (gx0 + 5.0 * col_w)).abs() < 1.0, "{rect:?}");
        assert!((rect[3] - (gy0 + 4.0 * row_h)).abs() < 1.0, "{rect:?}");
    }

    #[test]
    fn solve_calibration_falls_back_to_panel_split_without_span() {
        let corners = CornerSamples {
            top_left: (0, 0),
            top_right: (1000, 0),
            bottom_left: (0, 800),
            bottom_right: (1000, 800),
        };
        // 两个探针落在同一列：横向退回「面板等分」，纵向仍由探针反解
        let probe = GridProbe {
            first_cell: (0, 0),
            first_point: (100, 100),
            last_cell: (3, 0),
            last_point: (100, 700),
            cols: 5,
            rows: 4,
        };
        let c = solve_calibration(&corners, &probe).unwrap();
        let rect = c.partition.rect.unwrap();
        assert!((rect[0] - 0.0).abs() < 1.0);
        assert!((rect[2] - 1000.0).abs() < 1.0);
    }

    #[test]
    fn solve_calibration_rejects_bad_input() {
        let corners = CornerSamples {
            top_left: (0, 0),
            top_right: (4752, 0),
            bottom_left: (0, 3008),
            bottom_right: (4752, 3008),
        };
        let same = GridProbe {
            first_cell: (0, 0),
            first_point: (420, 537),
            last_cell: (0, 0),
            last_point: (420, 537),
            cols: 5,
            rows: 4,
        };
        assert!(solve_calibration(&corners, &same).is_err());

        let outside = GridProbe {
            first_cell: (0, 0),
            first_point: (420, 537),
            last_cell: (3, 4),
            last_point: (8000, 3000),
            cols: 5,
            rows: 4,
        };
        assert!(solve_calibration(&corners, &outside).is_err());

        let flipped = CornerSamples {
            top_left: (4752, 3008),
            top_right: (0, 3008),
            bottom_left: (4752, 0),
            bottom_right: (0, 0),
        };
        assert!(solve_calibration(&flipped, &outside).is_err());
    }

    #[test]
    fn partition_roundtrips_through_json() {
        let text = r#"{
            "layout": "HANDMADE",
            "rect": [300, 200, 4500, 2900],
            "cols": 5,
            "rows": 4,
            "col_ratios": [1, 1, 1, 1, 1],
            "numlock_cell": [0, 4],
            "snap_outside": true
        }"#;
        let p: PartitionSpec = serde_json::from_str(text).unwrap();
        assert_eq!(p.layout, "HANDMADE");
        assert_eq!(p.cols, Some(5));
        assert!(p.snap_outside);
        let back: PartitionSpec = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
        // 拼错字段必须报错，而不是静默忽略
        assert!(serde_json::from_str::<PartitionSpec>(r#"{"rectt": [1,2,3,4]}"#).is_err());
    }
}
