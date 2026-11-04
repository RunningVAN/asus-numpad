# asus-numpad

在 ASUS 笔记本触摸板上启用数字小键盘（NumberPad）的 Linux 用户态驱动，Rust 实现。

通过 evdev 读触点、uinput 出按键，**不替换内核模块、不改 ACPI 表**，卸载即恢复原状。
算法参数直接取自官方 `AsusNUMPADFilter.inf` 的 `AlgoParameters` 原值，不是拟合出来的。

## 原理

### 1. 数据链路

```
触摸板 ──evdev(/dev/input/eventN)──> 帧缓冲(8ms) ──> 状态机 ──> uinput(/dev/uinput) ──> 系统按键
                                                  └──> hidraw FEATURE 报文 ──> 数字区背光
```

驱动独占触摸板（`grab`），把触点坐标换算成数字区格位，再合成对应的数字键事件交给内核。
系统因此只看到一块普通键盘，与真实 NumPad 无差别。

### 2. 参数来源

官方 INF 中 `AlgoParameters` 共 425 项，本实现逐项对齐关键项：

| 参数 | 值 | 含义 |
| --- | --- | --- |
| 帧长 | 8 ms | 状态机时间基准，长按 62 帧 = 496 ms |
| 长度单位 | 0.1 mm | 布局几何与阈值统一用此单位 |
| 点按时长 | 350 ms | 区分点按与滑动 |
| NumLock 长按 | 62 帧 | 图标区长按切换数字模式 |
| 手势位移阈值 | 4838 µm | 低于此值不算滑动 |
| 双指间距 | 40 mm | 判定双指滚动 |
| 应用手势 | 7000 µm | 计算器热区触发位移 |

### 3. 布局换算

键位不按整机型号区分，而按**触摸板模组 + HID 名**区分。`/sys/class/input/*/name` 中的
`ASUE1416`、`ASCF1400` 之类前缀是模组厂代码，同名模组在各机型上描述符一致，
于是驱动只维护「HID 名 → 键表」映射（`HID_LAYOUTS`，44 条，覆盖官方 `SupportList` 全部模组）。

数字区取触摸板右上角区域，按 5 行 4 列切格，每格中心落在官方 `NUMPADLAYOUT_GH3XLNDZ_00..14`
给定的坐标上（单位 0.1 mm）。坐标→格位→键码的换算全在该模块内完成，可脱离硬件单测。

### 4. 背光协议

数字区背光走 HID FEATURE 报告，与数字键事件是两条独立通道：

```
报文: 0d 14 03 <level> ad      # 6 字节，FEATURE report ID = 0x0d
level: 0x41..0x48 = 1..8 档    # 0x00 = 熄灭，0x01 = 点亮（不指定档位）
下发: HIDIOCSFEATURE           # 无需先发 0x60 解锁
```

「跟随键盘背光」= 读 `/sys/class/leds/asus::kbd_backlight/brightness` 的 `current/max`，
线性归一到 1..8 档，每 500 ms 轮询，仅档位变化时下发，不刷 EC。
手动档位与跟随共用同一张档表的换算（`bright N` 取第 N 个等分点），
因此开关跟随不会造成亮度跳变。

背光节点选择有优先级（`led_policy.rs`）：OMNI 无线接收器 `0b05:1ace` 永久排除，
ELAN `04f3:*` 且带 FEATURE `0x0d` 最优先，其次 ASUS `0b05:*`，最后 i2c 兜底（`0x38` / `0x15`）。

### 5. 权限模型

整条链路不需要 root：udev 规则给 hidraw / event 节点打 `TAG+="uaccess"`，
服务以 `systemd --user` 用户单元随会话启动，`/dev/uinput` 由 systemd 会话授权。

## 模块

| 路径 | 职责 |
| --- | --- |
| `src/main.rs` | CLI 子命令与参数装配 |
| `src/engine.rs` | 官方 Algo 状态机，纯逻辑无系统调用 |
| `src/layout.rs` | 布局几何、44 条 HID→键表映射、坐标换算 |
| `src/config.rs` | 配置默认值、读写、旧路径迁移 |
| `src/brightness.rs` | 键盘背光 → 数字区档位换算 |
| `src/led_policy.rs` | 背光节点打分与选择 |
| `src/sku.rs` | 机型闸门，包与机器对不上则拒启动 |
| `src/i18n.rs` | 三语文案，支持外部 `.lang` 覆盖 |
| `src/linux/` | evdev / uinput / hidraw / i2c 后端 |
| `lang/` | 简中、繁中、英文文案 |
| `packaging/` | deb / rpm / Arch / Alpine / Gentoo / Nix / tarball 打包脚本与仓库工具 |

## 安装

