# 安装

支持 Debian 系的 apt、Fedora/RHEL/openSUSE 的 dnf/zypper、Arch 的 pacman、Alpine 的 apk，
以及 Gentoo、Nix/NixOS 和任意发行版都能用的静态 tarball。

两种取用方式，任选其一：

- **走软件源**（推荐）：加一条仓库配置，之后 `apt upgrade` / `dnf upgrade` / `pacman -Syu` 跟着升级。
- **直接装包**：从 [Releases](https://github.com/RunningVAN/asus-numpad/releases) 下载本机格式安装一次，升级要自己重复。

包按机型拆分，装错会拒绝启动（退出码 3）。装之前先确认机型：

```bash
cat /sys/class/dmi/id/product_name    # 例如 G634JY → 装 g614 包
```

机型与包名的对应关系见 [README 机型表](../README.md#机型)，模组与键表的对应见
[machines.md](machines.md)。拿不准就装 `asus-numpad`（兼容包，不校验机型）。

## 软件源

仓库地址是 `https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo`，
由 CI 在每次发版后自动重建，每个生态保留最近 3 个版本。仓库未做 GPG 签名——
密钥轮换的成本和收益不成比例；索引里的哈希校验值齐全，介意完整性的请改用 Release
里的包手工安装。

### 源里放的是哪个包

同一次发版会为多个发行版各编一份，但**仓库里只收一份「基线包」**——取编译环境
glibc 最老的那个，这样一条源能覆盖整条线，不会出现「新版能装、老版报
`GLIBC_2.38 not found`」：

| 生态 | 源里的基线 | 由哪编出 | 覆盖范围 |
| --- | --- | --- | --- |
| apt | `asus-numpad_<ver>~jammy_amd64.deb` | Ubuntu 22.04 | Ubuntu 22.04 / 24.04、Debian 12 / 13、Mint 21+ |
| rpm | `asus-numpad-<ver>-1.el9.x86_64.rpm` | AlmaLinux 9 | RHEL / Rocky / Alma 9 与 10、Fedora、openSUSE |
| pacman | `asus-numpad-<ver>-1-x86_64.pkg.tar.zst` | Arch | Arch / Manjaro / EndeavourOS |
| apk | `asus-numpad-<ver>-r0.apk` | Alpine | Alpine 3.19+ |

想用**本发行版原生编译**的包（比如 bookworm / trixie / noble / fedora / openSUSE
各自的构建），从 Release 页面下带发行版后缀的那一份，文件名里的 `~jammy`、`~noble`、
`~trixie`、`~bookworm`、`.el9`、`.fc44`、`.suse` 就是编译环境标记。它们和基线包
是同一个软件，只是编译时用的 C 库版本不同。

### Debian / Ubuntu / Mint

```bash
echo "deb [trusted=yes] https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/asus-numpad.list
sudo apt update
sudo apt install asus-numpad          # 或 asus-numpad-g614 等机型包
```

`trusted=yes` 表示跳过 GPG 校验；`apt` 仍会按 `Release` 里的 SHA256 校验包体。

### Fedora / RHEL / Rocky / Alma / openSUSE

```bash
sudo tee /etc/yum.repos.d/asus-numpad.repo >/dev/null <<'EOF'
[asus-numpad]
name=asus-numpad
baseurl=https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/rpm/$basearch
enabled=1
gpgcheck=0
EOF
sudo dnf install asus-numpad
```

openSUSE 把 `dnf` 换成 `zypper install asus-numpad` 即可，仓库文件通用
（`/etc/zypp/repos.d/` 下同样识别这种 `.repo`）。源里放的这份是 el9 基线包，
Tumbleweed / Leap 上照装（只依赖 glibc）；要 openSUSE 原生构建的那份，去
Releases 下 `.suse.x86_64.rpm`。

装之前不必自己验一遍：每次发版 CI 都会在 9 个干净容器（Debian 12/13、
Ubuntu 22.04/24.04、AlmaLinux 9、Fedora、openSUSE、Arch、Alpine）里真装一次、
跑一遍 `--version`，有一个装不上就不发版。

### Arch / Manjaro

```bash
sudo tee -a /etc/pacman.conf >/dev/null <<'EOF'

[asus-numpad]
SigLevel = Optional TrustAll
Server = https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/arch/$arch
EOF
sudo pacman -Sy asus-numpad
```

### Alpine

```bash
echo "https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/alpine/$(apk --print-arch)" \
  | sudo tee -a /etc/apk/repositories
sudo apk update
sudo apk add --allow-untrusted asus-numpad
```

## 直接装包

从 Releases 下载对应文件，文件名里的版本号以页面为准：

| 发行版 | 文件 |
| --- | --- |
| Debian / Ubuntu | `asus-numpad_<ver>~<distro>_amd64.deb`（`~jammy` / `~noble` / `~bookworm` / `~trixie`） |
| Fedora / RHEL / openSUSE | `asus-numpad-<ver>-1.<dist>.x86_64.rpm`（`.el9` / `.fc44` / `.suse`） |
| Arch / Manjaro | `asus-numpad-<ver>-1-x86_64.pkg.tar.zst` |
| Alpine | `asus-numpad-<ver>-r0.apk` |
| 任意 x86_64 | `asus-numpad-<ver>-x86_64.tar.gz` |

```bash
sudo apt install ./asus-numpad_1.2.0-1_amd64.deb
sudo dnf install ./asus-numpad-1.2.0-1.fc44.x86_64.rpm
sudo pacman -U asus-numpad-1.2.0-1-x86_64.pkg.tar.zst
sudo apk add --allow-untrusted ./asus-numpad-1.2.0-r0.apk
```

发版同时产出 `SHA256SUMS`，校验方式：

```bash
sha256sum -c asus-numpad-1.2.1-SHA256SUMS --ignore-missing
```

## tarball（任意发行版）

静态链接（musl）的 tarball 不依赖 glibc 版本，理论上任何 x86_64 Linux 都能跑；
包里带安装脚本，不经过包管理器：

```bash
tar xzf asus-numpad-1.2.1-x86_64.tar.gz
cd asus-numpad-1.2.1
sudo ./install.sh --dry-run     # 先看它要做什么
sudo ./install.sh
```

`install.sh` 的参数：

| 参数 | 作用 |
| --- | --- |
| `--prefix /usr/local` | 装到非默认前缀（默认 `/`） |
| `--dry-run` | 只打印动作，不落盘 |
| `--no-service` | 只放文件，不装 systemd 用户单元与 udev 规则 |
| `--keep-config` | 已存在配置文件时不覆盖 |

卸载用同目录的 `uninstall.sh`（`--purge` 连配置一起删）。两者做的事与 deb/rpm 包等价，
区别只是文件不归包管理器记账。

## Gentoo

```bash
sudo mkdir -p /etc/portage/repos.conf
# 用本地 overlay 即可：
sudo mkdir -p /var/db/repos/asus-numpad/sys-apps/asus-numpad
sudo cp packaging/gentoo/asus-numpad-1.2.1.ebuild /var/db/repos/asus-numpad/sys-apps/asus-numpad/
cd /var/db/repos/asus-numpad && sudo repoman manifest 2>/dev/null || sudo pkgdev manifest
sudo emerge -av sys-apps/asus-numpad
```

`asus-numpad-9999.ebuild` 是 live 版，直接跟 main 分支。

## Nix / NixOS

```bash
# 临时跑
nix run github:RunningVAN/asus-numpad

# 装进 profile
nix profile install github:RunningVAN/asus-numpad

# NixOS（flake）
{
  inputs.asus-numpad.url = "github:RunningVAN/asus-numpad";
  # 在某个 nixosSystem 的 modules 里：
  # { imports = [ inputs.asus-numpad.nixosModules.default ];
  #   services.asus-numpad.enable = true; }
}
```

## 源码构建

```bash
git clone https://github.com/RunningVAN/asus-numpad
cd asus-numpad
cargo build --release --locked
sudo install -Dm755 target/release/asus-numpad /usr/bin/asus-numpad
sudo install -Dm644 packaging/common/asus-numpad.service /usr/lib/systemd/user/
sudo install -Dm644 packaging/common/99-asus-numpad.rules /usr/lib/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=hidraw --action=change
sudo udevadm settle
systemctl --user enable --now asus-numpad
```

`udevadm trigger --subsystem-match=hidraw` 这步别省：hidraw 节点在装包前就存在，
`--reload-rules` 不会把新规则重放到既有节点上，漏了这一步会一直 `Permission denied`。

## 装完自检

```bash
asus-numpad check          # 机型、设备、几何换算、背光候选，只读
systemctl --user status asus-numpad
getfacl /dev/hidraw* | grep -E 'user|group'   # 应看到 uaccess 授权
```

`check` 的机型段若报不匹配，会直接列出该装的包名。

## 升级与卸载

| 方式 | 升级 | 卸载 |
| --- | --- | --- |
| apt | `sudo apt upgrade` | `sudo apt remove asus-numpad` |
| dnf / zypper | `sudo dnf upgrade` | `sudo dnf remove asus-numpad` |
| pacman | `sudo pacman -Syu` | `sudo pacman -R asus-numpad` |
| apk | `sudo apk upgrade` | `sudo apk del asus-numpad` |
| tarball | 下新 tarball 重跑 `install.sh` | `sudo ./uninstall.sh` |

配置文件 `~/.config/asus-numpad/asus-numpad.json` 不会随卸载删除，需要重置就手动删掉。
