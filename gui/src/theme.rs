//! 界面外观：一套「卡片 + 单一强调色」的样式，以及几个排版小工具。
//!
//! 样式只在 [`App::new`](crate::app::App::new) 里装一次（[`install`]），页面代码照旧
//! 用 `ui.button` / `ui.label`，颜色、圆角、间距、字号统一从这里出，避免各页面各写一套。
//!
//! 配色跟着系统深浅色走：Windows / Linux 桌面默认是浅色，硬塞深色反而突兀，所以两套
//! 调色板都在这里定义，按 `ctx.theme()` 取一套；`ASUS_NUMPAD_GUI_THEME=dark|light`
//! 可以强制覆盖（截图和交稿时用得上）。

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Response, RichText, Sense, Stroke,
    TextStyle, Vec2,
};

/// 一整套颜色。两套调色板字段一致，界面代码只用字段名，不写死色值。
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// 窗口底色（卡片之间的留白）
    pub bg: Color32,
    /// 侧栏 / 顶栏底色
    pub panel: Color32,
    /// 卡片底色
    pub card: Color32,
    /// 卡片内的凹陷区 / 悬停底色
    pub sunk: Color32,
    /// 分割线、卡片描边
    pub stroke: Color32,
    /// 正文
    pub text: Color32,
    /// 次要正文（说明文字）
    pub text_weak: Color32,
    /// 强调色（选中、主按钮、进度）
    pub accent: Color32,
    /// 强调色的淡底
    pub accent_soft: Color32,
    /// 正常状态
    pub ok: Color32,
    /// 告警
    pub warn: Color32,
    /// 错误
    pub bad: Color32,
}

/// 深色：背景近黑的蓝灰，强调色偏冷的蓝紫，长时间看不刺眼。
pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(13, 16, 21),
    panel: Color32::from_rgb(18, 22, 29),
    card: Color32::from_rgb(23, 28, 37),
    sunk: Color32::from_rgb(30, 36, 47),
    stroke: Color32::from_rgb(40, 47, 60),
    text: Color32::from_rgb(231, 235, 242),
    text_weak: Color32::from_rgb(147, 160, 180),
    accent: Color32::from_rgb(124, 156, 255),
    // 14% 透明度的强调色，已按预乘写好（egui 用预乘 alpha）
    accent_soft: Color32::from_rgba_premultiplied(21, 27, 44, 44),
    ok: Color32::from_rgb(74, 202, 138),
    warn: Color32::from_rgb(233, 176, 78),
    bad: Color32::from_rgb(240, 110, 110),
};

/// 浅色：白卡片浮在浅灰底上，强调色用更深的蓝，保证在浅底上也有足够对比。
pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(242, 245, 250),
    panel: Color32::from_rgb(255, 255, 255),
    card: Color32::from_rgb(255, 255, 255),
    sunk: Color32::from_rgb(237, 241, 248),
    stroke: Color32::from_rgb(219, 226, 236),
    text: Color32::from_rgb(26, 33, 46),
    text_weak: Color32::from_rgb(108, 120, 138),
    accent: Color32::from_rgb(46, 104, 232),
    accent_soft: Color32::from_rgba_premultiplied(5, 12, 27, 30),
    ok: Color32::from_rgb(21, 145, 90),
    warn: Color32::from_rgb(176, 112, 16),
    bad: Color32::from_rgb(200, 62, 62),
};

/// 卡片圆角，比控件大一档，视觉上才能把里面的控件「装」住。
const CARD_RADIUS: u8 = 14;
/// 控件圆角。
const WIDGET_RADIUS: u8 = 9;

/// 装上主题，返回当前生效的调色板。
pub fn install(ctx: &egui::Context) -> Palette {
    let forced = std::env::var("ASUS_NUMPAD_GUI_THEME").ok();
    let dark = match forced.as_deref().map(str::trim) {
        Some("dark") => true,
        Some("light") => false,
        _ => ctx.style_of(ctx.theme()).visuals.dark_mode,
    };
    // 两套都装：用户中途切系统主题时，界面不会掉回 egui 默认的紫色灰底
    ctx.set_style_of(egui::Theme::Dark, style_for(DARK, true));
    ctx.set_style_of(egui::Theme::Light, style_for(LIGHT, false));
    let theme = egui::Theme::from_dark_mode(dark);
    ctx.set_theme(theme);
    if dark {
        DARK
    } else {
        LIGHT
    }
}

