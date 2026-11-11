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
    /// 输入框、下拉框、次级按钮的底色。
    ///
    /// 和 `sunk` 分开是有原因的：凹陷色是「往里凹」的灰面，用在白卡片上的输入框会
    /// 变成一块块灰方块；输入框要的是「白底 + 一圈细线」，所以浅色下它等于卡片色，
    /// 深色下比卡片亮一档。
    pub field: Color32,
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

/// 深色：近黑的中性灰，强调色是偏冷的蓝，长时间看不刺眼。
pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(11, 13, 17),
    panel: Color32::from_rgb(16, 19, 25),
    card: Color32::from_rgb(21, 24, 31),
    sunk: Color32::from_rgb(30, 34, 43),
    field: Color32::from_rgb(27, 31, 39),
    stroke: Color32::from_rgb(38, 43, 53),
    text: Color32::from_rgb(232, 235, 240),
    text_weak: Color32::from_rgb(154, 163, 178),
    accent: Color32::from_rgb(124, 156, 255),
    // 17% 透明度的强调色，已按预乘写好（egui 用预乘 alpha）
    accent_soft: Color32::from_rgba_premultiplied(21, 26, 44, 44),
    ok: Color32::from_rgb(74, 202, 138),
    warn: Color32::from_rgb(233, 176, 78),
    bad: Color32::from_rgb(240, 110, 110),
};

/// 浅色：白卡片浮在中性浅灰底上，强调色用偏深的蓝，保证在浅底上也有足够对比。
pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(244, 245, 247),
    panel: Color32::from_rgb(255, 255, 255),
    card: Color32::from_rgb(255, 255, 255),
    sunk: Color32::from_rgb(241, 243, 246),
    field: Color32::from_rgb(255, 255, 255),
    stroke: Color32::from_rgb(227, 229, 233),
    text: Color32::from_rgb(22, 24, 29),
    text_weak: Color32::from_rgb(107, 114, 128),
    accent: Color32::from_rgb(37, 99, 235),
    accent_soft: Color32::from_rgba_premultiplied(4, 11, 26, 28),
    ok: Color32::from_rgb(18, 133, 90),
    warn: Color32::from_rgb(180, 83, 9),
    bad: Color32::from_rgb(220, 38, 38),
};

/// 卡片圆角，比控件大一档，视觉上才能把里面的控件「装」住。
const CARD_RADIUS: u8 = 16;
/// 控件圆角。
const WIDGET_RADIUS: u8 = 8;

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

    // 字号：正文 14，说明文字压到 12，标题 19 拉开层级
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(19.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        ),
    ]
    .into();

    // 间距：默认值偏挤，卡片式布局要留出呼吸感
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(16.0, 8.0);
    style.spacing.interact_size = Vec2::new(44.0, 28.0);
    style.spacing.slider_width = 190.0;
    style.spacing.combo_width = 220.0;
    style.spacing.indent = 22.0;
    style.spacing.menu_margin = Margin::same(8);
    // 滚动条：macOS 那种浮起的窄条——平时几乎看不见，鼠标进到滚动区才浮出来，
    // 且不占内容宽度。非浮动模式下滚动条会从卡片右边啃掉 10px，卡片宽度跟着抖。
    style.spacing.scroll = egui::style::ScrollStyle::floating();
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating_width = 6.0;
    style.spacing.scroll.floating_allocated_width = 0.0;
    style.spacing.scroll.foreground_color = true;
    style.spacing.scroll.dormant_handle_opacity = 0.0;
    style.spacing.scroll.active_handle_opacity = 0.35;
    style.spacing.scroll.interact_handle_opacity = 0.7;
    style.spacing.scroll.dormant_background_opacity = 0.0;
    style.spacing.scroll.active_background_opacity = 0.0;
    style.spacing.scroll.interact_background_opacity = 0.15;

    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.sunk;
    v.hyperlink_color = p.accent;
    v.weak_text_color = Some(p.text_weak);
    v.override_text_color = Some(p.text);
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(12);
    v.window_stroke = Stroke::new(1.0, p.stroke);
    v.window_shadow = egui::epaint::Shadow {
        offset: [0, 12],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 120 } else { 36 }),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 20,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 100 } else { 28 }),
    };
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);

    // 控件四态：静默=白底细描边（输入框该有的样子），悬停=描边转强调色，
    // 按下/展开=强调色描边 + 淡底。灰底只留给凹陷区，不再拿来当输入框底色。
    let widgets = &mut v.widgets;
    widgets.noninteractive.bg_fill = p.card;
    widgets.noninteractive.weak_bg_fill = p.card;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.stroke);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    widgets.noninteractive.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.inactive.bg_fill = p.field;
    widgets.inactive.weak_bg_fill = p.field;
    widgets.inactive.bg_stroke = Stroke::new(1.0, p.stroke);
    widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    widgets.inactive.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.hovered.bg_fill = p.field;
    widgets.hovered.weak_bg_fill = p.field;
    widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent.gamma_multiply(0.7));
    widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    widgets.hovered.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.active.bg_fill = p.accent_soft;
    widgets.active.weak_bg_fill = p.accent_soft;
    widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    widgets.active.fg_stroke = Stroke::new(1.0, p.accent);
    widgets.active.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.open.bg_fill = p.field;
    widgets.open.weak_bg_fill = p.field;
    widgets.open.bg_stroke = Stroke::new(1.0, p.accent.gamma_multiply(0.7));
    widgets.open.fg_stroke = Stroke::new(1.0, p.text);
    widgets.open.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    style.visuals = v;
    style
}

