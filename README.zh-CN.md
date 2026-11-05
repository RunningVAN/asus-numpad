# asus-numpad

[English](README.md) | 简体中文

ASUS NumberPad 的 Linux 用户态驱动。通过 evdev 独占触摸板，把触点换算成小键盘格，通过 uinput
注入按键。不替换内核模块，不打 ACPI 补丁，卸载即恢复原状。

时序与几何常量取自厂商文件 `AsusNUMPADFilter.inf` 中的 `AlgoParameters`。

## 常见问题

### 为什么只支持华硕机型？

代码里没有华硕专属的东西：触摸板走 evdev 读，触点映射到格子，按键走 uinput 注入。要支持别的
触摸板，改的是机型表（DMI 与触摸板 HID 名）和布局常量，不是代码路径。

真正的限制在硬件。NumberPad 触摸板比别家多两样东西：一层能把图案点亮的 LED，以及一套把几何、
时序、手势阈值、背光档位定死的厂商参数。没有那层灯，按下去不知道手指落在哪一格、也不知道数字
模式开没开——键能出，但等于盲打；没有那套参数，轻点、滑动、双指的阈值只能在一块从来没按小键盘
调过的面板上瞎试。所以支持范围只留在两者都具备的机型上，判断依据只有机型表。

### 为什么做成用户态驱动？

- **内核升级不会把它带崩。** 不替换任何自带模块、不动 ACPI 与 i8042，卸载即恢复原状；跟内核
  对着干，正是这块触摸板一直没有 Linux 支持的原因。
- **用厂商原值，不靠猜。** 时序、几何、阈值、背光档位直接取华硕自己驱动文件里的数，状态机行为
  与 Windows 侧一致。
- **一份包覆盖多机型。** 机型靠 DMI 与触摸板 HID 名判别，G614、G634 等同一份包就能用。
- **硬件已经买了。** 在 Linux 上它本来只是一块普通触摸板；小键盘缺的不是功能，是驱动。

## 工作原理

### 数据链路

```
触摸板 --evdev(/dev/input/eventN)--> 帧缓冲(8 ms) --> 状态机 --uinput(/dev/uinput)--> 按键事件
                                                         |
                                                         `--> hidraw FEATURE 报文 --> 小键盘背光
