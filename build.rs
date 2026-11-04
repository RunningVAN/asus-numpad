//! 构建脚本：把「本二进制属于哪个机型」写成编译期常量，并声明机型变量变了必须重编。
//!
//! 为什么不用 `option_env!`：cargo 默认不会因为环境变量变化重编 crate，连续出多个机型包
//! 时会拿到上一个机型的二进制（包名对、内容错）。这里读 `ASUS_NUMPAD_SKU` 生成
//! `$OUT_DIR/build_sku.rs`，同时用 `rerun-if-env-changed` 让 cargo 正确失效缓存；
//! 生成的字符串常量进了 .rodata，出包脚本可以直接 grep 二进制自证机型。

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=ASUS_NUMPAD_SKU");

    let raw = env::var("ASUS_NUMPAD_SKU").unwrap_or_default();
    let sku = raw.trim();
    let sku = if sku.is_empty() { "universal" } else { sku };

    let out =
        PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR 必须由 cargo 提供")).join("build_sku.rs");
    let body = format!(
        "// 由 build.rs 生成，勿手改。\n\
         pub const BUILD_SKU_ID: &str = \"{sku}\";\n\
         pub const BUILD_PACKAGE_NAME: &str = \"asus-numpad-{sku}\";\n",
        sku = sku
    );
    fs::write(&out, body).expect("写 build_sku.rs 失败");
}