fn style_for(p: Palette, dark: bool) -> egui::Style {
    let mut style = egui::Theme::from_dark_mode(dark).default_style();
    let mut v = style.visuals;
    v.dark_mode = dark;

    // 字号：正文略大于 egui 默认的 14，说明文字压到 12，标题 19 拉开层级
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(19.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    // 间距：默认值偏挤，卡片式布局要留出呼吸感
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 7.0);
    style.spacing.interact_size = Vec2::new(44.0, 28.0);
    style.spacing.slider_width = 190.0;
    style.spacing.combo_width = 220.0;
    style.spacing.indent = 22.0;
    style.spacing.menu_margin = Margin::same(8);

    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.extreme_bg_color = if dark { p.bg } else { p.sunk };
    v.faint_bg_color = p.sunk;
    v.hyperlink_color = p.accent;
    v.weak_text_color = Some(p.text_weak);
    v.override_text_color = Some(p.text);
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(12);
    v.window_stroke = Stroke::new(1.0, p.stroke);
    v.window_shadow = egui::epaint::Shadow {
        offset: [0, 10],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 120 } else { 40 }),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 100 } else { 32 }),
    };
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);

    // 控件四态：静默=卡片底的浅一档，悬停=提亮并描一圈强调色，按下=强调色淡底
    let widgets = &mut v.widgets;
    widgets.noninteractive.bg_fill = p.card;
    widgets.noninteractive.weak_bg_fill = p.card;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.stroke);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    widgets.noninteractive.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.inactive.bg_fill = p.sunk;
    widgets.inactive.weak_bg_fill = p.sunk;
    widgets.inactive.bg_stroke = Stroke::new(1.0, p.stroke);
    widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    widgets.inactive.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.hovered.bg_fill = p.sunk;
    widgets.hovered.weak_bg_fill = p.sunk;
    widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    widgets.hovered.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.active.bg_fill = p.accent_soft;
    widgets.active.weak_bg_fill = p.accent_soft;
    widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    widgets.active.fg_stroke = Stroke::new(1.0, p.accent);
    widgets.active.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.open.bg_fill = p.sunk;
    widgets.open.weak_bg_fill = p.sunk;
    widgets.open.bg_stroke = Stroke::new(1.0, p.stroke);
    widgets.open.fg_stroke = Stroke::new(1.0, p.text);
    widgets.open.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    style.visuals = v;
    style
}

/// 一张卡片：圆角、细描边、内留白，外加一层几乎看不见的投影。
///
/// 投影不是为了「拟物」，而是让白卡片压在浅灰底上有个边界——只靠 1px 描边时，
/// 高分屏上卡片和背景会糊成一片。
pub fn card<R>(ui: &mut egui::Ui, p: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let dark = p.bg.to_array()[0] < 128;
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.stroke))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::symmetric(16, 14))
        .shadow(egui::epaint::Shadow {
            offset: [0, 2],
            blur: 6,
            spread: 0,
            color: Color32::from_black_alpha(if dark { 70 } else { 16 }),
        })
        .show(ui, |ui| {
            // 卡片里不缩进：折叠头、列表这些默认会按 indent 往右推，一推控件列就跟着
            // 偏移，同一份表单在「折叠卡片」和「带标题卡片」里就错开十几像素。
            ui.spacing_mut().indent = 0.0;
            add(ui)
        })
        .inner
}

/// 带标题的卡片：标题前一小条强调色，代替原来的 `heading + separator`。
pub fn section<R>(
    ui: &mut egui::Ui,
    p: &Palette,
    title: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    card(ui, p, |ui| {
        title_row(ui, p, title);
        ui.add_space(2.0);
        add(ui)
    })
}