/// 一张卡片：大圆角、宽留白，浅色下靠投影和底色分层，深色下再补一圈极淡描边。
///
/// 浅色卡片**不画描边**：白卡 + 1px 灰线是上一版的观感问题来源——线在缩放和高分屏上
/// 一半糊一半锐，看着像表格线；改用一层柔和大半径投影（`blur 18`）后边界由明度差交代，
/// 干净得多。深色底上投影几乎不可见，所以那一套保留 1px 描边来托边界。
pub fn card<R>(ui: &mut egui::Ui, p: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let dark = p.bg.to_array()[0] < 128;
    let frame = egui::Frame::new()
        .fill(p.card)
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin {
            left: 20,
            right: 20,
            top: 18,
            bottom: 18,
        })
        .shadow(egui::epaint::Shadow {
            offset: [0, 2],
            blur: if dark { 12 } else { 22 },
            spread: 0,
            color: Color32::from_black_alpha(if dark { 90 } else { 26 }),
        });
    let frame = if dark {
        frame.stroke(Stroke::new(1.0, p.stroke))
    } else {
        frame.stroke(Stroke::new(0.0, Color32::TRANSPARENT))
    };
    frame
        .show(ui, |ui| {
            // 卡片宽度一律吃满内容列，不按内容收缩：`Frame` 默认是「有多大内容就多宽」，
            // 于是「机型与布局」这种窄卡片右边缘停在 1030，「命令行」那种宽内容撑到
            // 1159，一页里几个圆角矩形右边缘参差不齐。宽度交给页面统一规定。
            let w = ui.available_width();
            if w.is_finite() {
                ui.set_width(w);
            }
            // 卡片里不缩进：折叠头、列表这些默认会按 indent 往右推，一推控件列就跟着
            // 偏移，同一份表单在「折叠卡片」和「带标题卡片」里就错开十几像素。
            ui.spacing_mut().indent = 0.0;
            add(ui)
        })
        .inner
}

/// 带标题的卡片。
pub fn section<R>(
    ui: &mut egui::Ui,
    p: &Palette,
    title: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    card(ui, p, |ui| {
        title_row(ui, p, title);
        ui.add_space(4.0);
        add(ui)
    })
}

/// 卡片标题：一行粗体标题，下面一条**横贯整张卡片**的细线。
///
/// 上一版是「标题 + 拖到右缘的尾线」，十几个卡片排下来，右侧全是长短不一的横线，
/// 反而显得琐碎。改成标题独立成行、分隔线走满卡片宽度（左右各伸出内边距，
/// 直接贴到卡片边缘），层次更像系统设置面板：标题区与内容区一刀切开，读者不会
/// 把标题误当成第一行内容。
pub fn title_row(ui: &mut egui::Ui, p: &Palette, title: &str) {
    ui.label(RichText::new(title).size(14.5).strong().color(p.text));
    let r = ui.max_rect();
    let y = ui.cursor().top() - ui.spacing().item_spacing.y * 0.5;
    ui.painter().hline(
        egui::Rangef::new(r.left() - CARD_PAD_X, r.right() + CARD_PAD_X),
        y,
        Stroke::new(1.0, p.stroke),
    );
}

/// 悬停时在控件上盖一层极淡的中性色。
///
/// 按钮的底色是定制的（白底/实心强调色），egui 的 hovered 样式不会生效，而「整块换色」
/// 会让一排按钮的明度互相跳。盖一层几乎看不见的浮色，鼠标扫过去有反应、静止时又不变。
fn wash(ui: &egui::Ui, resp: &Response, p: &Palette, radius: CornerRadius) {
    if !resp.hovered() {
        return;
    }
    let dark = p.bg.to_array()[0] < 128;
    let tint = if dark {
        Color32::from_white_alpha(12)
    } else {
        Color32::from_black_alpha(8)
    };
    ui.painter().rect_filled(resp.rect, radius, tint);
}

