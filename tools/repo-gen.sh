#!/usr/bin/env bash
# 把 dist/ 里的包整理成四套可被包管理器直接消费的仓库，供给 GitHub 分发。
#
# 产出目录（默认 <repo>/repo）：
#   apt/     Debian / Ubuntu      deb [trusted=yes] <base>/apt stable main
#   rpm/     Fedora / RHEL / SUSE baseurl=<base>/rpm/$basearch
#   arch/    Arch / Manjaro       Server = <base>/arch/$arch
#   alpine/  Alpine               <base>/alpine/v3.x/main 形式的手工源
#   index.html                    给人看的安装说明
#
# 索引工具缺失时只跳过对应生态并告警，不中断其它生态——打包机不必装齐所有工具。
#
# 用法:
#   ./tools/repo-gen.sh                       # dist/ -> repo/
#   ./tools/repo-gen.sh --dist DIR --out DIR
#   ./tools/repo-gen.sh --keep 2              # 每个生态只留最近 2 个版本
#   ./tools/repo-gen.sh --base-url https://... # 写进 index.html 的地址

set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/dist"
OUT="$ROOT/repo"
KEEP=3
BASE_URL="https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo"

log()  { printf '==> %s\n' "$*"; }
warn() { printf '警告: %s\n' "$*" >&2; }
die()  { printf '错误: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

while (($#)); do
    case "$1" in
        --dist)     DIST="${2:?}"; shift 2 ;;
        --out)      OUT="${2:?}"; shift 2 ;;
        --keep)     KEEP="${2:?}"; shift 2 ;;
        --base-url) BASE_URL="${2:?}"; shift 2 ;;
        -h|--help)  sed -n '2,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *)          die "未知参数: $1" ;;
    esac
done

[[ -d "$DIST" ]] || die "产物目录不存在: $DIST"

# 版本从 deb/rpm 文件名里取，四个生态共用同一套版本号
VERSION="$(ls "$DIST" | sed -n 's/^asus-numpad_\([0-9][^-]*\)-[0-9]*_[a-z0-9]*\.deb$/\1/p' | sort -V | tail -1)"
[[ -n "$VERSION" ]] || VERSION="$(ls "$DIST" | sed -n 's/^asus-numpad-\([0-9][^-]*\)-[0-9]*\..*\.rpm$/\1/p' | sort -V | tail -1)"
[[ -n "$VERSION" ]] || warn "推不出版本号，索引里的 Date 用当天"
VERSION="${VERSION:-0.0.0}"

log "版本 $VERSION，产物 $DIST，输出 $OUT"
mkdir -p "$OUT"

# --------------------------------------------------------------------------- #
# 清理旧版本：仓库在 git 里，无限堆积会把仓库撑爆
# --------------------------------------------------------------------------- #
prune() {
    local dir="$1" count
    count=$(find "$dir" -maxdepth 1 -type f -name '*.deb' -o -maxdepth 1 -type f -name '*.rpm' \
            -o -maxdepth 1 -type f -name '*.pkg.tar.*' -o -maxdepth 1 -type f -name '*.apk' 2>/dev/null | wc -l | tr -d ' ')
    ((count <= KEEP)) && return 0
    local drop=$((count - KEEP))
    log "清理 $dir 下最旧 $drop 个包"
    find "$dir" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.pkg.tar.*' -o -name '*.apk' \) \
        | sort -V | head -n "$drop" | while read -r f; do rm -f -- "$f"; done
}