/// 卡片内的小标题。标题字重加粗，前面一条 3×16 的强调色竖条。
pub fn title_row(ui: &mut egui::Ui, p: &Palette, title: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(3.0, 16.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same(2), p.accent);
        ui.add_space(2.0);
        ui.label(RichText::new(title).size(15.5).strong().color(p.text));
    });
}

/// 主按钮：实心强调色，用于「点亮」这类主动作。
pub fn primary(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    // 两套调色板的强调色都足够深，白字始终是安全的
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
            .fill(p.accent)
            .stroke(Stroke::new(0.0, Color32::TRANSPARENT))
            .corner_radius(CornerRadius::same(WIDGET_RADIUS))
            .min_size(Vec2::new(BTN_W, CTRL_H)),
    )
}

/// 次级按钮：卡片底色 + 描边，和主按钮并排时不抢注意力。
pub fn secondary(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(p.text))
            .fill(p.sunk)
            .stroke(Stroke::new(1.0, p.stroke))
            .corner_radius(CornerRadius::same(WIDGET_RADIUS))
            .min_size(Vec2::new(BTN_W, CTRL_H)),
    )
}

/// 状态胶囊：圆角小标签，用来显示「真实硬件 / 模拟后端」这类一眼要看到的状态。
pub fn pill(ui: &mut egui::Ui, text: &str, color: Color32) -> Response {
    let font = FontId::proportional(12.0);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let size = Vec2::new(galley.size().x + 18.0, galley.size().y + 8.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let bg = Color32::from_rgba_premultiplied(
        (color.r() as u16 * 44 / 255) as u8,
        (color.g() as u16 * 44 / 255) as u8,
        (color.b() as u16 * 44 / 255) as u8,
        44,
    );
    ui.painter().rect(
        rect,
        CornerRadius::same((size.y / 2.0) as u8),
        bg,
        Stroke::new(1.0, color.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    resp
}

/// 分段控件（页签）：一条圆角轨道，选中的那格用强调色淡底托起。
///
/// 用自绘而不是 `selectable_label`：默认的选中态只有一点点底色，五个页签横排时
/// 分不清「当前在哪」，分段控件一眼就能看出来。
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    p: &Palette,
    id: &str,
    current: T,
    items: &[(T, &str, &str)],
) -> Option<T> {
    let font = FontId::proportional(14.0);
    let pad = Vec2::new(16.0, 7.0);
    let widths: Vec<f32> = items
        .iter()
        .map(|(_, text, _)| {
            ui.painter()
                .layout_no_wrap((*text).to_owned(), font.clone(), p.text)
                .size()
                .x
                + pad.x * 2.0
        })
        .collect();
    // 铺满控件列：两格按文字宽度成比例分配，右边缘于是和下拉框、按钮组落在同一条线上
    let total = ui.available_width().min(ctrl_width()).max(120.0);
    let natural = widths.iter().sum::<f32>() + 8.0;
    let scale = if natural > 0.0 {
        (total - 8.0) / (natural - 8.0)
    } else {
        1.0
    };
    let widths: Vec<f32> = widths.iter().map(|w| w * scale).collect();
    let height = pad.y * 2.0 + 16.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, height), Sense::hover());

    ui.painter()
        .rect_filled(rect, CornerRadius::same((height / 2.0) as u8), p.sunk);

    let mut picked = None;
    let mut x = rect.left() + 4.0;
    for ((value, text, help), width) in items.iter().zip(&widths) {
        let item = egui::Rect::from_min_size(
            egui::pos2(x, rect.top() + 4.0),
            Vec2::new(*width, height - 8.0),
        );
        let selected = *value == current;
        let resp = ui.interact(item, ui.scope_id().with((id, text)), Sense::click());
        if selected || resp.hovered() {
            ui.painter().rect_filled(
                item,
                CornerRadius::same(((height - 8.0) / 2.0) as u8),
                if selected {
                    ui.visuals().window_fill
                } else {
                    p.stroke.gamma_multiply(0.5)
                },
            );
        }
        let color = if selected { p.accent } else { p.text_weak };
        ui.painter().text(
            item.center(),
            egui::Align2::CENTER_CENTER,
            *text,
            font.clone(),
            if resp.hovered() && !selected {
                p.text
            } else {
                color
            },
        );
        if resp.hovered() && !help.is_empty() {
            tip(resp.clone(), *help);
        }
        if resp.clicked() {
            picked = Some(*value);
        }
        x += width;
    }
    picked
}

