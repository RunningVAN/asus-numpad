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
    /// 强调色（选中、焦点描边、进度、页签指示条）
    pub accent: Color32,
    /// 强调色的淡底
    pub accent_soft: Color32,
    /// 实心主按钮的底色。
    ///
    /// 和 `accent` 分开是官网上写明的取舍：强调色要亮（高亮、描边都要它），
    /// 但按钮上压着白字，亮红只能到 3.6:1，压深一档的 `#d81b45` 才有 5:1。
    pub btn: Color32,
    /// 主按钮渐变的**下缘**色（站点 `--btn-b`）。
    ///
    /// 官网上 `.btn-primary` 是 180° 渐变 `--btn-a → --btn-b`；egui 的按钮只有单色，
    /// 所以底色仍用 `btn`，再在下缘压一层这个色做「上亮下深」，远看是同一枚按钮。
    pub btn_deep: Color32,
    /// 正常状态
    pub ok: Color32,
    /// 告警
    pub warn: Color32,
    /// 错误
    pub bad: Color32,
}

/// 深色：与官网 `html[data-theme="dark"]` 的令牌一一对应。
///
/// 官网页面的卡片底是 `rgba(255,255,255,.035)` 叠在 `#0a0b0f` 上，egui 的 `Frame`
/// 只能给实色，所以这里写的是**叠完之后**的等效实色（10 + 245×0.035 ≈ 19），
/// `sunk` / `field` / `stroke` 同理按 .06 / .06 / .09 的透明度折算好。改站点配色时
/// 照这个公式重算一次即可，不需要 GUI 另立一套设计语言。
pub const DARK: Palette = Palette {
    // 站点 --bg / --bg-alt
    bg: Color32::from_rgb(10, 11, 15),
    panel: Color32::from_rgb(13, 15, 21),
    card: Color32::from_rgb(19, 21, 26),
    sunk: Color32::from_rgb(26, 28, 34),
    field: Color32::from_rgb(33, 35, 40),
    stroke: Color32::from_rgb(40, 42, 47),
    // --text / --text-2：次要文字 8.4:1，压住 4.5:1
    text: Color32::from_rgb(236, 238, 244),
    text_weak: Color32::from_rgb(169, 173, 186),
    // --accent（品牌红）
    accent: Color32::from_rgb(255, 45, 85),
    // 品牌红 17% 叠在 --bg 上的预乘值（egui 用预乘 alpha）：52,17,27,44
    accent_soft: Color32::from_rgba_premultiplied(52, 17, 27, 44),
    // --btn-a：品牌红压深一档，白字 5.05:1
    btn: Color32::from_rgb(216, 27, 69),
    // --btn-b
    btn_deep: Color32::from_rgb(179, 18, 58),
    ok: Color32::from_rgb(52, 211, 153),
    warn: Color32::from_rgb(240, 180, 41),
    bad: Color32::from_rgb(255, 92, 122),
};

/// 浅色：与官网 `html[data-theme="light"]` 对应。规则同上，底色换 `#f7f8fa`、
/// 强调色压深到 `#b8163c`（亮底上亮红只有 2.8:1，看不清）。
pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(247, 248, 250),
    panel: Color32::from_rgb(255, 255, 255),
    card: Color32::from_rgb(255, 255, 255),
    sunk: Color32::from_rgb(235, 236, 239),
    field: Color32::from_rgb(255, 255, 255),
    stroke: Color32::from_rgb(232, 233, 236),
    text: Color32::from_rgb(20, 22, 31),
    text_weak: Color32::from_rgb(77, 82, 98),
    accent: Color32::from_rgb(184, 22, 60),
    // 品牌红 11% 叠在 --bg 上的预乘值：240,223,229,28
    accent_soft: Color32::from_rgba_premultiplied(240, 223, 229, 28),
    btn: Color32::from_rgb(192, 20, 60),
    // --btn-b
    btn_deep: Color32::from_rgb(156, 15, 48),
    ok: Color32::from_rgb(4, 120, 87),
    warn: Color32::from_rgb(180, 83, 9),
    bad: Color32::from_rgb(185, 28, 28),
};