# --------------------------------------------------------------------------- #
# APT
# --------------------------------------------------------------------------- #
do_apt() {
    local debs; shopt -s nullglob; debs=("$DIST"/asus-numpad_*.deb); shopt -u nullglob
    ((${#debs[@]})) || { log "跳过 apt（没有 .deb）"; return 0; }

    log "布置 apt 仓库"
    local pool="$OUT/apt/pool/main/a/asus-numpad"
    mkdir -p "$pool" "$OUT/apt/dists/stable/main"
    cp -f "${debs[@]}" "$pool/"
    prune "$pool"

    # 按 deb 架构分目录；apt 靠它决定给谁下发
    local arch listed=0
    for a in amd64 arm64 armhf all; do
        local dir="$OUT/apt/dists/stable/main/binary-$a"
        mkdir -p "$dir"
        ( cd "$OUT/apt" && dpkg-scanpackages --multiversion --arch "$a" pool/main 2>/dev/null ) > "$dir/Packages" || true
        if [[ -s "$dir/Packages" ]]; then
            gzip -9c "$dir/Packages" > "$dir/Packages.gz"
            listed=1
        else
            rm -rf "$dir"
        fi
    done
    ((listed)) || warn "apt：dpkg-scanpackages 没扫出包（工具缺失？）"

    # Release：不签名的仓库，apt 用 [trusted=yes] 绕过 GPG；校验值仍然写全，
    # 这样即使以后加签名也不用重做结构
    local rel="$OUT/apt/dists/stable/Release"
    {
        printf 'Origin: asus-numpad\n'
        printf 'Label: asus-numpad\n'
        printf 'Suite: stable\n'
        printf 'Codename: stable\n'
        printf 'Version: %s\n' "$VERSION"
        printf 'Architectures: amd64 arm64\n'
        printf 'Components: main\n'
        printf 'Description: ASUS NumberPad userspace driver for Linux\n'
        printf 'Date: %s\n' "$(LC_ALL=C date -u '+%a, %d %b %Y %H:%M:%S UTC')"
        printf 'SHA256:\n'
        ( cd "$OUT/apt" && find dists/stable/main -name 'Packages*' -type f | sort | while read -r f; do
            printf ' %s %s %s\n' "$(sha256sum "$f" | cut -d' ' -f1)" "$(wc -c < "$f" | tr -d ' ')" "$f"
        done )
        printf 'MD5Sum:\n'
        ( cd "$OUT/apt" && find dists/stable/main -name 'Packages*' -type f | sort | while read -r f; do
            printf ' %s %s %s\n' "$(md5sum "$f" | cut -d' ' -f1)" "$(wc -c < "$f" | tr -d ' ')" "$f"
        done )
    } > "$rel"
    gzip -9c "$rel" > "$rel.gz"
}

# --------------------------------------------------------------------------- #
# RPM (dnf / yum / zypper)
# --------------------------------------------------------------------------- #
do_rpm() {
    shopt -s nullglob
    local rpms=("$DIST"/asus-numpad-*.rpm)
    shopt -u nullglob
    ((${#rpms[@]})) || { log "跳过 rpm（没有 .rpm）"; return 0; }

    local arch
    for r in "${rpms[@]}"; do
        arch="$(basename "$r")"
        case "$arch" in
            *.x86_64.rpm)  arch=x86_64 ;;
            *.aarch64.rpm) arch=aarch64 ;;
            *)             arch=x86_64 ;;
        esac
        local dir="$OUT/rpm/$arch"
        mkdir -p "$dir"
        cp -f "$r" "$dir/"
    done

    for d in "$OUT"/rpm/*/; do
        [[ -d "$d" ]] || continue
        prune "$d"
        if have createrepo_c; then
            log "生成 rpm repodata: $(basename "$d")"
            createrepo_c --quiet --update --database "$d"
        elif have createrepo; then
            log "生成 rpm repodata（createrepo）: $(basename "$d")"
            createrepo --quiet --update --database "$d"
        else
            warn "缺少 createrepo_c，跳过 $(basename "$d") 的元数据（apt install createrepo-c）"
        fi
    done
}

# --------------------------------------------------------------------------- #
# Pacman
# --------------------------------------------------------------------------- #
do_arch() {
    shopt -s nullglob
    local pkgs=("$DIST"/asus-numpad-*.pkg.tar.*)
    shopt -u nullglob
    ((${#pkgs[@]})) || { log "跳过 arch（没有 .pkg.tar.*）"; return 0; }

    local arch
    for p in "${pkgs[@]}"; do
        arch="$(basename "$p")"
        case "$arch" in
            *.aarch64.*) arch=aarch64 ;;
            *)           arch=x86_64 ;;
        esac
        local dir="$OUT/arch/$arch"
        mkdir -p "$dir"
        cp -f "$p" "$dir/"
    done

    for d in "$OUT"/arch/*/; do
        [[ -d "$d" ]] || continue
        prune "$d"
        ( cd "$d" && rm -f asus-numpad.db* asus-numpad.files* )
        if have repo-add; then
            log "生成 pacman 数据库: $(basename "$d")"
            local files=()
            while IFS= read -r f; do files+=("$(basename "$f")"); done \
                < <(find "$d" -maxdepth 1 -name 'asus-numpad-*.pkg.tar.*' | sort)
            ( cd "$d" && repo-add -q asus-numpad.db.tar.gz "${files[@]}" >/dev/null )
        else
            warn "缺少 repo-add（pacman），跳过 $(basename "$d") 的数据库"
        fi
    done
}

# --------------------------------------------------------------------------- #
# Alpine (apk)
# --------------------------------------------------------------------------- #
do_alpine() {
    shopt -s nullglob
    local apks=("$DIST"/*.apk)
    shopt -u nullglob
    ((${#apks[@]})) || { log "跳过 alpine（没有 .apk）"; return 0; }

    for a in "${apks[@]}"; do
        local arch
        case "$(basename "$a")" in
            *aarch64*) arch=aarch64 ;;
            *)         arch=x86_64 ;;
        esac
        local dir="$OUT/alpine/$arch"
        mkdir -p "$dir"
        cp -f "$a" "$dir/"
    done

    for d in "$OUT"/alpine/*/; do
        [[ -d "$d" ]] || continue
        prune "$d"
        if have apk; then
            log "生成 APKINDEX: $(basename "$d")"
            ( cd "$d" && rm -f APKINDEX.tar.gz && apk index -o APKINDEX.tar.gz --rewrite-arch "$(basename "$d")" ./*.apk >/dev/null )
        else
            warn "缺少 apk 工具，跳过 $(basename "$d") 的索引（apk-tools）"
        fi
    done
}

do_apt
do_rpm
do_arch
do_alpine

# --------------------------------------------------------------------------- #
# 人看的入口页
# --------------------------------------------------------------------------- #
log "生成 index.html"
cat > "$OUT/index.html" <<HTML
<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>asus-numpad ${VERSION} 软件源</title>
<style>
  :root { color-scheme: light dark; }
  body { font: 15px/1.65 -apple-system, "Segoe UI", "Noto Sans SC", sans-serif;
         max-width: 46rem; margin: 3rem auto; padding: 0 1.2rem; }
  h1 { font-size: 1.5rem; margin-bottom: .2rem; }
  .sub { opacity: .65; margin-top: 0; }
  h2 { font-size: 1.05rem; margin-top: 2.2rem; border-bottom: 1px solid #8884; padding-bottom: .3rem; }
  pre { background: #8881; padding: .8rem 1rem; border-radius: 6px; overflow-x: auto; font-size: 13px; }
  code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
  table { border-collapse: collapse; width: 100%; font-size: 14px; }
  th, td { text-align: left; padding: .35rem .6rem; border-bottom: 1px solid #8883; }
</style>
</head>
<body>
<h1>asus-numpad ${VERSION}</h1>
<p class="sub">ASUS 触摸板数字小键盘（NumberPad）的 Linux 用户态驱动 · GPL-3.0-or-later</p>

<p>本页是软件源根地址：<code>${BASE_URL}</code>。下面的命令直接可用，无需下载密钥。</p>

<h2>Debian / Ubuntu / Mint</h2>
<pre><code>echo "deb [trusted=yes] ${BASE_URL}/apt stable main" \\
  | sudo tee /etc/apt/sources.list.d/asus-numpad.list
sudo apt update &amp;&amp; sudo apt install asus-numpad</code></pre>
<p>仓库未做 GPG 签名，因此用 <code>trusted=yes</code>。介意的话请改用 Release 里的 .deb 手工装。</p>

<h2>Fedora / RHEL / Rocky / openSUSE</h2>
<pre><code>sudo tee /etc/yum.repos.d/asus-numpad.repo &lt;&lt;'EOF'
[asus-numpad]
name=asus-numpad
baseurl=${BASE_URL}/rpm/\$basearch
enabled=1
gpgcheck=0
EOF
sudo dnf install asus-numpad</code></pre>

<h2>Arch / Manjaro</h2>
<pre><code>sudo tee -a /etc/pacman.conf &lt;&lt;'EOF'

[asus-numpad]
SigLevel = Optional TrustAll
Server = ${BASE_URL}/arch/\$arch
EOF
sudo pacman -Sy asus-numpad</code></pre>

<h2>Alpine</h2>
<pre><code>echo "${BASE_URL}/alpine/\$(apk --print-arch)" \\
  | sudo tee -a /etc/apk/repositories
sudo apk update &amp;&amp; sudo apk add --allow-untrusted asus-numpad</code></pre>

<h2>Nix / NixOS</h2>
<pre><code>nix run github:RunningVAN/asus-numpad
# NixOS: services.asus-numpad.enable = true;</code></pre>

<h2>其它发行版</h2>
<p>用 Release 里的通用 tarball（musl 静态链接，任意 x86_64 Linux 可跑）：</p>
<pre><code>tar xzf asus-numpad-${VERSION}-x86_64.tar.gz
cd asus-numpad-${VERSION} &amp;&amp; sudo ./install.sh</code></pre>

<h2>内容</h2>
<table>
<tr><th>路径</th><th>说明</th></tr>
<tr><td><code>apt/</code></td><td>APT 仓库（dists/stable + pool）</td></tr>
<tr><td><code>rpm/&lt;arch&gt;/</code></td><td>RPM 仓库（含 repodata）</td></tr>
<tr><td><code>arch/&lt;arch&gt;/</code></td><td>Pacman 仓库（asus-numpad.db）</td></tr>
<tr><td><code>alpine/&lt;arch&gt;/</code></td><td>Alpine 仓库（APKINDEX）</td></tr>
</table>
<p class="sub">每个生态只保留最近 ${KEEP} 个版本；更早的版本仍可从 GitHub Releases 取。</p>
</body>
</html>
HTML

log "仓库已生成于 $OUT"
find "$OUT" -maxdepth 2 -type d | sort | sed 's/^/  /'