/// 亮度档位条：一格一档，点哪格就是哪档。
///
/// 比默认 `Slider` 直观——滑块只能看到当前位置，看不到「一共几档、还能往哪走」；
/// 这里每格都画出来，前 N 格实心表示当前亮度，后面留空。
pub fn level_meter(ui: &mut egui::Ui, p: &Palette, id: &str, step: u8, steps: u8) -> Option<u8> {
    let steps = steps.max(1);
    let gaps = (steps - 1) as f32 * 6.0;
    let width = ui.available_width().min(520.0);
    let cell = ((width - gaps) / steps as f32).max(18.0);
    let height = 30.0;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(cell * steps as f32 + gaps, height),
        Sense::hover(),
    );

    let mut picked = None;
    for i in 0..steps {
        let cell_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + i as f32 * (cell + 6.0), rect.top()),
            Vec2::new(cell, height),
        );
        let index = i + 1;
        let lit = index <= step;
        let resp = tip(
            ui.interact(cell_rect, ui.scope_id().with((id, index)), Sense::click()),
            format!("第 {index} 档：点这一格直接切到该亮度"),
        );
        let fill = if lit {
            // 越亮越偏暖，暗档用强调色的深色版本，视觉上有「渐亮」的连续感
            let t = if steps > 1 {
                (index - 1) as f32 / (steps - 1) as f32
            } else {
                1.0
            };
            lerp_color(
                p.accent.gamma_multiply(0.7),
                Color32::from_rgb(255, 208, 138),
                t,
            )
        } else {
            p.sunk
        };
        let stroke = if resp.hovered() {
            Stroke::new(1.0, p.accent)
        } else {
            Stroke::new(1.0, p.stroke)
        };
        ui.painter().rect(
            cell_rect,
            CornerRadius::same(8),
            fill,
            stroke,
            egui::StrokeKind::Inside,
        );
        // 点亮的格子是暖色实心，用深色字；未点亮是凹底色，用次要色
        let text_color = if lit {
            Color32::from_rgb(38, 30, 12)
        } else {
            p.text_weak
        };
        ui.painter().text(
            cell_rect.center(),
            egui::Align2::CENTER_CENTER,
            index.to_string(),
            FontId::proportional(13.0),
            text_color,
        );
        if resp.clicked() {
            picked = Some(index);
        }
    }
    picked
}

fn lerp_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

/// 表单左列宽度。所有「标签 + 控件」的行共用这一列，控件因此永远从同一条竖线起，
/// 换行、换卡片都不会左右错位。
///
/// 取 132 而不是更窄：中文标签（「应用手势触发距离」「独占触摸板」）在 13px 下排开
/// 就要 100 出头，列宽给小了标签会折成两行，行高一变，整片表单就跟着参差。
pub const LABEL_W: f32 = 132.0;

/// 卡片内容到卡片边缘的左右留白（[`card`] 的 `inner_margin`）。
pub const CARD_PAD_X: f32 = 16.0;

/// 标签列与控件列之间的间距，与全局 `item_spacing.x` 取同一个值。
pub const ROW_GAP: f32 = 10.0;

/// 控件列宽度下限：窄窗口下也不能窄到把下拉框里的档位名切掉。
pub const CTRL_MIN_W: f32 = 280.0;

/// 控件列宽度上限：正好放得下一排四颗 [`BTN_W`] 按钮（4×104 + 3×8）。
///
/// 上限存在的意义是别让控件在大窗口里拉成一条横贯整页的输入框；
/// 四颗按钮这一档是页面上最宽的一组控件，控件列只要够它，其余行就不会有任何
/// 元素越出控件列的右边缘。
pub const CTRL_MAX_W: f32 = 4.0 * BTN_W + 3.0 * 8.0;

/// 控件行高。输入框、下拉框、数字框、按钮都取这个高度，一行里高矮一致。
pub const CTRL_H: f32 = 30.0;