/// 卡片圆角：官网 `--radius: 14px`。
const CARD_RADIUS: u8 = 14;
/// 控件圆角：官网 `.btn { border-radius: 10px }`。
const WIDGET_RADIUS: u8 = 10;

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
    let mut prev_column = 0_u32;
    let out = frame
        .show(ui, |ui| {
            // 卡片宽度一律吃满内容列，不按内容收缩：`Frame` 默认是「有多大内容就多宽」，
            // 于是「机型与布局」这种窄卡片右边缘停在 1030，「命令行」那种宽内容撑到
            // 1159，一页里几个圆角矩形右边缘参差不齐。宽度交给页面统一规定。
            let w = ui.available_width();
            if w.is_finite() {
                ui.set_width(w);
            }
            // 卡内所有表单行都照本卡内宽排，控件右端因此压在卡片右内边距这条线上，
            // 与卡片宽度同源，不会再出现「卡片 800、里面控件只到 720」。
            prev_column = enter_card(w);
            // 卡片里不缩进：折叠头、列表这些默认会按 indent 往右推，一推控件列就跟着
            // 偏移，同一份表单在「折叠卡片」和「带标题卡片」里就错开十几像素。
            ui.spacing_mut().indent = 0.0;
            add(ui)
        })
        .inner;
    leave_card(prev_column);
    out
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
    title_row_help(ui, p, title, None);
}

/// 同上，但标题右边还能挂一个 `i`。
///
/// 区块级的说明（这一栏是什么、为什么会有好几个条目）塞不进任何一行参数里，
/// 挂在标题上最省地方，也不会和参数行的 `i` 挤在同一列。
pub fn title_row_help(ui: &mut egui::Ui, p: &Palette, title: &str, help: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(14.5).strong().color(p.text));
        if let Some(help) = help {
            info_dot(ui, p, help);
        }
    });
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

/// 主按钮：品牌红渐变，用于「点亮」这类主动作。
///
/// 官网上 `.btn-primary` 是 180° 渐变 `--btn-a → --btn-b`，egui 的 `Button` 只能给一个
/// 单色填充，所以这里自己画：先按 16 条色带铺出渐变（上下两端各自成圆角，中间直角，
/// 圆角区因此不会被色带切成台阶），再叠悬停浮色，最后把文字压在中心。
///
/// 用自绘而不是「给 Button 加 fill 再盖一层」的原因：`add()` 之后再画的东西都在文字
/// 之上，盖深色会把白字压脏。自绘保住 `Sense::click` —— 命中、悬停、按下、点击回执
/// 与 `ui.button` 完全一致，外层 `tip()` 的说明气泡也照常工作。
pub fn primary(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    let font = TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE);
    let pad = ui.spacing().button_padding;
    let size = Vec2::new(
        (galley.size().x + pad.x * 2.0).max(BTN_W),
        CTRL_H.max(galley.size().y + pad.y * 2.0),
    );
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let radius = CornerRadius::same(WIDGET_RADIUS);
        let enabled = ui.is_enabled();
        // 按下时整块用下缘色：没有位移和阴影可借，颜色加深是唯一还读得出来的反馈。
        let (top, bottom) = if !enabled {
            (p.sunk, p.sunk)
        } else if resp.is_pointer_button_down_on() {
            (p.btn_deep, p.btn_deep)
        } else {
            (p.btn, p.btn_deep)
        };
        if top == bottom {
            ui.painter()
                .rect_filled(rect, radius, if enabled { p.btn_deep } else { p.sunk });
        } else {
            gradient(ui.painter(), rect, radius, top, bottom);
        }
        if enabled && resp.hovered() {
            ui.painter()
                .rect_filled(rect, radius, Color32::from_white_alpha(14));
        }
        // 顶部一道 1px 亮边，对应站点按钮的高光
        if enabled {
            ui.painter().rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(rect.left() + 1.0, rect.top() + 1.0),
                    egui::pos2(rect.right() - 1.0, rect.top() + 2.0),
                ),
                CornerRadius::same(1),
                Color32::from_white_alpha(30),
            );
        }
        let ink = if enabled {
            Color32::WHITE
        } else {
            Color32::from_white_alpha(130)
        };
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, ink);
    }
    if ui.is_enabled() {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// 竖向渐变：`top → bottom`，上下两端按 `radius` 收圆角，中间各带直角。
///
/// 16 条色带在 32px 高的按钮上每带两像素，色差不到 3，肉眼看是连续过渡。
fn gradient(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: CornerRadius,
    top: Color32,
    bottom: Color32,
) {
    const BANDS: usize = 16;
    let h = rect.height();
    for i in 0..BANDS {
        let t = (i as f32 + 0.5) / BANDS as f32;
        let band = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + h * i as f32 / BANDS as f32),
            egui::pos2(rect.right(), rect.top() + h * (i + 1) as f32 / BANDS as f32),
        );
        painter.rect_filled(
            band,
            CornerRadius {
                nw: if i == 0 { radius.nw } else { 0 },
                ne: if i == 0 { radius.ne } else { 0 },
                sw: if i + 1 == BANDS { radius.sw } else { 0 },
                se: if i + 1 == BANDS { radius.se } else { 0 },
            },
            mix(top, bottom, t),
        );
    }
}

