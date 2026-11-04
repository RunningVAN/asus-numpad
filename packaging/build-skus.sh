#!/usr/bin/env bash
# 按机型分别出 RPM：每个机型一份二进制 + 一份包，包之间互斥。macOS/Linux 均可跑。
#
#   ./packaging/build-skus.sh                 # skus.tsv 里全部机型
#   ./packaging/build-skus.sh g614 g615       # 只出指定机型
#   ./packaging/build-skus.sh --universal     # 额外出不校验机型的兼容包
#
# 前置：cargo（含 x86_64-unknown-linux-gnu target）、python3；macOS 另需 zig（ASUS_ZIGCC）。
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HANDOFF="$(cd "$REPO/.." && pwd)"
RPMGEN="$REPO/../../tools/rpmgen/build_rpm.py"
[[ -f "$RPMGEN" ]] || RPMGEN="$HANDOFF/tools/rpmgen/build_rpm.py"
SKUS_TSV="$REPO/packaging/skus.tsv"
OUT="${ASUS_OUT:-$HOME/Downloads}"
TARGET="x86_64-unknown-linux-gnu"
BUILDROOT="${ASUS_BUILDROOT:-$HANDOFF/build-skus}"
WITH_UNIVERSAL=0
SELECTED=()

for arg in "$@"; do
  case "$arg" in
    --universal) WITH_UNIVERSAL=1 ;;
    -h|--help) sed -n '2,16p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) SELECTED+=("$arg") ;;
  esac
done

[[ -f "$RPMGEN" ]] || { echo "找不到 build_rpm.py（试过 ${RPMGEN}）" >&2; exit 1; }
[[ -f "$SKUS_TSV" ]] || { echo "找不到 $SKUS_TSV" >&2; exit 1; }
mkdir -p "$OUT" "$BUILDROOT"

export PATH="$HOME/.cargo/bin:$PATH"

# ---------- 0. 交叉编译工具链 ----------
if [[ "$(uname -s)" == "Darwin" ]]; then
  ZIGCC="${ASUS_ZIGCC:-}"
  if [[ -z "$ZIGCC" ]]; then
    for cand in "$REPO/.zigcc-linker" "$HANDOFF/zigcc.sh" "$HANDOFF/tools/zigcc.sh"; do
      [[ -x "$cand" ]] && ZIGCC="$cand" && break
    done
  fi
  if [[ -z "$ZIGCC" ]]; then
    # 没有现成 wrapper 就自己包一层：cargo 的 linker 只接受可执行路径，不能带参数
    zbin="$(command -v zig || true)"
    [[ -z "$zbin" ]] && zbin="$(find "$(dirname "$HANDOFF")" -maxdepth 2 -type f -name zig -perm -u+x 2>/dev/null | head -1)"
    if [[ -n "$zbin" ]]; then
      ZIGCC="$BUILDROOT/zigcc"
      mkdir -p "$BUILDROOT"
      printf '#!/bin/sh\nexec "%s" cc -target x86_64-linux-gnu.2.28 "$@"\n' "$zbin" > "$ZIGCC"
      chmod +x "$ZIGCC"
    fi
  fi
  [[ -n "$ZIGCC" && -x "$ZIGCC" ]] || { echo "macOS 上需要 zig 交叉链接器：设 ASUS_ZIGCC=/path/to/zigcc.sh" >&2; exit 1; }
  echo "== 交叉链接器：$ZIGCC"
  export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$ZIGCC"
  export CC_x86_64_unknown_linux_gnu="$ZIGCC"
fi

# ---------- 1. 清单与代码里的机型集合必须一致 ----------
tsv_ids="$(awk -F'\t' '!/^#/ && NF>=4 {print $1}' "$SKUS_TSV" | sort)"
code_ids="$(grep -oE '^[[:space:]]*id: "[a-z0-9]+"' "$REPO/src/sku.rs" | grep -oE '"[a-z0-9]+"' | tr -d '"' | grep -v '^universal$' | sort)"
if [[ "$tsv_ids" != "$code_ids" ]]; then
  echo "机型清单漂移：skus.tsv 与 src/sku.rs 不一致" >&2
  diff <(echo "$tsv_ids") <(echo "$code_ids") >&2 || true
  exit 1
fi