/// 一格并排两个控件时左边那个（勾选框、小序号）的宽度；右边那个补齐到
/// 控件列剩下的宽度，于是「两个小控件一行」与「一个大控件」的右边缘仍然齐平。
pub const PAIR_A: f32 = 84.0;
/// 并排两控件之间的间距。
pub const PAIR_GAP: f32 = 8.0;

/// 按钮统一宽度：全界面只有这一个按钮宽度。
pub const BTN_W: f32 = 104.0;

/// 当前页的控件列宽度（以千分之一像素存，`AtomicU32` 不能直接存 `f32`）。
///
/// **为什么要做成每帧算一次的全局值**：每张卡片、每个折叠区里 `ui.available_width()`
/// 各不相同（折叠头、滚动条、内层 `Frame` 都会吃掉几像素），如果每一行各自按自己的
/// 可用宽度算控件列，同一页里就会同时出现 300 / 294 / 288 三种列宽——右边缘差几像素，
/// 看着就是「没对齐」。所以一页只算一次，整页所有行都用这一个数字。
static COLUMN_W_MILLI: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(300_000);

/// 每页开画时调用一次：按本页内容宽度定下控件列宽度。
pub fn begin_page(ui: &mut egui::Ui) {
    use std::sync::atomic::Ordering;
    let content = ui.available_width() - CARD_PAD_X * 2.0;
    let width = (content - LABEL_W - ROW_GAP).clamp(CTRL_MIN_W, CTRL_MAX_W);
    COLUMN_W_MILLI.store((width * 1000.0).round() as u32, Ordering::Relaxed);
}

/// 本页控件列宽度。
pub fn ctrl_width() -> f32 {
    use std::sync::atomic::Ordering;
    COLUMN_W_MILLI.load(Ordering::Relaxed) as f32 / 1000.0
}

/// 并排两控件时右边那个的宽度：控件列减掉左边那格与间距。
pub fn pair_b() -> f32 {
    ctrl_width() - PAIR_A - PAIR_GAP
}

/// 给任意控件挂一段悬浮说明：统一宽度与字号，各页 tooltip 不会长短不一。
pub fn tip(resp: Response, text: impl Into<String>) -> Response {
    let text = text.into();
    resp.on_hover_ui(move |ui| {
        ui.set_max_width(320.0);
        ui.label(RichText::new(text).size(12.5));
    })
}

/// 标签后面那个小圆圈 `i`：鼠标停上去出说明。
///
/// 参数面板里每一项都值得解释一句，但全铺成正文会把界面撑散，所以统一做法是
/// 标签右侧留一个图标，想看的人停一下鼠标。
pub fn info_dot(ui: &mut egui::Ui, p: &Palette, text: impl Into<String>) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
    let hot = resp.hovered();
    // 常态就用主题色描边 + 淡底：灰描边的 1px 圆在浅色卡片上几乎看不见，
    // 用户根本不知道这里能悬浮，所以底色与描边都取主色，只在悬浮时加深。
    let stroke = if hot {
        p.accent
    } else {
        p.accent.gamma_multiply(0.75)
    };
    ui.painter().circle(
        rect.center(),
        7.0,
        if hot {
            p.accent_soft
        } else {
            p.accent_soft.gamma_multiply(0.7)
        },
        Stroke::new(1.2, stroke),
    );
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 0.5),
        egui::Align2::CENTER_CENTER,
        "i",
        FontId::proportional(10.5),
        stroke,
    );
    tip(resp, text)
}

/// 在行内切出一块**宽度精确**的格子，让内部内容照自己的布局画。
///
/// 不用 `allocate_ui_with_layout`：它按内容实际大小结算，标签只有两个字时那一格就
/// 只占两字的宽度，控件起点于是跟着往左跳（同一页里曾出现 492 / 514 / 520 三种起点）。
/// 这里先定死矩形、画完再把矩形整块交给布局，起点与宽度都不受内容影响。
fn cell<R>(
    ui: &mut egui::Ui,
    size: Vec2,
    layout: egui::Layout,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let rect = egui::Rect::from_min_size(ui.next_widget_position(), size);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(layout));
    let out = add(&mut child);
    ui.allocate_rect(rect, Sense::hover());
    out
}