走官方软件源，装一次以后跟着系统升级：

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
```

也可以从 [Releases](https://github.com/RunningVAN/asus-numpad/releases) 直接下本机格式的包
（deb / rpm / pkg.tar.zst / apk / 静态 tarball）。Alpine、Gentoo、Nix/NixOS、tarball
手工安装、升级卸载，见 **[docs/install.md](docs/install.md)**。

包按机型拆分，装错会拒启动（退出码 3）。先看机型：

```bash
cat /sys/class/dmi/id/product_name      # 例如 G634JY → 装 g614 包
```

拿不准就装 `asus-numpad`（兼容包，不校验机型）。

## 用法

```bash
asus-numpad check          # 自检：设备、几何换算、背光候选，只读
asus-numpad dump           # 打印原始事件流
asus-numpad run            # 前台运行（systemd 跑的就是这个）
asus-numpad --dump-config  # 打印生效配置
```

亮度：

```bash
asus-numpad bright 0|1|2|3      # 灭 / 暗 / 中 / 亮（固定档，关闭跟随）
asus-numpad bright up|down|cycle
asus-numpad bright follow on|off
asus-numpad bright status
```

语言：

```bash
asus-numpad lang [zh-CN|zh-TW|en|auto]
asus-numpad --lang en check     # 仅本次生效
```

优先级：`ASUS_NUMPAD_LANG` > `--lang` > 配置 `language` > 系统 locale。

## 配置

`~/.config/asus-numpad/asus-numpad.json`（root 手工运行时写 `/etc/asus-numpad/`）。

| 键 | 取值 | 说明 |
| --- | --- | --- |
| `numlock` | `sync` / `toggle` / `off` | 与系统 NumLock 联动 |
| `calc_gesture` | `true` / `false` | 手势触发计算器 |
| `calc_gesture_mode` | `long` / `off` | 触发方式 |
| `mixed_mode` | `auto` / `on` / `off` | 数字键与光标共存，需要 `/dev/uinput` |
| `grab` | `true` / `false` | 抓取触摸板；关掉后点击会穿透到桌面 |
| `led_kind` | `auto` / `hidraw` / `i2c` / `none` | 背光通道 |
| `follow_keyboard_brightness` | `true` / `false` | 是否跟随键盘背光，默认 `true` |
| `manual_brightness_step` | `0`..`4` | 手动档（0=灭，1..4 线性递增），仅跟随关闭时生效 |
| `manual_brightness_levels` | 数组 | 自定义四档下发值，默认按 `brightness_levels` 线性等分；每项取 `0`（熄灭）或 `0x41`..`0x48`，须从暗到亮严格递增 |
| `brightness_levels` | 数组 | 硬件全档下发值，默认 `0x41`..`0x48`（1..8 档） |
| `brightness_poll_ms` | 毫秒 | 轮询间隔，默认 `500` |
| `allow_unverified_layout` | `true` / `false` | 放行未收录键表的同族布局 |

未写入的键按官方默认值生效。

## 机型

| 包 | 机型 |
| --- | --- |
| `g533` | 枪神6（G533） |
| `g614` | 枪神7 / 魔霸新锐 2023 / 超竞版 16"（G614、G634） |
| `g615` | 枪神9/10 16"、魔霸新锐 2025、ProArt P16（G615、G635、H7606） |
| `gx550` | 冰刃双屏 15/16"（GX550、GX551、GX650） |
| `ux582` | 灵耀双屏（UX582、UX482） |

一个包 = 一个触控板模组族，不是一个整机 SKU；被合并的旧包名保留在 `Obsoletes` 里，
升级自动接管。机型包之间互相 `Conflicts`。

机型号判定用 DMI 的多字段并集（`product_name` / `board_name` / `product_family`），
BIOS 占位串按「读不到」处理、不拦启动；厂商字段（`sys_vendor` / `board_vendor`）明确不是
ASUS 时直接拒绝。装错包时 `check` 会按机型编号最长公共前缀给出该装的包名。

各机型模组与键表的对应关系见 [docs/machines.md](docs/machines.md)。

## 构建

```bash
./packaging/build.sh                        # 按当前发行版自动出包
./packaging/build.sh deb rpm tarball        # 指定目标（deb/rpm/arch/alpine/tarball）
./packaging/build-skus.sh g614              # 只出指定机型
./tools/repo-gen.sh                         # dist/ → 本地仓库 repo/（apt/rpm/pacman/apk）
```

静态链接构建（跨发行版分发）：

```bash
ASUS_NUMPAD_SKU=g614 RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes" \
  cargo build --target x86_64-unknown-linux-musl --release
```

打包细节见 [packaging/README.md](packaging/README.md)。推 `v*` 标签会触发
`.github/workflows/release.yml`：各发行版容器里出包 → 生成四套仓库索引 → 发 Release →
把 `repo/` 提交回 `main`。

## 已知差异

1. **亮度档位由硬件全档表线性等分**：`brightness_levels` 是面板的完整档表
   （默认 `0x41`..`0x48`，8 档），手动档位取其中的等分点——默认 4 档即
   `0x41`/`0x43`/`0x46`/`0x48`（0 档熄灭）。档数与具体下发值都可用
   `manual_brightness_levels` 覆写（上限 `brightness_levels` 的长度）：每项取 `0`
   （熄灭）或 `0x41`..`0x48`，且必须从暗到亮严格递增，否则整表丢弃并回落线性等分。
   想「从全灭到最亮」四档就写 `[0, 0x43, 0x46, 0x48]`；跟随模式下
   键盘背光档位同样归一到这张表上，键盘关到 0 时取最低档，亮度只降不升。
2. **NumLock 长按阈值**：官方 `iNumLockMaxVaildDist = 2741` 在 0.1 mm 单位下仅 0.27 mm，
   与「长按不动」的物理语义矛盾。本实现按字面值使用，若长按被判成滑动，先查这里。
3. **图标子区边界未闭合**：官方布局里 icon 区与计算器热区有重叠描述，本实现按整格归入
   NumLock 触发区。
4. **背光不可回读**：`HIDIOCGFEATURE` 恒返回全 0，灯是否点亮只能看写入报文与驱动状态行。

## 许可

GPL-3.0-or-later（GPLv3 或更高版本），全文见 [LICENSE](LICENSE)。

Copyright (C) 2026 RunningVAN

改协议的原因：本项目的键表与几何换算来自官方驱动逆向和厂商 INF 的实测结论，再分发条件
与 copyleft 一致；同时避免下游把本项目闭源塞进固件工具链后不回馈。