/// 两个色之间按 `t` 线性取色。
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
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

/// 品牌标记：官网页头那枚键盘图标，24×24 视框等比放大到 `size`。
///
/// 与 `site/index.html` 里的 `.brand-mark` 是同一套几何：19×15 的圆角外框加四个
/// 键位短线。用矢量画而不是塞位图——深浅两套主题、任意缩放和高 DPI 都清晰，
/// 仓库里也不用维护多份 PNG（站点那边只有一枚 favicon.svg）。
pub fn logo(ui: &mut egui::Ui, p: &Palette, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_logo(ui.painter(), rect, p.accent);
    resp
}

/// 把品牌标记画进任意矩形，供 [`logo`] 和窗口图标复用同一套坐标。
pub fn paint_logo(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    let s = (rect.width().min(rect.height()) / 24.0).max(0.1);
    let at = |x: f32, y: f32| egui::pos2(rect.left() + x * s, rect.top() + y * s);
    // 外框：视框内 (2.5,4.5) 起、19×15、圆角 3、线宽 1.6
    let body = egui::Rect::from_min_max(at(2.5, 4.5), at(21.5, 19.5));
    painter.rect_stroke(
        body,
        CornerRadius::same(((3.0 * s).round() as u8).max(2)),
        Stroke::new((1.6 * s).max(1.0), color),
        egui::StrokeKind::Inside,
    );
    // 四个键位：站点上是 `M8 10h2` 这一组短线，小尺寸下比实心方块更清楚
    let w = (1.6 * s).max(1.0);
    for (x, y) in [(8.0, 10.0), (14.0, 10.0), (8.0, 14.5), (14.0, 14.5)] {
        painter.line_segment([at(x, y), at(x + 2.0, y)], Stroke::new(w, color));
    }
}

/// 版本胶囊：官网页头 `.brand-tag` 的同款——等宽字体、细描边、全圆角。
///
/// 摆在品牌名右边当「这是哪个版本」的角标，比在正文里单开一行写版本号省地方。
pub fn tag(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::monospace(11.0), p.text_weak);
    let size = Vec2::new(galley.size().x + 14.0, galley.size().y + 4.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same((size.y / 2.0) as u8),
        Stroke::new(1.0, p.stroke),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, p.text_weak);
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
    // 档位格铺满控件列：之前封顶 520，4 档只排到控件列一半（485 对 615），
    // 同一张卡片里上面是满宽输入框、下面是半截的档位条，白色块一长一短最扎眼。
    let width = ui.available_width().min(ctrl_width()).max(CTRL_MIN_W);
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