/// 同上，但高度按内容走（多行格子用）。
fn cell_auto_height<R>(
    ui: &mut egui::Ui,
    width: f32,
    layout: egui::Layout,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let start = ui.next_widget_position();
    let max = egui::Rect::from_min_size(start, Vec2::new(width, ui.available_height().max(CTRL_H)));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(max).layout(layout));
    let out = add(&mut child);
    let height = child.min_rect().height().max(CTRL_H);
    ui.allocate_rect(
        egui::Rect::from_min_size(start, Vec2::new(width, height)),
        Sense::hover(),
    );
    out
}

/// 一行「标签 + 控件」：标签右对齐占固定宽度，控件从控件列起、宽度锁死为控件列宽。
///
/// 有没有 info 图标都预留同样的位置，控件再被塞进宽度正好是控件列的格子里，无论里面
/// 放下拉框、滑块还是数字框，左右边缘都落在同一条竖线上——调用点不用再各自 add_sized。
pub fn row<R>(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    help: Option<&str>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        label_cell(ui, p, label, help);
        ctrl_cell(ui, add)
    })
    .inner
}

/// 与 [`row`] 相同，但控件右边再挂一句说明：说明从固定的说明列起排，
/// 于是整张卡片里所有说明文字的左边缘也是一条线。
pub fn row_hint<R>(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    help: Option<&str>,
    hint: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        label_cell(ui, p, label, help);
        let value = ctrl_cell(ui, add);
        hint_cell(ui, p, hint);
        value
    })
    .inner
}

/// 多行控件用的行：标签仍在左列，右侧格子的宽度同样是控件列，高度由内容决定。
///
/// 档位表、列表这类「一格里有好几行」的内容走这个，标签列与单行控件同样对齐。
pub fn block<R>(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    help: Option<&str>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal_top(|ui| {
        label_cell(ui, p, label, help);
        cell_auto_height(
            ui,
            ctrl_width(),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                add(ui)
            },
        )
    })
    .inner
}

/// 没有标签的一行：内容缩进到控件列，左右边缘与其它带标签的行对齐。
///
/// 档位条、「执行 / 复制命令」这类整排控件没有标签，若直接从卡片左边起排，
/// 就会比别处整整左移一个标签列，看着像单独一栏。
pub fn ctrl_column<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal_top(|ui| {
        ui.add_space(LABEL_W + ROW_GAP);
        cell_auto_height(
            ui,
            ctrl_width(),
            egui::Layout::left_to_right(egui::Align::Center),
            add,
        )
    })
    .inner
}

/// 控件格：宽度锁死控件列，一行里的控件从同一竖线起、到同一竖线止。
fn ctrl_cell<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    cell(
        ui,
        Vec2::new(ctrl_width(), CTRL_H),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ctrl_spacing(ui);
            add(ui)
        },
    )
}

/// 说明格：控件右边那句话，左边缘与其它行的说明对齐。
fn hint_cell(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    let width = ui.available_width().max(120.0);
    cell_auto_height(
        ui,
        width,
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add(
                egui::Label::new(RichText::new(text).size(12.0).color(p.text_weak))
                    .wrap()
                    .halign(egui::Align::Min),
            )
        },
    )
}

/// 把当前 `ui` 的控件尺寸统一到控件列宽 / [`CTRL_H`]。
///
/// 滑块要把「滑块长度 + 右侧数值框」凑成整列，所以这里给数值框留出 52 像素。
fn ctrl_spacing(ui: &mut egui::Ui) {
    ui.spacing_mut().combo_width = ctrl_width();
    ui.spacing_mut().slider_width = ctrl_width() - 52.0;
    ui.spacing_mut().interact_size.y = CTRL_H;
}

/// 单行输入框：宽度与行高都取控件列，和下拉框、滑块右边缘齐平。
pub fn text_field(ui: &mut egui::Ui, value: &mut String, hint: &str) -> Response {
    ui.add_sized(
        Vec2::new(ctrl_width(), CTRL_H),
        egui::TextEdit::singleline(value).hint_text(hint),
    )
}

