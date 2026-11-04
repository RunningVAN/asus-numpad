#!/usr/bin/env bash
# 构建 Alpine 包（.apk）。
#
# 前置：apk add alpine-sdk；abuild-keygen -a -i（一次即可，否则 abuild 拒绝签名）
#
# 用法: ./packaging/alpine/make-alpine.sh [--version X] [--out DIR]

set -euo pipefail
SELF_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SELF_DIR/../lib/common.sh"

VERSION=""
OUT_DIR=""

while (($#)); do
    case "$1" in
        --version) VERSION="${2:?}"; shift 2 ;;
        --out)     OUT_DIR="${2:?}"; shift 2 ;;
        -h|--help) sed -n '2,7p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *)         die "未知参数: $1" ;;
    esac
done

ROOT="$(repo_root)"
VERSION="${VERSION:-$(cargo_version)}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
mkdir -p "$OUT_DIR"

have abuild || die "缺少 abuild（apk add alpine-sdk）"
if ! ls "$HOME/.abuild/"*.rsa.pub >/dev/null 2>&1; then
    warn "没有 abuild 密钥，先执行 abuild-keygen -a -i，否则 abuild 会拒绝出包"
fi

# APKBUILD 的 source 指向 GitHub 归档，本地出包时改用当前工作树打出来的 tarball，
# 这样未发布的改动也能测到
WORK="$(mktmp)"
APKROOT="$WORK/aports"
SRCDEST="$WORK/srcdest"
mkdir -p "$APKROOT" "$SRCDEST"

log "导出源码树 $PKG_NAME-$VERSION"
SRCTREE="$WORK/$PKG_NAME-$VERSION"
mkdir -p "$SRCTREE"
tar -C "$ROOT" --exclude='./target' --exclude='./dist' --exclude='./.git' -cf - . \
    | tar -C "$SRCTREE" -xf -
tar -C "$WORK" -czf "$SRCDEST/$PKG_NAME-$VERSION.tar.gz" "$PKG_NAME-$VERSION"
SHA="$(sha512sum "$SRCDEST/$PKG_NAME-$VERSION.tar.gz" | cut -d' ' -f1)"

sed -e "s/^pkgver=.*/pkgver=$VERSION/" \
    -e "s/^sha512sums=.*/sha512sums=\"$SHA\"/" \
    -e "s|^source=.*|source=\"$PKG_NAME-\$pkgver.tar.gz\"|" \
    "$SELF_DIR/APKBUILD" > "$APKROOT/APKBUILD"
install -m 0644 "$SELF_DIR/$PKG_NAME.post-install" "$APKROOT/"

log "abuild -r（产物目录 $OUT_DIR）"
(
    cd "$APKROOT"
    SRCDEST="$SRCDEST" PKGDEST="$OUT_DIR" REPODEST="$WORK/packages" \
        abuild -r -P "$WORK/packages"
)

log "完成，产物："
find "$OUT_DIR" -maxdepth 1 -name '*.apk' -printf '  %f\n' 2>/dev/null || ls -l "$OUT_DIR"