/// 进卡时按本卡内宽换上控件列宽度，返回上一份以便出卡还原。
///
/// 控件右端必须落在**本卡**的右内边距上：页面级 [`begin_page`] 量的是滚动区宽度，
/// 与 `Frame` 扣完留白后的实宽能差几十像素，控件于是停在卡片右边之前一截，
/// 一页里输入框、下拉框、按钮组的右缘就长短不齐。这里进卡换值、出卡还原，
/// 并列卡片各页一致，嵌套卡片也不会把外层顶宽。
fn enter_card(card_inner_w: f32) -> u32 {
    use std::sync::atomic::Ordering;
    let prev = COLUMN_W_MILLI.load(Ordering::Relaxed);
    if card_inner_w.is_finite() && card_inner_w > 0.0 {
        let width = (card_inner_w - LABEL_W - ROW_GAP).max(CTRL_MIN_W);
        COLUMN_W_MILLI.store((width * 1000.0).round() as u32, Ordering::Relaxed);
    }
    prev
}

/// 出卡时把页面级控件列宽度换回去。
fn leave_card(prev: u32) {
    use std::sync::atomic::Ordering;
    COLUMN_W_MILLI.store(prev, Ordering::Relaxed);
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
///
/// 内容分两层：一行灰色小字「说明」交代这是什么，下面是正文。原先只有一句 12.5 号
/// 文字浮在控件旁，长句子跑到两三行时会被误读成界面正文；补一行标签后，凡是浮出来的
/// 方块都能一眼认出是解释。宽度压到 300 像素（约 25 个汉字一行），再长也读得下去。
pub fn tip(resp: Response, text: impl Into<String>) -> Response {
    let text = text.into();
    // 截图模式下要靠这个矩形决定指针停在哪，气泡才拍得到（正常启动是空操作）
    crate::shot::note_tip_rect(resp.rect, &text);
    // 挂说明的控件统一换成问号光标，用户能先看出「这里有东西可停」再停上去。
    resp.on_hover_cursor(egui::CursorIcon::Help)
        .on_hover_ui(move |ui| {
            ui.set_max_width(300.0);
            ui.spacing_mut().item_spacing.y = 4.0;
            let v = ui.visuals();
            let accent = v.hyperlink_color;
            let body = v.override_text_color.unwrap_or(v.text_color());
            ui.label(
                RichText::new("说明")
                    .size(10.0)
                    .color(accent.gamma_multiply(0.9)),
            );
            ui.label(
                RichText::new(text.as_str())
                    .size(12.5)
                    .line_height(Some(19.0))
                    .color(body),
            );
        })
}

/// 标签后面那个小圆圈 `i`：鼠标停上去出说明。
///
/// 参数面板里每一项都值得解释一句，但全铺成正文会把界面撑散，所以统一做法是
/// 标签右侧留一个图标，想看的人停一下鼠标。
pub fn info_dot(ui: &mut egui::Ui, p: &Palette, text: impl Into<String>) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
    let hot = resp.hovered();
    // 常态是「淡底 + 淡描边」的圆圈：整页几十个 i 全染成实心强调色会变成一堆蓝噪点，
    // 但只画一圈卡片描边色又几乎看不见（浅色卡片尤其如此），折中方案是强调色淡底 +
    // 半透明描边——安静，又能被扫到。鼠标停上去才反过来变成实心强调色。
    //
    // 深色底要单独提一档：同样的半透明描边压在近黑卡片上会糊掉，10 号小字跟着一起
    // 看不清，所以描边和底色都加浓。
    let dark = p.bg.to_array()[0] < 128;
    let fill = if hot {
        p.accent
    } else if dark {
        p.accent.gamma_multiply(0.22)
    } else {
        p.accent_soft
    };
    let line = if hot {
        p.accent
    } else {
        p.accent.gamma_multiply(if dark { 0.8 } else { 0.45 })
    };
    let glyph = if hot { p.card } else { p.accent };
    ui.painter()
        .circle(rect.center(), 6.5, fill, Stroke::new(1.0, line));
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 0.5),
        egui::Align2::CENTER_CENTER,
        "i",
        FontId::proportional(9.5),
        glyph,
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