/// 一行里并排两个控件：左边固定 [`PAIR_A`]，右边补到控件列剩下的宽度。
///
/// 「自动 + 数字框」「勾选框 + 数值」都用这个，右边缘因此不会比整宽控件短一截。
pub fn pair<A, B>(
    ui: &mut egui::Ui,
    left: impl FnOnce(&mut egui::Ui) -> A,
    right: impl FnOnce(&mut egui::Ui) -> B,
) -> (A, B) {
    let a = cell(
        ui,
        Vec2::new(PAIR_A, CTRL_H),
        egui::Layout::left_to_right(egui::Align::Center),
        left,
    );
    ui.add_space(PAIR_GAP);
    let b = cell(
        ui,
        Vec2::new(pair_b(), CTRL_H),
        egui::Layout::left_to_right(egui::Align::Center),
        right,
    );
    (a, b)
}

/// 等宽数字框：宽度取 [`pair_b`]，和 [`pair`] 右边的格子一样宽。
pub fn pair_num<Num: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut Num,
    range: std::ops::RangeInclusive<Num>,
) -> Response {
    ui.add_sized(
        Vec2::new(pair_b(), CTRL_H),
        egui::DragValue::new(value).range(range),
    )
}

/// 档位表格子：每档一个数字框，四格一行铺满控件列，第二行的格子与第一行严格对齐。
///
/// 之前用的是 `horizontal_wrapped` + 固定 74 像素的框，档数一变就换行位置不定，
/// 表里表外的框宽还各不相同。
pub fn slot_grid(ui: &mut egui::Ui, p: &Palette, values: &mut [u8], low: u8, high: u8) {
    const COLS: usize = 4;
    let gap = 6.0;
    let slot = (ctrl_width() - gap * (COLS as f32 - 1.0)) / COLS as f32;
    for chunk_start in (0..values.len()).step_by(COLS) {
        let chunk_end = (chunk_start + COLS).min(values.len());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (offset, value) in values[chunk_start..chunk_end].iter_mut().enumerate() {
                let index = chunk_start + offset + 1;
                let resp = ui.add_sized(
                    Vec2::new(slot, CTRL_H),
                    egui::DragValue::new(value)
                        .range(low..=high)
                        .custom_formatter(|v, _| format!("0x{:02x}", v as u8)),
                );
                tip(
                    resp,
                    format!("第 {index} 档的命令字，取值 {low:#04x}..={high:#04x}"),
                );
            }
        });
    }
    let _ = p;
}

/// 一排按钮：每颗都正好 [`BTN_W`] 宽、从当前行首依次排开，**永远不缩**。
///
/// 之前按「总数平分」算宽度，于是同一页里出现 123 / 104 / 91 三种按钮宽——一排三颗
/// 的被压窄、一排两颗的又被拉宽，整页看上去宽度乱七八糟。这里定死一颗按钮的尺寸：
/// 同一行几颗就几颗排过去，多出的宽度不够就让它往右伸，宁可伸进空白，也不改按钮宽。
pub fn actions(ui: &mut egui::Ui, p: &Palette, items: &[(&str, bool, &str)]) -> Option<usize> {
    let gap = ui.spacing().item_spacing.x;
    // 一排按钮的总宽不能超过控件列：四颗 104 的按钮加上间距是 446，比控件列多出几像素，
    // 最后一颗就会顶出卡片右边。按钮多于「控件列放得下」时按控件列等分，右边缘永远齐平。
    let count = items.len().max(1) as f32;
    let btn_w = if count > 1.0 {
        ((ctrl_width() - gap * (count - 1.0)) / count).min(BTN_W)
    } else {
        BTN_W
    };
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (index, (text, primary_style, help)) in items.iter().enumerate() {
            let button = if *primary_style {
                egui::Button::new(RichText::new(*text).color(Color32::WHITE).strong())
                    .fill(p.accent)
                    .stroke(Stroke::new(0.0, Color32::TRANSPARENT))
                    .corner_radius(CornerRadius::same(WIDGET_RADIUS))
                    .min_size(Vec2::new(btn_w, CTRL_H))
            } else {
                egui::Button::new(RichText::new(*text).color(p.text))
                    .fill(p.sunk)
                    .stroke(Stroke::new(1.0, p.stroke))
                    .corner_radius(CornerRadius::same(WIDGET_RADIUS))
                    .min_size(Vec2::new(btn_w, CTRL_H))
            };
            // 用 add_sized 给死宽高：`min_size` 只是下限，文字一长按钮就自己长胖，
            // 一排里几颗按钮于是宽窄不一、还把右边那颗顶出控件列。
            let resp = ui.add_sized(Vec2::new(btn_w, CTRL_H), button);
            let resp = if help.is_empty() {
                resp
            } else {
                tip(resp, *help)
            };
            if resp.clicked() && clicked.is_none() {
                clicked = Some(index);
            }
        }
    });
    clicked
}