/// 主按钮：实心强调色，用于「点亮」这类主动作。
pub fn primary(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    // 两套调色板的强调色都足够深，白字始终是安全的
    let resp = ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
            .fill(p.accent)
            .stroke(Stroke::new(0.0, Color32::TRANSPARENT))
            .corner_radius(CornerRadius::same(WIDGET_RADIUS))
            .min_size(Vec2::new(BTN_W, CTRL_H)),
    );
    wash(ui, &resp, p, CornerRadius::same(WIDGET_RADIUS));
    resp
}

/// 次级按钮：白底 + 细描边，和主按钮并排时不抢注意力。
///
/// 底色从「凹陷灰」改成「输入框同款的白底」，一排按钮旁边就是下拉框和输入框，
/// 底色一致之后它们看起来才是一套控件，而不是两种材质。
pub fn secondary(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    let resp = ui.add(
        egui::Button::new(RichText::new(text).color(p.text))
            .fill(p.field)
            .stroke(Stroke::new(1.0, p.stroke))
            .corner_radius(CornerRadius::same(WIDGET_RADIUS))
            .min_size(Vec2::new(BTN_W, CTRL_H)),
    );
    wash(ui, &resp, p, CornerRadius::same(WIDGET_RADIUS));
    resp
}

/// 开关：自绘的 34×20 胶囊，开时滑到右侧并染成强调色。
///
/// 复选框那一格小白框在卡片式界面里存在感太弱，「开 / 关」这类二元项用开关更符合
/// 现在的阅读习惯；顺带省掉了勾选框里那圈看不清的描边。
pub fn switch(ui: &mut egui::Ui, p: &Palette, on: &mut bool) -> Response {
    let size = Vec2::new(36.0, 20.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    // 位置按时间插值：点击瞬间滑块滑过去，而不是硬跳
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let track = if *on {
        p.accent.gamma_multiply(0.35 + 0.65 * t)
    } else {
        p.stroke
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same((size.y / 2.0) as u8), track);
    let pad = 3.0;
    let travel = size.x - size.y;
    let knob_x = rect.left() + pad + travel * t;
    let knob = egui::Rect::from_center_size(
        egui::pos2(knob_x + size.y / 2.0 - pad, rect.center().y),
        Vec2::splat(size.y - pad * 2.0),
    );
    ui.painter().rect_filled(
        knob,
        CornerRadius::same((knob.height() / 2.0) as u8),
        Color32::WHITE,
    );
    resp
}

/// 状态胶囊：圆角小标签，用来显示「真实硬件 / 模拟后端」这类一眼要看到的状态。
pub fn pill(ui: &mut egui::Ui, text: &str, color: Color32) -> Response {
    let font = FontId::proportional(11.5);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let size = Vec2::new(galley.size().x + 18.0, galley.size().y + 8.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let bg = Color32::from_rgba_premultiplied(
        (color.r() as u16 * 32 / 255) as u8,
        (color.g() as u16 * 32 / 255) as u8,
        (color.b() as u16 * 32 / 255) as u8,
        32,
    );
    ui.painter().rect(
        rect,
        CornerRadius::same((size.y / 2.0) as u8),
        bg,
        Stroke::new(1.0, color.gamma_multiply(0.35)),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    resp
}

/// 页签：文字 + 底部指示条，整条基线横贯控件区。
///
/// 上一版是 iOS 那种分段胶囊：一条灰轨道，选中格用实心强调色托起。功能没错，
/// 但深色实心块在桌面工具里太抢眼——一屏十几处强调色之后，页签变成最重的一块，
/// 反而压过内容。改成下划线页签：未选中只有灰字，当前页靠加粗的黑字 + 2px 强调色
/// 短条表达，视线重心回到内容上。
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    p: &Palette,
    id: &str,
    current: T,
    items: &[(T, &str, &str)],
) -> Option<T> {
    let font = FontId::proportional(13.5);
    let pad = Vec2::new(14.0, 8.0);
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
    let total = ui.available_width();
    let height = pad.y * 2.0 + 18.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, height), Sense::hover());

    // 基线：一条压在所有页签下面的细线，让「一排页签」自己成为一条控件，
    // 而不是几个散落的文字。
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, p.stroke),
    );

    let mut picked = None;
    let mut x = rect.left();
    for ((value, text, help), width) in items.iter().zip(&widths) {
        let item =
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(*width, height - 1.0));
        let selected = *value == current;
        let resp = ui.interact(item, ui.scope_id().with((id, text)), Sense::click());
        if resp.hovered() && !selected {
            ui.painter().rect_filled(
                item.shrink2(Vec2::new(4.0, 6.0)),
                CornerRadius::same(WIDGET_RADIUS),
                p.stroke.gamma_multiply(0.5),
            );
        }
        let color = match (selected, resp.hovered()) {
            (true, _) => p.text,
            (false, true) => p.text,
            (false, false) => p.text_weak,
        };
        let label = ui
            .painter()
            .layout_no_wrap((*text).to_owned(), font.clone(), color);
        let center = item.center();
        ui.painter().galley(
            egui::pos2(
                center.x - label.size().x / 2.0,
                center.y - label.size().y / 2.0,
            ),
            label.clone(),
            color,
        );
        if selected {
            // 指示条只覆盖文字宽度，两端留出内边距，看起来像「贴在词下面」而不是「铺满格子」
            let bar_w = (label.size().x + 8.0).min(item.width() - 8.0);
            let bar = egui::Rect::from_min_size(
                egui::pos2(center.x - bar_w / 2.0, item.bottom() - 3.0),
                Vec2::new(bar_w, 2.5),
            );
            ui.painter()
                .rect_filled(bar, CornerRadius::same(2), p.accent);
        }
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
pub const CARD_PAD_X: f32 = 20.0;

