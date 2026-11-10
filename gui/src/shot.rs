//! 开发辅助：把当前画面存成 PPM，用来更新文档和官网截图。
//!
//! 只有设置了环境变量 `ASUS_NUMPAD_GUI_SHOT=<路径>` 才生效，正常启动完全不走这里。
//! 之所以不用系统截图工具：一是 macOS 屏幕录制权限在新系统上经常拿不到，
//! 二是系统截图还会带窗口阴影和桌面背景，而这里拿到的是渲染出来的原始像素，1:1、干净。
//!
//!   ASUS_NUMPAD_GUI_FULL=1 ASUS_NUMPAD_GUI_SHOT=full.ppm ./target/debug/asus-numpad-gui
//!
//! 存完即退出，方便脚本化；PPM 交给 `PIL`/`sips` 转 PNG（不为此给 GUI 加图片依赖）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use eframe::egui;

/// 截图前先空转几帧：字体、面板宽度和滚动区域都要几帧才稳定。
const WARMUP_FRAMES: u32 = 16;

static FRAMES: AtomicU32 = AtomicU32::new(0);
static REQUESTED: AtomicBool = AtomicBool::new(false);

fn target_path() -> Option<PathBuf> {
    std::env::var_os("ASUS_NUMPAD_GUI_SHOT").map(PathBuf::from)
}

/// 每帧调用一次。到点就请求截图，回调里落盘并退出。
pub fn tick(ctx: &egui::Context) {
    let Some(path) = target_path() else {
        return;
    };
    let frames = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if frames < WARMUP_FRAMES {
        ctx.request_repaint();
        return;
    }
    if REQUESTED.swap(true, Ordering::Relaxed) {
        return;
    }
    ctx.request_screenshot(move |image| {
        match write_ppm(&path, &image) {
            Ok(()) => eprintln!("已保存截图 {}", path.display()),
            Err(err) => eprintln!("保存截图失败：{err}"),
        }
        std::process::exit(0);
    });
}

/// 二进制 PPM（P6）：三字节一像素，任何图像工具都能读，不需要额外依赖。
fn write_ppm(path: &Path, image: &egui::ColorImage) -> std::io::Result<()> {
    let [width, height] = image.size;
    let mut bytes = Vec::with_capacity(width * height * 3 + 32);
    bytes.extend_from_slice(format!("P6\n{width} {height}\n255\n").as_bytes());
    for pixel in &image.pixels {
        bytes.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b()]);
    }
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(path, bytes)
}