```

### 时序与几何

| 参数 | 取值 | 含义 |
| --- | --- | --- |
| 帧长 | 8 ms | 状态机时基，长按 62 帧（496 ms） |
| 长度单位 | 0.1 mm | 布局几何与阈值的单位 |
| 点击判定 | 350 ms | 区分点按与滑动 |
| NumLock 长按 | 62 帧 | 图标区切换数字模式 |
| 滑动阈值 | 4838 µm | 低于此位移不算滑动 |
| 双指间距 | 40 mm | 双指滚动识别 |
| 唤起手势 | 7000 µm | 计算器热区触发距离 |

### 键表

键表按触摸板模组和 HID 名称选择，不按笔记本型号。`HID_LAYOUTS` 收录 44 条 HID 名称到键表的映射，
覆盖厂商 `SupportList` 中的每个模组；`ASUE1416`、`ASCF1400` 这类模组前缀可在
`/sys/class/input/*/name` 中看到。

小键盘位于触摸板右上角，5 行 4 列。格子中心即 `NUMPADLAYOUT_GH3XLNDZ_00..14` 的坐标，单位
0.1 mm。坐标到格子、格子到键码的换算不含硬件依赖，有单元测试。

### 背光

```
报文: 0d 14 03 <档位> ad   # 6 字节，HID FEATURE 报文 ID 0x0d
档位: 0x41..0x48           # 第 1..8 档
      0x00                 # 关闭
      0x01                 # 开启，沿用当前档位
写入: HIDIOCSFEATURE       # 无需 0x60 解锁
```

`bright follow on` 读取 `/sys/class/leds/asus::kbd_backlight/brightness` 的 `current` 与 `max`，
把比值映射到 1..8 档，每 500 ms 轮询，仅在档位变化时写入。手动档位与跟随模式共用一张档位表。

节点选择（`led_policy.rs`）：始终排除 OMNI 接收器 `0b05:1ace`；ELAN `04f3:*` 且带 FEATURE
`0x0d` 优先，其次 ASUS `0b05:*`，最后 i2c（`0x38` / `0x15`）。

### 权限

不需要 root。udev 给 hidraw 与 event 节点打 `TAG+="uaccess"`，守护进程以 `systemd --user` 单元
运行，`/dev/uinput` 由会话授权。

## 源码结构

| 路径 | 用途 |
| --- | --- |
| `src/main.rs` | CLI 解析与装配 |
| `src/engine.rs` | 厂商算法状态机 |
| `src/layout.rs` | 布局几何、44 条 HID 表、坐标换算 |
| `src/config.rs` | 默认值、读写、旧路径迁移 |
| `src/brightness.rs` | 键盘背光到小键盘档位的映射 |
| `src/led_policy.rs` | 背光节点打分 |
| `src/sku.rs` | 机型闸门 |
| `src/i18n.rs` | 三种语言，可用外部 `.lang` 文件覆盖 |
| `src/linux/` | evdev、uinput、hidraw、i2c 后端 |
| `lang/` | zh-CN、zh-TW、en 文案 |
| `packaging/` | deb、rpm、Arch、Alpine、Gentoo、Nix、tarball 脚本与仓库工具 |

## 安装

```bash
# Debian / Ubuntu
echo "deb [trusted=yes] https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/asus-numpad.list
sudo apt update && sudo apt install asus-numpad

# Fedora / RHEL / Rocky / Alma
sudo tee /etc/yum.repos.d/asus-numpad.repo >/dev/null <<'EOF'
[asus-numpad]
name=asus-numpad
baseurl=https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/rpm/$basearch
enabled=1
gpgcheck=0
EOF
sudo dnf install asus-numpad

# openSUSE Leap / Tumbleweed
sudo zypper addrepo --no-gpgcheck --refresh \
  "https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/rpm/$(uname -m)" asus-numpad
sudo zypper install asus-numpad

# Arch / Manjaro
sudo tee -a /etc/pacman.conf >/dev/null <<'EOF'

[asus-numpad]
SigLevel = Optional TrustAll
Server = https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/arch/$arch
EOF
sudo pacman -Sy asus-numpad

# Arch / Manjaro，源码包 asus-numpad-rs
# 把 release 页面的 PKGBUILD 与 asus-numpad-rs.install 放在同一目录：
makepkg -si
```

每个生态一个包。deb 在 Ubuntu 22.04 上构建，覆盖 22.04 至 24.04 与 Debian 12、13。rpm 在
AlmaLinux 9 上构建，覆盖 RHEL、AlmaLinux、Rocky 9 至 10、Fedora、openSUSE Leap 16 与
Tumbleweed：spec 只用标准宏，运行时只依赖 glibc 与 systemd，仓库是 rpm-md，zypper 直接读，
不按发行版各编一份。原生包也在
[Releases](https://github.com/RunningVAN/asus-numpad/releases)：deb、rpm、pkg.tar.zst、带
.SRCINFO 的 PKGBUILD、apk、静态 tarball。Alpine、Gentoo、NixOS 与手动 tarball 步骤见
[docs/install.md](docs/install.md)。

CI 在每个目标发行版的干净容器里安装每个包并执行 `--version`，装不上的包会挡住发布。

机型包与机器不符时以退出码 3 拒绝。先确认机型代码：

```bash
cat /sys/class/dmi/id/product_name      # 例如 G634JY，对应 g614 包
```

`asus-numpad` 是不做机型检查的包。运行依赖：glibc，Alpine 上为 musl。

## 使用

```bash
asus-numpad check          # 只读自检：设备、布局换算、背光候选
asus-numpad dump           # 原始事件流
asus-numpad run            # 前台运行，由 systemd 单元启动
asus-numpad --dump-config  # 当前生效配置
```

```bash
asus-numpad bright 0|1|2|3      # 关、低、中、高（固定档位，关闭跟随）
asus-numpad bright up|down|cycle
asus-numpad bright follow on|off
asus-numpad bright status
```

```bash
asus-numpad lang [zh-CN|zh-TW|en|auto]
asus-numpad --lang en check     # 仅本次生效
```

语言优先级：`ASUS_NUMPAD_LANG`、`--lang`、配置里的 `language`、系统 locale。

## 配置

`~/.config/asus-numpad/asus-numpad.json`，root 运行时为 `/etc/asus-numpad/`。未写入的键取厂商默认值。

| 键 | 取值 | 含义 |
| --- | --- | --- |
| `numlock` | `sync` / `toggle` / `off` | 跟随系统 NumLock 状态 |
| `calc_gesture` | `true` / `false` | 手势唤起计算器 |
| `calc_gesture_mode` | `long` / `off` | 触发方式 |
| `mixed_mode` | `auto` / `on` / `off` | 按键与指针并存，需要 `/dev/uinput` |
| `grab` | `true` / `false` | 独占触摸板；关闭后点击交给桌面 |
| `led_kind` | `auto` / `hidraw` / `i2c` / `none` | 背光通道 |
| `follow_keyboard_brightness` | `true` / `false` | 跟随键盘背光，默认 `true` |
| `manual_brightness_step` | `0`..`4` | 当前手动档（0 为关闭），跟随关闭时生效 |
| `manual_brightness_levels` | 数组 | 档位到取值的表，默认 `[0, 0x43, 0x45, 0x48]`；元素为 `0` 或 `0x41`..`0x48`；长度 1 到 8 即档位数；允许相邻相等；递减的表被丢弃 |
| `brightness_levels` | 数组 | 硬件档位，默认 `0x41`..`0x48` |
| `brightness_poll_ms` | 毫秒 | 轮询间隔，默认 `500` |
| `allow_unverified_layout` | `true` / `false` | 接受已知家族中未收录的布局 |

## 支持机型

| 包名 | 机型 |
| --- | --- |
| `g533` | 枪神 15 / Strix G15（2022）：`G533`、`G513` |
| `g614` | 枪神 16 / Strix G16（2023）：`G614`、`G634` |
| `g615` | 枪神 16 / Strix G16（2025 及以后）：`G615`、`G635` |
| `gx550` | 冰刃 Duo 15 与 16：`GX550`、`GX551`、`GX650` |
| `ux582` | 灵耀 Duo：`UX582`、`UX482` |

**实测机型只有 `G634`**（枪神 16 2023）。表中其余机型依据触摸板模组、键表与厂商资料支持，
均未上真机验证，属理论支持。其他机型的实测反馈欢迎提交。

DMI 中的机型代码为准，市场名仅供对照。一个包服务一个触摸板模组家族，不是单个 SKU。合并过的
包名留在 `Obsoletes` 中，机型包之间声明 `Conflicts`。

### 不含小键盘的机型

这些机器的触摸板里没有虚拟小键盘。驱动与 `packaging/install-sku.sh` 以退出码 `4` 拒绝并给出原因。

| 机型 | 原因 |
| --- | --- |
| ProArt P16 `H7606` | 触摸板为 ASUS DialPad；规格书写的是 DialPad，用旋钮 HID usage 而非背光 FEATURE `0x0d` |
| 枪神 17 英寸 `G713` | 键盘自带物理小键盘；规格书写 `NumberPad` 为 `N/A` |
| 18 英寸 Plus：`G834`、`G835`、`G814`、`G815`、`G813` | 键盘自带物理小键盘 |

### 机型识别

识别取 DMI 的 `product_name`、`board_name`、`product_family` 三者的并集。BIOS 占位字符串按缺失
处理，不阻塞启动；明确不是 ASUS 的厂商字段会阻塞启动。装了不匹配的包时，`check` 会打印与最长
公共机型前缀相符的包名。

触摸板模组名（如 `ASUE1416`，来自 i2c-hid 与 HID）是第二判据，在 DMI 没有可用机型时启用。DMI
给出已知机型但不属于本包时，不会被模组名覆盖。各机型的模组与键表见
[docs/machines.md](docs/machines.md)。

## 构建

```bash
./packaging/build.sh                        # 当前发行版
./packaging/build.sh deb rpm tarball        # 指定目标（deb/rpm/arch/alpine/tarball）
./packaging/build-skus.sh g614              # 单个机型
./tools/repo-gen.sh                         # dist/ 生成本地仓库到 repo/（apt/rpm/pacman/apk）
```

```bash
ASUS_NUMPAD_SKU=g614 RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes" \
  cargo build --target x86_64-unknown-linux-musl --release
```

打包细节见 [packaging/README.md](packaging/README.md)。打 `v*` 标签会触发
`.github/workflows/release.yml`，在按发行版划分的容器里构建、生成四套仓库索引、发布 release 并把
`repo/` 提交回 `main`。

## 许可

GPL-3.0-or-later，见 [LICENSE](LICENSE)。

Copyright (C) 2026 RunningVAN
*（内容由AI生成，仅供参考）*
