# asus-numpad

[English](README.md) | 简体中文

ASUS NumberPad 的 Linux 用户态驱动。通过 evdev 独占触摸板，把触点换算成小键盘格，通过 uinput
注入按键。不替换内核模块，不打 ACPI 补丁，卸载即恢复原状。

时序与几何常量取自厂商文件 `AsusNUMPADFilter.inf` 中的 `AlgoParameters`。

## 常见问题

### 为什么只支持华硕机型？

驱动不依赖华硕私有接口：输入取自触摸板的 evdev 节点，按键经 uinput 输出，不涉及厂商内核模块与
ACPI 对象，任意多点触控板走的是同一条代码路径。移植工作集中在替换机型条目（DMI 的
`product_name` 加触摸板 HID 名）与布局常量（格心坐标、各阈值）。

无法移植的是硬件。NumberPad 面板提供两样其他 OEM 触摸板通常没有的能力：

- **可寻址背光。** 落指格位与数字模式状态由 LED 图案指示，经 HID FEATURE 报文 `0x0d`
  （`[0d 14 03 <v> ad]`，`0x41`..`0x48` 为亮度档）驱动。没有这层背光，按键仍能输出，但无法指示
  手指所在格位，也无法指示数字模式是否开启。
- **出厂标定表。** `AsusNUMPADFilter.inf` 中的 `AlgoParameters` 固定了帧长（8 ms）、几何单位
  （0.1 mm）、轻点窗口（350 ms）、NumLock 长按（62 帧）、滑动阈值（4838 µm）与双指间距
  （40 mm）。没有这张表，这些值只能在从未按小键盘调校过的面板上拟合，轻点与滑动识别无法与
  Windows 驱动对齐。

因此支持范围限定在同时具备两者的机型，判定入口只有 `src/sku.rs`。面板若具备这两项能力，只需
改机型表与布局常量即可接入，欢迎提交补丁。

### 为什么做成用户态驱动而不是内核模块？

- 面板经 ELAN i2c-hid（`04F3:3275` / `ASUE1416`）接入，不在华硕 `0B05` 键盘控制器之下，现有
  内核模块覆盖不到它，接入也不需要替换任何模块。
- 背光控制是触摸板自身 hidraw 节点上的 HID FEATURE 报文，用户态进程即可下发，不必进入内核。
- 时序、几何与阈值逐项取自厂商 INF，状态机行为与 Windows 驱动一致，机型差异集中在一张表里。
- 启动路径没有改动。卸载即移除 service unit 与 udev 规则，恢复原状，内核升级不会带来回归。

### 会考虑 PR 进 asusctl 吗？

不会。

- 只有少数华硕机型搭载这块触摸板。
- 机型匹配的包装上后几乎没有需要调节的东西，小键盘直接可用，合入 asusctl 对其余用户收益很小。
- 维护者没有精力适配 asusctl 的架构与发布节奏。

两条路都欢迎：向本项目提交补丁，或在 asusctl 侧实现这部分功能。

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
