#!/usr/bin/env bash
# 按本机 DMI 型号挑对应的机型包并安装（只有装包要 sudo）。
#
#   sudo ./install-sku.sh              # 自动识别机型，从同目录/下载目录找包
#   sudo ./install-sku.sh --dry-run    # 只说会装哪个
#   sudo ./install-sku.sh --from ~/rpm # 指定包目录
#
# 判断依据：/sys/class/dmi/id/product_name。18 英寸 Plus 机型是物理数字键，直接拒绝。
set -euo pipefail

FROM_DIR=""
DRY_RUN=0
while [ $# -gt 0 ]; do
    case "$1" in
        --from) FROM_DIR="${2:-}"; shift 2 ;;
        --dry-run) DRY_RUN=1; shift ;;
        -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
        *) echo "未知参数：$1" >&2; exit 2 ;;
    esac
done

DMI_FILE="${ASUS_DMI_FILE:-/sys/class/dmi/id/product_name}"
if [ ! -r "$DMI_FILE" ]; then
    echo "读不到 ${DMI_FILE}，不是 Linux 或权限不足（这个脚本只在装了驱动的机器上跑）" >&2
    exit 2
fi
DMI="$(tr -d '\n\r' < "$DMI_FILE" | sed 's/[[:space:]]*$//')"
echo "本机 DMI 型号：$DMI"

# 18 英寸 Plus：官方规格是物理 Num-key（with Num-key），没有虚拟数字键盘
case "$DMI" in
    *G834*|*G835*|*G815*|*G814*|*G813*)
        echo "这台是 18 英寸 Plus 机型，触控板没有虚拟数字键盘（物理数字键），不需要装 asus-numpad。" >&2
        exit 4
        ;;
esac

SKU=""
case "$DMI" in
    *G533*) SKU=g533 ;;
    *G614*) SKU=g614 ;;
    *G615*) SKU=g615 ;;
    *G634*) SKU=g634 ;;
    *G635*) SKU=g635 ;;
    *GX550*|*GX551*) SKU=gx550 ;;
    *GX650*) SKU=gx650 ;;
    *UX582*|*UX482*) SKU=ux582 ;;
    *H7606*) SKU=h7606 ;;
esac

if [ -z "$SKU" ]; then
    echo "型号 $DMI 不在已知机型表里。" >&2
    echo "可选项：装 asus-numpad-universal（不做机型校验），或先在 src/sku.rs 的 SKUS 里加一条。" >&2
    exit 3
fi
echo "匹配机型包：asus-numpad-$SKU"

# 找包：指定目录 > 脚本所在目录 > ~/Downloads
SEARCH=()
[ -n "$FROM_DIR" ] && SEARCH+=("$FROM_DIR")
SEARCH+=("$(cd "$(dirname "$0")" && pwd)" "$HOME/Downloads")
PKG=""
for d in "${SEARCH[@]}"; do
    [ -d "$d" ] || continue
    cand="$(ls -1 "$d"/asus-numpad-"$SKU"-*.rpm 2>/dev/null | sort -V | tail -1 || true)"
    if [ -n "$cand" ]; then PKG="$cand"; break; fi
done

if [ -z "$PKG" ]; then
    echo "在 ${SEARCH[*]} 里没找到 asus-numpad-$SKU-*.rpm" >&2
    exit 5
fi
echo "使用安装包：$PKG"

# 已装其它机型包时先提示（两个包互相 Conflicts，dnf 自己也会拦）
if command -v rpm >/dev/null 2>&1; then
    for other in g533 g614 g615 g634 g635 gx550 gx650 ux582 h7606 universal; do
        [ "$other" = "$SKU" ] && continue
        if rpm -q "asus-numpad-$other" >/dev/null 2>&1; then
            echo "注意：当前已装 asus-numpad-${other}，安装前需要先卸载它（sudo dnf remove asus-numpad-${other}）"
        fi
    done
fi

if [ "$DRY_RUN" = "1" ]; then
    echo "（--dry-run）未执行安装。真正的命令："
    echo "  sudo dnf install '$PKG'"
    exit 0
fi

if command -v dnf >/dev/null 2>&1; then
    dnf install -y "$PKG"
elif command -v rpm >/dev/null 2>&1; then
    rpm -Uvh --replacepkgs "$PKG"
else
    echo "没有 dnf 也没有 rpm，无法安装" >&2
    exit 6
fi

echo
echo "装完先跑这两条确认："
echo "  asus-numpad sku      # 应显示本包机型 $SKU 与本机 DMI ${DMI}，判定为 match"
echo "  asus-numpad doctor   # 查触控板与背光节点"