# merged 列（被合并掉的旧机型包）不能与本轮出包清单里的 id 撞车：
# 同一个 id 既单独出包、又被别的包 Obsoletes，dnf 升级时会直接绕死。
merged_ids="$(awk -F'\t' '!/^#/ && NF>=5 {print $5}' "$SKUS_TSV" | tr ',' '\n' | sed '/^[[:space:]]*$/d' | tr -d ' ' | sort)"
if [[ -n "$merged_ids" ]]; then
  dup="$(comm -12 <(printf '%s\n' "$merged_ids") <(printf '%s\n' "$tsv_ids") || true)"
  if [[ -n "$dup" ]]; then
    echo "skus.tsv 的 merged 列与出包清单撞车：$dup" >&2
    exit 1
  fi
fi

# ---------- 2. 待出包机型 ----------
if [[ ${#SELECTED[@]} -gt 0 ]]; then
  IDS=("${SELECTED[@]}")
else
  IDS=()
  while IFS=$'\t' read -r id _rest; do
    [[ "$id" == \#* || -z "$id" ]] && continue
    IDS+=("$id")
  done < "$SKUS_TSV"
fi
[[ "$WITH_UNIVERSAL" == "1" ]] && IDS+=("universal")

# Conflicts 必须覆盖「其余全部机型包」，与本次只出哪几个无关；
# 否则分批出包时新包只与同批互斥，跨批可共存。
ALL_IDS="$(awk -F'\t' '!/^#/ && NF>=4 {print $1}' "$SKUS_TSV" | paste -sd, -)"

echo "== 待出包机型：${IDS[*]}"
echo "== 输出目录：$OUT"
echo

made=()
for id in "${IDS[@]}"; do
  display=""; models=""; note=""; merged=""
  if [[ "$id" != "universal" ]]; then
    line="$(awk -F'\t' -v want="$id" '!/^#/ && $1==want {print; exit}' "$SKUS_TSV")"
    [[ -n "$line" ]] || { echo "skus.tsv 里没有机型 $id" >&2; exit 1; }
    display="$(cut -f2 <<<"$line")"
    models="$(cut -f3 <<<"$line")"
    note="$(cut -f4 <<<"$line")"
    merged="$(cut -f5 <<<"$line")"
  else
    display="全机型兼容包（不做机型校验）"
    note="仅在不确定机型时兜底使用"
  fi

  echo "---- [$id] 编译"
  # sku.rs 的机型来自编译期环境变量，先 touch 保证 cargo 一定重编（build.rs 也有声明，双保险）
  touch "$REPO/src/sku.rs"
  ASUS_NUMPAD_SKU="$id" cargo build --release --target "$TARGET" --manifest-path "$REPO/Cargo.toml" \
    --quiet
  bin="$BUILDROOT/$id/asus-numpad"
  mkdir -p "$(dirname "$bin")"
  cp "$REPO/target/$TARGET/release/asus-numpad" "$bin"

  # 出包前先自证二进制确实是这一机型编出来的：cargo 不会因环境变量变化自动重编，
  # 一旦拿到上一个机型的二进制，包名对了内容却是错的（最难查的一类问题）。
  bin_sha="$(shasum -a 256 "$bin" | cut -c1-16)"
  if [[ -n "${prev_sha:-}" && "$bin_sha" == "$prev_sha" ]]; then
    echo "   !! 二进制与上一个机型完全相同（${bin_sha}）——疑似没有重编，停止出包" >&2
    exit 1
  fi
  prev_sha="$bin_sha"
  if ! grep -aq -- "asus-numpad-${id}" "$bin"; then
    echo "   !! 二进制里找不到包名标记 asus-numpad-${id}，机型注入没生效，停止出包" >&2
    exit 1
  fi
  echo "     机型标记校验通过：asus-numpad-${id}（sha ${bin_sha}）"

  echo "---- [$id] 打 RPM"
  ASUS_SKU="$id" \
  ASUS_SKU_DISPLAY="$display" \
  ASUS_SKU_MODELS="$models" \
  ASUS_SKU_NOTE="$note" \
  ASUS_SKU_MERGED="$merged" \
  ASUS_SKU_ALL="$ALL_IDS" \
  ASUS_REPO="$REPO" \
  ASUS_BIN="$bin" \
  ASUS_OUT="$OUT" \
    python3 "$RPMGEN" region

  pkg="$(ls -t "$OUT"/asus-numpad-"$id"-*.rpm 2>/dev/null | head -1)"
  [[ -n "$pkg" ]] || { echo "没有产出 $id 的 rpm" >&2; exit 1; }
  made+=("$pkg")
  echo
done

echo "==== 共出 ${#made[@]} 个包 ===="
for p in "${made[@]}"; do
  printf '%-72s %8s  %s\n' "$(basename "$p")" "$(du -h "$p" | cut -f1)" "$(shasum -a 256 "$p" 2>/dev/null | cut -c1-16 || sha256sum "$p" | cut -c1-16)"
done