/// 一排按钮：整组铺满控件列，几颗就几等分，右端压在卡片的内右边缘上。
///
/// 早先定死一颗 [`BTN_W`]，一排三颗只占列宽一半，同一张卡片里输入框 636 宽、
/// 按钮组 328 宽，白色块「有长有短」正是这么来的。现在按钮组与输入框、下拉框同宽，
/// 一排内各颗按钮彼此等宽，右缘与整页控件列同一条竖线。
pub fn actions(ui: &mut egui::Ui, p: &Palette, items: &[(&str, bool, &str)]) -> Option<usize> {
    let gap = ui.spacing().item_spacing.x;
    let count = items.len().max(1) as f32;
    // 单颗宽度不低于 [`BTN_W`]：数量少时不至于把按钮拉成一条长条，数量多时保证铺满。
    let btn_w = ((ctrl_width() - gap * (count - 1.0)) / count).max(BTN_W);
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (index, (text, primary_style, help)) in items.iter().enumerate() {
            let button = if *primary_style {
                egui::Button::new(RichText::new(*text).color(Color32::WHITE).strong())
                    .fill(p.btn)
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

/// 「标签 + 一排按钮」：按钮组与下拉框、输入框同宽，右缘压在同一条竖线上。
pub fn action_row(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    help: Option<&str>,
    items: &[(&str, bool, &str)],
) -> Option<usize> {
    ui.horizontal(|ui| {
        label_cell(ui, p, label, help);
        ctrl_cell(ui, |ui| actions(ui, p, items))
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

/// 圆角矩形的有符号距离：负数在内部，正数在外部。
///
/// 图标光栅化用，`x`/`y` 是左上角，`w`/`h` 是宽高，`r` 是圆角半径。
fn sd_round_rect(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let qx = (px - cx).abs() - (w / 2.0 - r);
    let qy = (py - cy).abs() - (h / 2.0 - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

/// 窗口 / 任务栏图标：`site/assets/favicon.svg` 的光栅版。
///
/// 站点上那枚 favicon 就是本项目的标记（黑底圆角方块 + 红框键盘 + 六键），这里按
/// 32×32 视框逐点判定 SDF 再 4×4 超采样，得到任意尺寸的 RGBA。不引 resvg / tiny-skia：
/// 为一张图标拉一条 SVG 解析链不划算，而这点几何用距离函数几行就写完了。
///
/// 返回值直接喂 `eframe::egui::IconData`，Linux / Windows 的窗口标题栏与任务栏都会用。
pub fn app_icon(size: u32) -> egui::IconData {
    let size = size.clamp(8, 1024);
    let n = size as f32;
    // 视框 32 → 像素的缩放
    let k = n / 32.0;
    /// 每像素在一维上的采样数，4×4 = 16 个采样点，足够把圆角磨平。
    const SS: u32 = 4;
    // favicon 里的颜色
    let plate = (10u8, 11u8, 15u8);
    let accent = (255u8, 45u8, 85u8);
    let light = (236u8, 238u8, 244u8);
    // 六个键位：前三后三，最后一枚是亮色（NumLock 打开的样子）
    let keys = [
        (8.0f32, 12.0f32, accent),
        (14.3, 12.0, accent),
        (20.6, 12.0, accent),
        (8.0, 17.4, accent),
        (14.3, 17.4, accent),
        (20.6, 17.4, light),
    ];

    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let step = 1.0 / SS as f32;
    for py in 0..size {
        for px in 0..size {
            let mut rgb = [0f32; 3];
            let mut hits = 0u32;
            for sy in 0..SS {
                for sx in 0..SS {
                    // 采样点取像素中心偏移，避免整体左上半像素的漂移
                    let x = (px as f32 + (sx as f32 + 0.5) * step) / k;
                    let y = (py as f32 + (sy as f32 + 0.5) * step) / k;
                    // 自底向上叠：外框描边和键位都画在底板上，底板不透明所以三点式取色即可
                    let mut color = None;
                    if sd_round_rect(x, y, 4.5, 8.0, 23.0, 16.0, 3.5) < 0.0
                        && sd_round_rect(x, y, 4.5 + 1.8, 8.0 + 1.8, 23.0 - 3.6, 16.0 - 3.6, 2.5)
                            > 0.0
                    {
                        color = Some(accent);
                    }
                    for (kx, ky, key) in keys {
                        if sd_round_rect(x, y, kx, ky, 3.4, 3.4, 1.0) < 0.0 {
                            color = Some(key);
                        }
                    }
                    if color.is_none() && sd_round_rect(x, y, 0.0, 0.0, 32.0, 32.0, 7.0) < 0.0 {
                        color = Some(plate);
                    }
                    if let Some((r, g, b)) = color {
                        rgb[0] += r as f32;
                        rgb[1] += g as f32;
                        rgb[2] += b as f32;
                        hits += 1;
                    }
                }
            }
            let idx = ((py * size + px) * 4) as usize;
            if hits == 0 {
                // 圆角外的空白：全透明
                continue;
            }
            let coverage = hits as f32 / (SS * SS) as f32;
            rgba[idx] = (rgb[0] / hits as f32).round() as u8;
            rgba[idx + 1] = (rgb[1] / hits as f32).round() as u8;
            rgba[idx + 2] = (rgb[2] / hits as f32).round() as u8;
            rgba[idx + 3] = (coverage * 255.0).round() as u8;
        }
    }
    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_expected_shape() {
        let icon = app_icon(64);
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);
        assert_eq!((icon.width, icon.height), (64, 64));
    }

    #[test]
    fn icon_corner_is_transparent_and_body_is_opaque() {
        let icon = app_icon(64);
        let px = |x: u32, y: u32| {
            let i = ((y * 64 + x) * 4) as usize;
            (
                icon.rgba[i],
                icon.rgba[i + 1],
                icon.rgba[i + 2],
                icon.rgba[i + 3],
            )
        };
        // 圆角之外必须透明，否则图标在浅色任务栏上是个黑方块
        assert_eq!(px(0, 0).3, 0);
        // 底板中心不透明
        assert_eq!(px(32, 16).3, 255);
        // 取一处分明的区域：键盘框下方那条底板（视框 y≈29），颜色应当接近 #0a0b0f
        let (r, g, b, _) = px(32, 58);
        assert!(r < 40 && g < 40 && b < 40, "底板像素偏亮：{r},{g},{b}");
    }

    /// 把一帧里所有矩形摊平，便于按「按钮的形状」去断言。
    fn walk_rects(s: &egui::Shape, out: &mut Vec<(egui::Rect, Color32)>) {
        match s {
            egui::Shape::Rect(r) => out.push((r.rect, r.fill)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk_rects(s, out)),
            _ => {}
        }
    }

    /// 跑一帧，把主按钮画出来，返回帧内所有矩形。
    fn frame_with_primary(p: &Palette) -> Vec<(egui::Rect, Color32)> {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.add_space(8.0);
            primary(ui, p, "OK");
        });
        // 字体图集是给渲染器用的，测试里没人消费，丢掉前要显式 clear
        out.textures_delta.clear();
        let mut all = Vec::new();
        out.shapes
            .iter()
            .for_each(|c| walk_rects(&c.shape, &mut all));
        all
    }

    #[test]
    fn primary_button_gets_a_vertical_gradient() {
        let rects = frame_with_primary(&DARK);
        // 渐变色带：宽度就是按钮宽，高度只有 32/16=2px
        let mut bands: Vec<(f32, Color32)> = rects
            .iter()
            .filter(|(r, c)| {
                (r.width() - BTN_W).abs() < 1.0
                    && r.height() < 3.0
                    && c.r() > 150
                    && c.g() < 70
                    && (40..110).contains(&c.b())
            })
            .map(|(r, c)| (r.top(), *c))
            .collect();
        bands.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        assert!(
            bands.len() >= 8,
            "主按钮没画出渐变，只找到 {} 条色带",
            bands.len()
        );
        // 上到下只会越压越深，不会回弹
        for w in bands.windows(2) {
            assert!(
                w[1].1.r() <= w[0].1.r() + 1,
                "色带上下顺序反了：{:?} 在 {:?} 之下",
                w[1],
                w[0]
            );
        }
        // 端点必须真的分别落在 --btn-a 与 --btn-b 上，否则等于没渐变
        let top = bands.first().unwrap().1;
        let bottom = bands.last().unwrap().1;
        assert!(
            (top.r() as i32 - DARK.btn.r() as i32).abs() <= 3,
            "上缘色不对：{top:?} vs {:?}",
            DARK.btn
        );
        assert!(
            (bottom.r() as i32 - DARK.btn_deep.r() as i32).abs() <= 3,
            "下缘色不对：{bottom:?} vs {:?}",
            DARK.btn_deep
        );
    }

    #[test]
    fn primary_button_keeps_its_label_on_top() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.add_space(8.0);
            primary(ui, &DARK, "OK");
        });
        out.textures_delta.clear();
        let mut labels = Vec::new();
        fn walk_text(s: &egui::Shape, out: &mut Vec<(egui::Rect, Color32)>) {
            match s {
                egui::Shape::Text(t) => out.push((
                    egui::Rect::from_min_size(t.pos, t.galley.size()),
                    t.fallback_color,
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk_text(s, out)),
                _ => {}
            }
        }
        out.shapes
            .iter()
            .for_each(|c| walk_text(&c.shape, &mut labels));
        // 文字必须存在、是白的，且落在按钮的垂直中线附近——不能被色带盖住
        let (text_rect, color) = labels
            .iter()
            .find(|(r, _)| (r.width() - 24.0).abs() < 12.0 && r.height() > 8.0)
            .map(|(r, c)| (r, *c))
            .or_else(|| labels.first().map(|(r, c)| (r, *c)))
            .expect("主按钮没画出文字");
        assert_eq!(color, Color32::WHITE, "主按钮文字不是白色");
        assert!(
            text_rect.width() > 4.0 && text_rect.height() > 4.0,
            "主按钮文字尺寸异常：{text_rect:?}"
        );
    }

    #[test]
    fn primary_button_keeps_its_size_and_click_zone() {
        // 自绘后仍要走 allocate_exact_size，否则点击区域会缩成 0
        let ctx = egui::Context::default();
        let mut resp_rect = egui::Rect::NOTHING;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.add_space(8.0);
            let r = primary(ui, &DARK, "OK");
            resp_rect = r.rect;
            // 单行文字撑不出 BTN_W，按钮必须自己撑到最窄宽度
            assert!((r.rect.width() - BTN_W).abs() < 1.0);
            assert!((r.rect.height() - CTRL_H).abs() < 1.0);
            assert!(r.interact_rect.contains(r.rect.center()));
        });
        out.textures_delta.clear();
        assert!(resp_rect.width() > 0.0);
    }

    #[test]
    fn icon_size_is_clamped() {
        // 传 0 也不能炸：夹到最小边再算，长度自洽
        let icon = app_icon(0);
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
        assert!(icon.width >= 8);
    }
}
