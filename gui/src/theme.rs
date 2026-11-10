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
        .show(ui, add)
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
            .min_size(Vec2::new(0.0, 30.0)),
    )
}

/// 次级按钮：卡片底色 + 描边，和主按钮并排时不抢注意力。
pub fn secondary(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(p.text))
            .fill(p.sunk)
            .stroke(Stroke::new(1.0, p.stroke))
            .corner_radius(CornerRadius::same(WIDGET_RADIUS))
            .min_size(Vec2::new(0.0, 30.0)),
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
    let total = widths.iter().sum::<f32>() + 8.0;
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
pub const LABEL_W: f32 = 112.0;

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

/// 一行「标签 + 控件」：标签右对齐占固定宽度，控件从控件列起。
///
/// 有没有 info 图标都预留同样的位置，这样同一张卡片里所有控件左边缘能连成一条线。
pub fn row<R>(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    help: Option<&str>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(LABEL_W, 24.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                match help {
                    Some(text) => {
                        info_dot(ui, p, text);
                    }
                    None => {
                        ui.add_space(15.0);
                    }
                }
                ui.label(RichText::new(label).size(13.0).color(p.text_weak));
            },
        );
        add(ui)
    })
    .inner
}

/// 只出标签、不带控件：给那些自带 `ui.horizontal(|ui| { label; combo })` 的老布局就地补对齐。
///
/// 标签本身也挂悬浮说明——这类行没有可以放 `i` 的地方，鼠标停在标签上一样能看解释。
pub fn label_cell(ui: &mut egui::Ui, p: &Palette, text: &str, help: Option<&str>) {
    ui.allocate_ui_with_layout(
        Vec2::new(LABEL_W, 24.0),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            match help {
                Some(h) => {
                    info_dot(ui, p, h);
                }
                None => {
                    ui.add_space(15.0);
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
                egui::Label::new(RichText::new(text.into()).size(12.0).color(p.text_weak)).wrap(),
            );
        });
    });
}