/// 标签列与控件列之间的间距，与全局 `item_spacing.x` 取同一个值。
pub const ROW_GAP: f32 = 12.0;

/// 控件列宽度下限：窄窗口下也不能窄到把下拉框里的档位名切掉。
pub const CTRL_MIN_W: f32 = 280.0;

/// 控件行高。输入框、下拉框、数字框、按钮都取这个高度，一行里高矮一致。
pub const CTRL_H: f32 = 32.0;

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
    // 控件列 = 卡片内宽 - 标签列 - 列间距，正好吃掉卡片右侧那段内边距：
    // 输入框、下拉框、档位条、按钮组右端于是都压在卡片右边这条竖线上。
    let content = ui.available_width() - CARD_PAD_X * 2.0;
    let width = (content - LABEL_W - ROW_GAP).max(CTRL_MIN_W);
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
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
    let hot = resp.hovered();
    // 常态是一枚安静的细描边圆圈：整页几十个 i 图标全都染色会变成一堆蓝色噪点，
    // 只有鼠标停上去才转成强调色。
    let line = if hot { p.accent } else { p.stroke };
    ui.painter().circle(
        rect.center(),
        6.5,
        Color32::TRANSPARENT,
        Stroke::new(1.0, line),
    );
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 0.5),
        egui::Align2::CENTER_CENTER,
        "i",
        FontId::proportional(9.5),
        if hot { p.accent } else { p.text_weak },
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
                    .fill(p.field)
                    .stroke(Stroke::new(1.0, p.stroke))
                    .corner_radius(CornerRadius::same(WIDGET_RADIUS))
                    .min_size(Vec2::new(btn_w, CTRL_H))
            };
            // 用 add_sized 给死宽高：`min_size` 只是下限，文字一长按钮就自己长胖，
            // 一排里几颗按钮于是宽窄不一、还把右边那颗顶出控件列。
            let resp = ui.add_sized(Vec2::new(btn_w, CTRL_H), button);
            wash(ui, &resp, p, CornerRadius::same(WIDGET_RADIUS));
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

/// 开关行：开关挪到控件列，和下拉框、输入框对齐，不再各自缩进。
pub fn check_row(
    ui: &mut egui::Ui,
    p: &Palette,
    checked: &mut bool,
    label: &str,
    help: Option<&str>,
) -> Response {
    row(ui, p, label, help, |ui| {
        let mut resp = switch(ui, p, checked);
        if let Some(text) = help {
            resp = tip(resp, text);
        }
        resp
    })
}

/// 只读行：左边标签、右边值。值右对齐到控件列右缘——一串路径、一个版本号，右对齐之后
/// 上下几行能对齐成一条线，读起来像一张表；过长先截断，悬浮看全文。
pub fn read_row(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    value: &str,
    help: Option<&str>,
) -> Response {
    let text = RichText::new(value)
        .size(12.0)
        .monospace()
        .color(p.text_weak);
    row(ui, p, label, help, |ui| {
        let resp = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(text).truncate())
        });
        tip(resp.inner, value.to_owned())
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