/// 「标签 + 一排按钮」：按钮组左边缘与下拉框、输入框同一条竖线起。
///
/// 这一行是整页唯一允许往右伸过控件列的布局——三颗按钮就是 3×[`BTN_W`] 加两道间距，
/// 硬塞进控件列只能靠压窄按钮，那正是界面看着不齐的根源。
pub fn action_row(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    help: Option<&str>,
    items: &[(&str, bool, &str)],
) -> Option<usize> {
    ui.horizontal(|ui| {
        label_cell(ui, p, label, help);
        ctrl_spacing(ui);
        actions(ui, p, items)
    })
    .inner
}

/// 只出标签、不带控件：给那些自带 `ui.horizontal(|ui| { label; combo })` 的老布局就地补对齐。
///
/// 标签本身也挂悬浮说明——这类行没有可以放 `i` 的地方，鼠标停在标签上一样能看解释。
pub fn label_cell(ui: &mut egui::Ui, p: &Palette, text: &str, help: Option<&str>) {
    // 左对齐四线网格：图标槽位 / 标签文字 / 控件左缘 / 控件右缘，整页各成一条竖线。
    // 标签右对齐会让左缘随字数参差，看上去像「一堆宽度不一样的东西」，所以改成左对齐、
    // 说明图标统一放最左侧的图标槽里，没有说明的行也留同样宽的槽。
    cell(
        ui,
        Vec2::new(LABEL_W, CTRL_H),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            match help {
                Some(h) => {
                    info_dot(ui, p, h);
                }
                None => {
                    ui.add_space(16.0);
                }
            }
            let resp = ui.label(RichText::new(text).size(13.0).color(p.text_weak));
            if let Some(h) = help {
                tip(resp, h);
            }
        },
    );
}

/// 复选框行：勾选框挪到控件列，和下拉框、输入框对齐，不再各自缩进。
pub fn check_row(
    ui: &mut egui::Ui,
    p: &Palette,
    checked: &mut bool,
    label: &str,
    help: Option<&str>,
) -> Response {
    row(ui, p, label, help, |ui| {
        let mut resp = ui.checkbox(checked, "");
        if let Some(text) = help {
            resp = tip(resp, text);
        }
        resp
    })
}

/// 只读行：左边标签、右边值。值过长先截断，悬浮看全文（路径类基本都超宽）。
pub fn read_row(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    value: &str,
    help: Option<&str>,
) -> Response {
    let text = RichText::new(value).size(12.5).color(p.text_weak);
    row(ui, p, label, help, |ui| {
        let resp = ui.add(egui::Label::new(text).truncate());
        tip(resp, value.to_owned())
    })
}

/// 说明文字：缩进到控件列，和上面的控件左缘对齐，不再贴着卡片左边堆成一片。
pub fn note(ui: &mut egui::Ui, p: &Palette, text: impl Into<String>) {
    let offset = LABEL_W + ui.spacing().item_spacing.x;
    ui.horizontal_top(|ui| {
        ui.add_space(offset);
        ui.vertical(|ui| {
            ui.add(
                egui::Label::new(RichText::new(text.into()).size(12.0).color(p.text_weak))
                    .wrap()
                    .halign(egui::Align::Min),
            );
        });
    });
}
