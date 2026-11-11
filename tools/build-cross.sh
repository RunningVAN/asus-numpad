#!/usr/bin/env bash
# 在 macOS / 任意主机上交叉构建 Windows 与 Linux 的 asus-numpad.exe / asus-numpad。
#
# 为什么要 zig：Rust 的 windows-gnu 目标要一个 MinGW 链接器，linux-gnu 目标要一个
# glibc 工具链。zig 自带这两套（`zig cc` 内含 mingw-w64 与 glibc），不用装
# mingw-w64、也不用起容器；cargo-zigbuild 负责把 Rust 的 `-nodefaultlibs`
# 链接参数翻译成 zig 认的形式。
#
#   tools/build-cross.sh              # 两个目标都建
#   tools/build-cross.sh windows      # 只建 exe
#   tools/build-cross.sh linux        # 只建 elfin
#
# 产物：
#   target/x86_64-pc-windows-gnu/release/asus-numpad.exe
#   target/x86_64-unknown-linux-gnu/release/asus-numpad
#
# Linux 目标固定 glibc 2.17 作为下限，Fedora / Ubuntu 18.04 之后都能直接跑。
set -euo pipefail
cd "$(dirname "$0")/.."

ZIG_VERSION="${ZIG_VERSION:-0.17.1}"
GLIBC="${GLIBC:-2.17}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/asus-numpad-zig"

case "$(uname -m)" in
aarch64 | arm64) ZIG_ARCH=aarch64 ;;
x86_64 | amd64) ZIG_ARCH=x86_64 ;;
*) echo "不认识的架构：$(uname -m)" >&2; exit 1 ;;
esac
case "$(uname -s)" in
Darwin) ZIG_OS=macos ;;
Linux) ZIG_OS=linux ;;
*) echo "只支持 macOS / Linux 作为构建机（Windows 主机直接 cargo build 即可）" >&2; exit 1 ;;
esac

if ! command -v cargo-zigbuild >/dev/null; then
  echo "缺 cargo-zigbuild，正在安装……"
  cargo install cargo-zigbuild --locked
fi

ZIG_BIN="$CACHE/zig-$ZIG_ARCH-$ZIG_OS-$ZIG_VERSION/zig"
if [ ! -x "$ZIG_BIN" ]; then
  mkdir -p "$CACHE"
  tarball="$CACHE/zig-$ZIG_VERSION.tar.xz"
  url="https://ziglang.org/download/$ZIG_VERSION/zig-$ZIG_ARCH-$ZIG_OS-$ZIG_VERSION.tar.xz"
  echo "下载 $url"
  curl -fsSL --retry 3 -o "$tarball" "$url"
  tar -xf "$tarball" -C "$CACHE"
fi
export PATH="$(dirname "$ZIG_BIN"):$PATH"
echo "zig $("$ZIG_BIN" version)"

want="${1:-both}"
if [ "$want" = both ] || [ "$want" = windows ]; then
  echo "==> x86_64-pc-windows-gnu"
  cargo zigbuild --release --target x86_64-pc-windows-gnu --bin asus-numpad
fi
if [ "$want" = both ] || [ "$want" = linux ]; then
  echo "==> x86_64-unknown-linux-gnu (glibc >= $GLIBC)"
  cargo zigbuild --release --target "x86_64-unknown-linux-gnu.$GLIBC" --bin asus-numpad
fi

for f in target/x86_64-pc-windows-gnu/release/asus-numpad.exe \
  target/x86_64-unknown-linux-gnu/release/asus-numpad; do
  [ -f "$f" ] && printf '%-56s %s\n' "$f" "$(du -h "$f" | cut -f1)"
done
