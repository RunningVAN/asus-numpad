# asus-numpad

[English](README.md) | 简体中文

把 ASUS 笔记本触摸板当数字小键盘（NumberPad）用的 Linux 用户态驱动。驱动通过 evdev 抓取
触摸板，把触点位置换算成数字区格位，再用 uinput 注入按键。不替换内核模块，不改 ACPI 表，
卸载后恢复原状。

时间与几何参数取自厂商文件 `AsusNUMPADFilter.inf` 里的 `AlgoParameters`。

## 工作原理

### 数据链路

```
触摸板 --evdev(/dev/input/eventN)--> 帧缓冲(8 ms) --> 状态机 --uinput(/dev/uinput)--> 按键事件
                                                        |
                                                        `--> hidraw FEATURE 报文 --> 数字区背光
```

触摸板被独占抓取。触点换算成格位，格位换算成键码，系统只看到一块普通键盘。

### 时间与几何

| 参数 | 取值 | 含义 |
| --- | --- | --- |
| 帧长 | 8 ms | 状态机时间基准，长按为 62 帧（496 ms） |
| 长度单位 | 0.1 mm | 布局几何与阈值统一使用 |
| 点按时长 | 350 ms | 区分点按与滑动 |
| NumLock 长按 | 62 帧 | 在图标区切换数字模式 |
| 滑动阈值 | 4838 µm | 低于此位移不判为滑动 |
| 双指间距 | 40 mm | 判定双指滚动 |
| 计算器手势 | 7000 µm | 触发热区所需位移 |

### 键位换算

键表按触摸板模组加 HID 名区分，不按整机型号区分。`/sys/class/input/*/name` 里的
`ASUE1416`、`ASCF1400` 是模组厂代码，同一模组在各机型上的描述符一致，因此驱动只维护一张
「HID 名 → 键表」映射（`HID_LAYOUTS`，44 条，覆盖厂商 `SupportList` 中的全部模组）。

数字区位于触摸板右上角，按 5 行 4 列切格，格心坐标取自 `NUMPADLAYOUT_GH3XLNDZ_00..14`
（单位 0.1 mm）。坐标、格位与键码的换算在模块内部完成，可脱离硬件做单元测试。

### 背光

数字区背光与按键事件是两条独立通道：

```
报文: 0d 14 03 <level> ad   # 6 字节，HID FEATURE 报告 ID 为 0x0d
取值: 0x41..0x48            # 1..8 档
      0x00                  # 熄灭
      0x01                  # 点亮，保持当前档
下发: HIDIOCSFEATURE        # 无需先发 0x60 解锁
```

`bright follow on` 读取 `/sys/class/leds/asus::kbd_backlight/brightness` 的 `current` 与
`max`，按比例归一到 1..8 档，每 500 ms 轮询一次，仅档位变化时下发。手动档与跟随模式共用
同一张档表，开关跟随不会造成亮度跳变。

节点选择见 `led_policy.rs`：OMNI 无线接收器 `0b05:1ace` 始终排除，ELAN `04f3:*` 且带
FEATURE `0x0d` 优先，其次是 ASUS `0b05:*`，最后是 i2c（`0x38` / `0x15`）。

### 权限

整条链路不需要 root。udev 规则给 hidraw 与 event 节点打 `TAG+="uaccess"`，服务以
`systemd --user` 单元随会话启动，`/dev/uinput` 由会话授权。

## 代码结构

| 路径 | 职责 |
| --- | --- |
| `src/main.rs` | 命令行解析与装配 |
| `src/engine.rs` | 厂商算法状态机，纯逻辑 |
| `src/layout.rs` | 布局几何、44 条 HID 到键表映射、坐标换算 |
| `src/config.rs` | 默认值、读写、旧路径迁移 |
| `src/brightness.rs` | 键盘背光到数字区档位的换算 |
| `src/led_policy.rs` | 背光节点打分与选择 |
| `src/sku.rs` | 机型闸门，机型不符时拒绝启动 |
| `src/i18n.rs` | 三语文案，可用外部 `.lang` 覆盖 |
| `src/linux/` | evdev、uinput、hidraw、i2c 后端 |
| `lang/` | 简中、繁中、英文文案 |
| `packaging/` | deb、rpm、Arch、Alpine、Gentoo、Nix、tarball 打包脚本与仓库工具 |

## 安装

加上软件源，之后随系统升级：

```bash
# Debian / Ubuntu
echo "deb [trusted=yes] https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/asus-numpad.list
sudo apt update && sudo apt install asus-numpad

# Fedora / RHEL / openSUSE
sudo tee /etc/yum.repos.d/asus-numpad.repo >/dev/null <<'EOF'
[asus-numpad]
name=asus-numpad
baseurl=https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/rpm/$basearch
enabled=1
gpgcheck=0
EOF
sudo dnf install asus-numpad

# Arch / Manjaro
sudo tee -a /etc/pacman.conf >/dev/null <<'EOF'

[asus-numpad]
SigLevel = Optional TrustAll
Server = https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/arch/$arch
EOF
sudo pacman -Sy asus-numpad

# Arch / Manjaro，源码包 asus-numpad-rs
# 把 Release 里的 PKGBUILD 与 asus-numpad-rs.install 放在同一目录：
makepkg -si
# AUR 收录后：
yay -S asus-numpad-rs
```

每个生态只出一份包。deb 在 Ubuntu 22.04 上构建，覆盖 22.04 至 24.04 以及 Debian 12、13；
rpm 在 AlmaLinux 9 上构建，覆盖 RHEL、AlmaLinux、Rocky 9 至 10，以及 Fedora 和 openSUSE。

也可以在 [Releases](https://github.com/RunningVAN/asus-numpad/releases) 下载本机格式的包
（deb、rpm、pkg.tar.zst、PKGBUILD 与 .SRCINFO、apk、静态 tarball）。Alpine、Gentoo、NixOS、
AUR、tarball 的手工安装、升级与卸载见 [docs/install.md](docs/install.md)。

发版前，CI 会在每个目标发行版的干净容器里安装一遍并运行 `--version`，装不上则不发版。

包按机型拆分，装错机型会拒绝启动（退出码 3）。先确认型号：

```bash
cat /sys/class/dmi/id/product_name      # 例如 G634JY，对应 g614 包
```

不确定时装 `asus-numpad`，该包不做机型校验。

运行时只依赖 glibc（Alpine 为 musl），不拉入 Rust 运行时。

## 用法

```bash
asus-numpad check          # 只读自检：设备、布局换算、背光候选
asus-numpad dump           # 打印原始事件流
asus-numpad run            # 前台运行，systemd 单元启动的就是这个
asus-numpad --dump-config  # 打印生效配置
```

亮度：

```bash
asus-numpad bright 0|1|2|3      # 灭、暗、中、亮（固定档，关闭跟随）
asus-numpad bright up|down|cycle
asus-numpad bright follow on|off
asus-numpad bright status
```

语言：

```bash
asus-numpad lang [zh-CN|zh-TW|en|auto]
asus-numpad --lang en check     # 仅本次生效
```

优先级：`ASUS_NUMPAD_LANG`、`--lang`、配置项 `language`、系统 locale。

## 配置

`~/.config/asus-numpad/asus-numpad.json`，root 直接运行时读写 `/etc/asus-numpad/`。

| 键 | 取值 | 说明 |
| --- | --- | --- |
| `numlock` | `sync` / `toggle` / `off` | 与系统 NumLock 联动 |
| `calc_gesture` | `true` / `false` | 手势触发计算器 |
| `calc_gesture_mode` | `long` / `off` | 触发方式 |
| `mixed_mode` | `auto` / `on` / `off` | 数字键与光标共存，需要 `/dev/uinput` |
| `grab` | `true` / `false` | 抓取触摸板；关闭后点击会穿透到桌面 |
| `led_kind` | `auto` / `hidraw` / `i2c` / `none` | 背光通道 |
| `follow_keyboard_brightness` | `true` / `false` | 跟随键盘背光，默认 `true` |
| `manual_brightness_step` | `0`..`4` | 当前手动档（0 为熄灭），仅在关闭跟随时生效 |
| `manual_brightness_levels` | 数组 | 档位到取值表，默认 `[0, 0x43, 0x45, 0x48]`。每项可为 `0` 或 `0x41`..`0x48`；长度 1 至 8 决定档数；允许相邻档相同；递减的表会被丢弃并回落默认值 |
| `brightness_levels` | 数组 | 硬件档位表，默认 `0x41`..`0x48` |
| `brightness_poll_ms` | 毫秒 | 轮询间隔，默认 `500` |
| `allow_unverified_layout` | `true` / `false` | 放行同族中未收录的布局 |

未写入的键按厂商默认值生效。

## 支持的机型

| 包 | 机型 |
| --- | --- |
| `g533` | 枪神6、魔霸6 15 英寸（2022）：`G533`、`G513` |
| `g614` | 枪神7、魔霸新锐 2023（2023）：`G614`、`G634` |
| `g615` | 枪神9、枪神10、魔霸新锐 2025（2025 及以后）：`G615`、`G635` |
| `gx550` | 冰刃双屏 15 / 16 英寸：`GX550`、`GX551`、`GX650` |
| `ux582` | 灵耀双屏：`UX582`、`UX482` |

以 DMI 中的机型编号为准，营销名仅作参考。

一个包对应一个触摸板模组族，而不是一个整机 SKU。被合并的旧包名保留在 `Obsoletes` 中，
升级时自动接管；机型包之间互相 `Conflicts`。

### 没有 NumberPad 的机型

这些机器的触摸板里没有虚拟数字键盘，装任何包都无效。驱动与 `packaging/install-sku.sh`
会拒绝启动并说明原因，退出码为 `4`。

| 机型 | 原因 |
| --- | --- |
| ProArt P16 `H7606` | 触摸板是 ASUS DialPad 智能旋钮，规格页标注 DialPad，走旋钮的 HID 用法，与数字区背光 FEATURE `0x0d` 无关 |
| 枪神 G17 `G713`，17 英寸 | 键盘自带物理数字区，规格页中 `NumberPad` 为 `N/A` |
| 18 英寸 Plus：`G834`、`G835`、`G814`、`G815`、`G813` | 键盘自带物理数字区 |

### 机型判定

判定使用 DMI 的 `product_name`、`board_name`、`product_family` 三个字段的并集。BIOS 占位串
按缺失处理，不阻断启动；厂商字段（`sys_vendor`、`board_vendor`）明确不是 ASUS 时拒绝启动。
装错包时，`check` 会按机型编号的最长公共前缀给出应装的包名。

硬件是第二判据。驱动会读取触摸板模组名（如 `ASUE1416`，来自 i2c-hid 与 HID），当 DMI 读不出
有效型号时用它识别机器；当 DMI 给出了明确但不属于本包的型号时，硬件判据不会推翻该结论。

各机型的模组与键表对应关系见 [docs/machines.md](docs/machines.md)。

## 构建

```bash
./packaging/build.sh                        # 按当前发行版出包
./packaging/build.sh deb rpm tarball        # 指定目标（deb/rpm/arch/alpine/tarball）
./packaging/build-skus.sh g614              # 只出指定机型
./tools/repo-gen.sh                         # dist/ 生成本地仓库 repo/（apt/rpm/pacman/apk）
```

跨发行版分发用的静态构建：

```bash
ASUS_NUMPAD_SKU=g614 RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes" \
  cargo build --target x86_64-unknown-linux-musl --release
```

打包细节见 [packaging/README.md](packaging/README.md)。推送 `v*` 标签会触发
`.github/workflows/release.yml`：在各自的发行版容器里构建，生成四套仓库索引，发布 Release，
并把 `repo/` 提交回 `main`。

## 已知限制

1. **手动档位取自硬件档表。** `brightness_levels` 列出面板的全部硬件档（默认 `0x41`..`0x48`），
   `manual_brightness_levels` 把手动档映射到这条轴上，默认四档 `[0, 0x43, 0x45, 0x48]`，
   从熄灭到最亮。每项可为 `0` 或 `0x41`..`0x48`，表长决定档数（1 至 8），相邻档允许相同；
   递减的表会被丢弃。跟随模式把键盘背光归一到同一张表上，数字区不会比键盘更亮。
2. **NumLock 长按阈值。** 厂商值 `iNumLockMaxVaildDist = 2741` 在 0.1 mm 单位下只有 0.27 mm，
   与「按住不动」的语义冲突。实现按字面值使用，若长按被判成滑动，先查这里。
3. **图标区边界。** 厂商布局中图标格与计算器热区存在重叠描述，实现按整格归入 NumLock 触发区。
4. **背光不可回读。** `HIDIOCGFEATURE` 恒返回 0，灯状态只能从写入报文与状态行判断。

## 许可

GPL-3.0-or-later，全文见 [LICENSE](LICENSE)。

Copyright (C) 2026 RunningVAN
