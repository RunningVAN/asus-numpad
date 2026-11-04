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
手动三档与跟随共用同一套换算（`bright N` ≡ 键盘 3 档时的第 N 档），
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
| `packaging/` | deb / rpm / Arch 打包脚本 |

## 安装

```bash
# Fedora / RHEL
sudo dnf install ./asus-numpad-g614-1.0.0-22.fc44.x86_64.rpm
# Debian / Ubuntu
sudo apt install ./asus-numpad-g614_1.0.0-22_amd64.deb
# Arch / Manjaro
sudo pacman -U asus-numpad-g614-1.0.0-22-1-x86_64.pkg.tar.zst
```

包按机型拆分，装错会拒启动（退出码 3）。先看机型：

```bash
cat /sys/class/dmi/id/product_name      # 例如 G634JY → 装 g614 包
```

源码安装：

```bash
cargo build --release --locked
sudo install -Dm755 target/release/asus-numpad /usr/bin/asus-numpad
sudo install -Dm644 packaging/common/asus-numpad.service /usr/lib/systemd/user/
sudo install -Dm644 packaging/common/99-asus-numpad.rules /usr/lib/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=hidraw
systemctl --user enable --now asus-numpad
```

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
| `manual_brightness_step` | `0`..`3` | 手动档，仅跟随关闭时生效 |
| `brightness_levels` | 数组 | 1..8 档下发值，默认 `0x41`..`0x48` |
| `brightness_poll_ms` | 毫秒 | 轮询间隔，默认 `500` |
| `allow_unverified_layout` | `true` / `false` | 放行未真机验证的同族布局 |

未写入的键按官方默认值生效。

## 机型

| 包 | 机型 |
| --- | --- |
| `g533` | 枪神6（G533） |
| `g614` | 枪神7 / 魔霸新锐 2023 / 超竞版 16"（G614、G634）—— 真机验证基准 |
| `g615` | 枪神9/10 16"、魔霸新锐 2025、ProArt P16（G615、G635、H7606） |
| `gx550` | 冰刃双屏 15/16"（GX550、GX551、GX650） |
| `ux582` | 灵耀双屏（UX582、UX482） |

一个包 = 一个触控板模组族，不是一个整机 SKU；被合并的旧包名保留在 `Obsoletes` 里，
升级自动接管。机型包之间互相 `Conflicts`。

未真机验证的机型（`gx550`、`ux582`、`g615` 的 H7606）按「探测不到就拒启动」处理，
不会误按键。想推进就在机器上跑 `asus-numpad check` 把模组名反馈回来。

跨机型细节、验证方法与证据见 [docs/machines.md](docs/machines.md)。

## 构建

```bash
./packaging/build.sh                # 按当前发行版自动出包
./packaging/build.sh deb rpm        # 指定目标
./packaging/build-skus.sh g614   # 只出指定机型
```

静态链接构建（跨发行版分发）：

```bash
ASUS_NUMPAD_SKU=g614 RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes" \
  cargo build --target x86_64-unknown-linux-musl --release
```

打包细节见 [packaging/README.md](packaging/README.md)。

## 已知差异

1. **亮度映射是实测近似**：键盘 3 档线性归一到数字区 `0x43`/`0x46`/`0x48`（3/6/8 档），
   数字区实际档数取决于面板，可用 `brightness_levels` 裁剪。键盘关到 0 时固定取最低档，
   亮度只降不升。
2. **NumLock 长按阈值**：官方 `iNumLockMaxVaildDist = 2741` 在 0.1 mm 单位下仅 0.27 mm，
   与「长按不动」的物理语义矛盾。本实现按字面值使用，若长按被判成滑动，先查这里。
3. **图标子区边界未闭合**：官方布局里 icon 区与计算器热区有重叠描述，本实现按整格归入
   NumLock 触发区。
4. **背光不可回读**：`HIDIOCGFEATURE` 恒返回全 0，灯是否点亮只能看写入报文与驱动状态行。

## 许可

MIT，见 [LICENSE](LICENSE)。
