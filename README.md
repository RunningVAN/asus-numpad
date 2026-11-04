# asus-numpad —— ASUS 触摸板数字小键盘 Linux 驱动（Rust）

在 ASUS 笔记本触摸板上启用数字小键盘（NumberPad）。用户态实现，通过 evdev 读触点、
uinput 出键，**不替换内核模块、不改 ACPI 表**，卸载即恢复原状。

参数来自官方驱动包 `V17.0.0.27` 中 `AsusNUMPADFilter.inf` 的 `AlgoParameters` 原值，
不是猜的：帧长 8 ms、长度单位 0.1 mm、NumLock 长按 62 帧（496 ms）、手势位移阈值 4838 µm、
点按 350 ms、双指间距 40 mm、应用手势 7000 µm。

## 状态

| 项 | 状态 |
|---|---|
| 逻辑层（布局换算 / 状态机 / 点灯联动） | 离线用例覆盖，含官方参数逐项断言 |
| 硬件层（evdev / uinput / hidraw） | 已在 Fedora 44 真机端到端闭合（1.0.0-19，抓包证据见「真机验证记录」） |
| 亮度控制（跟随键盘 / 手动三档） | `bright` 子命令 + 离线用例覆盖 |
| 打包（deb / rpm / Arch） | Shell 脚本就绪，`packaging/build.sh` 一条命令出包 |

真机上有一处必须先知道的差异：官方 `NUMPAD_09 iNumLockMaxVaildDist = 2741`
在 100 µm 单位下只有 0.27 mm，与「长按不动」的物理语义矛盾。本实现按字面值使用，
若长按被判成滑动、NumLock 切不动，先查这里（见「已知差异」）。

## 机型分包（1.0.0-11 起）

安装包按机型分开，一个机型一个包，包名自带机型：

| 包名 | 覆盖机型 | 官方规格里的触控板 |
|---|---|---|
| `asus-numpad-g533` | ASUS 枪神6（G533） | 带 NumberPad |
| `asus-numpad-g614` | 枪神7 / 魔霸新锐 2023、枪神7 超竞版 16 英寸（G614、G634） | 带 NumberPad（G634 为真机验证基准机；G614 低配 EACX 无此硬件） |
| `asus-numpad-g615` | 枪神9 / 枪神10 16 英寸、魔霸新锐 2025、超竞版、ProArt P16（G615、G635、H7606） | 带 NumberPad（H7606 官方限指定机型） |
| `asus-numpad-gx550` | 冰刃双屏 15 / 16 英寸（GX550、GX551、GX650） | 带 NumberPad 2.0 / 虚拟数字键盘 |
| `asus-numpad-ux582` | 灵耀双屏（UX582、UX482） | 带 NumberPad |

**一个包 = 一个触控板模组族，不是一个整机 SKU。** 同族判据是官方 INF 的 HID 模组名
→ 键表映射（见 `src/layout.rs` 的 `HID_LAYOUTS`）：模组名相同就是同一块触摸板、同一套键表，
没必要为每个 SKU 各出一份二进制。当前合并关系：

| 包 | 吸收的旧包 | 合并依据 |
|---|---|---|
| `g614` | `g634` | 同一块 ELAN 触摸板（04F3:3275 / ASUE1416 / ASUF1416 → GH3XLNDZ），G634JY 真机实测 |
| `g615` | `g635`、`h7606` | PixArt ASCP1400 / ASCF1400 → GH4XLNDZ 同族（需逐键核对） |
| `gx550` | `gx650` | 两代冰刃双屏上游实测同属 gx551 键位族（模组未真机验证） |

被吸收的旧机型包名仍保留在 `Obsoletes` / `Provides` 里，老用户 `dnf upgrade` 会被新包
自动接管，不用先手工卸载；同一台机器依旧只能装一个机型包（包间互相 `Conflicts`）。

双屏与 ProArt 系列（`gx550`、`ux582`、以及 `g615` 里的 H7606）的结论来自官方规格页与整机评测，**未做真机验证**；
其触控板 HID 模组是否落在官方 INF 的 44 个模组内尚未实测，因此这几个包按「探测不到就
拒绝启动」处理——不会误点键盘，但也可能在这几台上直接提示不支持。想推进就在机器上跑
`asus-numpad check` / `asus-numpad dump` 把识别到的触控板模组名发回来，对得上 `src/layout.rs` 的 `HID_LAYOUTS` 即可用。

华硕轻薄本里另有一批带 NumberPad 的机型（灵耀 13/14 常规款 UX425/UX434 等），
本仓库尚未逐一立项，需要时按同样方式加 `SKUS` 一行即可。

18 英寸 Plus 机型（G834 / G835 / G815 等）官方是物理 Num-key、没有虚拟数字键盘，
所以**不出包**；确实需要时用 `asus-numpad-universal` 兜底（不做机型校验）。

判据始终是「触控板能不能变数字键」，不是屏幕尺寸本身：冰刃双屏 15.6 英寸（GX550/GX551）
是副屏机型、键盘下移、触控板在右侧且集成 NumberPad 2.0，所以照出；ProArt 创16（H7604）
是物理数字键区，所以不出；灵耀 14 双屏 UX482 与灵耀 X 双屏 Pro UX582 都是触控板方案，出。

三条与机器无关的行为约定：

1. **包装错机型会直接拒绝启动**。启动时读 `/sys/class/dmi/id/product_name` 与包里写死的机型比对，
   对不上就打印机型信息并以退出码 3 退出，不会静默「装上了但按不出数字」。
   强行运行加 `--allow-sku-mismatch`（或配置 `allow_sku_mismatch: true`）。
2. **机型包互相 Conflicts**，一台机器只能装一个；`dnf install` 另一个机型包时会要求先卸载当前的。
3. **每个包内都有 `/usr/share/asus-numpad/sku.json`**，写明这包服务哪个机型，`rpm -ql` 可查；
   二进制里也编入了包名常量，出包脚本会 grep 自证，防止「包名对了、二进制还是上一台」。

装哪个先看机型：

```bash
cat /sys/class/dmi/id/product_name      # 例如 G634JY
sudo dnf install ./asus-numpad-g614-1.0.0-17.fc44.x86_64.rpm
```

从 1.0.0-10 的通用包升级前，先卸旧包（同名文件会冲突）：

```bash
sudo dnf remove asus-numpad && sudo dnf install ./asus-numpad-g614-1.0.0-17.fc44.x86_64.rpm
```

出全套包（源码树内，Mac/Linux 都能跑）：

```bash
./packaging/build-skus.sh                 # skus.tsv 里全部机型 → ~/Downloads
./packaging/build-skus.sh g614 g615       # 只出指定机型
./packaging/build-skus.sh --universal     # 额外出不做校验的兼容包
```

新增机型要改两处并保持一致：`src/sku.rs` 的 `SKUS` 表（运行期权威）与
`packaging/skus.tsv`（构建清单）；`build-skus.sh` 会校验两边 id 集合，不一致直接报错。

## 安装

### Debian / Ubuntu

```bash
sudo apt install ./asus-numpad_5.0.0_amd64.deb
```

### Fedora / RHEL

```bash
sudo dnf install ./asus-numpad-5.0.0-1.x86_64.rpm
```

### Arch / Manjaro

```bash
sudo pacman -U asus-numpad-5.0.0-1-x86_64.pkg.tar.zst
```

### 从源码装（任何发行版，不依赖包管理器）

```bash
git clone <repo> && cd asus-numpad
cargo build --release --locked
sudo install -Dm755 target/release/asus-numpad /usr/bin/asus-numpad
sudo install -Dm644 packaging/common/asus-numpad.service /usr/lib/systemd/user/asus-numpad.service
sudo install -Dm644 packaging/common/99-asus-numpad.rules /usr/lib/udev/rules.d/99-asus-numpad.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
systemctl --user daemon-reload && systemctl --user enable --now asus-numpad
```

## 使用

```bash
asus-numpad check      # 自检：设备、坐标范围、几何换算、点灯通道，只读
asus-numpad dump       # 打印原始事件流（排查手势识别用）
asus-numpad run   # 前台运行（systemd 跑的就是这个）
asus-numpad --dump-config   # 打印生效配置与官方参数原值
```

### 亮度：手动三档 / 跟随键盘背光

```bash
asus-numpad bright 1        # 暗（默认映射到 0x43）
asus-numpad bright 2        # 中（0x46）
asus-numpad bright 3        # 亮（0x48）
asus-numpad bright 0        # 熄灭
asus-numpad bright up       # 逐档变亮（1→2→3，封顶 3）
asus-numpad bright down     # 逐档变暗（3→2→1，最低 1）
asus-numpad bright cycle    # 三档循环：1→2→3→1
asus-numpad bright follow on   # 回到「跟随键盘背光」
asus-numpad bright follow off  # 固定在当前手动档，不再随键盘变化
asus-numpad bright status        # 查看当前状态（不需要 root）
```

语义要点：

- `bright 1|2|3|0`、`up`、`down`、`cycle` 都是「固定档」，会同时把
  `follow_keyboard_brightness` 置为 `false`——否则键盘背光一变就把手动档覆盖掉。
- 手动三档与「跟随键盘背光」**共用同一套换算**：`bright N` ≡ 键盘 3 档时调到第 N 档。
  默认 8 档表 `0x41`..`0x48` 下即 `0x43` / `0x46` / `0x48`
  （键 1 档 → 0x43，2 档 → 0x46，3 档 → 0x48）；改了 `brightness_levels`，两处同步变化。
  这样 `bright follow on|off` 来回切换不会出现亮度跳变（1.0.0-22 起；此前手动三档均匀铺满全表，
  1 档是 `0x41`，比跟随同档暗两级）。
- 命令写完 `~/.config/asus-numpad/asus-numpad.json`（写前留 `.bak` 备份）后会立即下发一次亮度；
  服务正在运行时自动 `systemctl --user restart asus-numpad` 让新档常驻生效。
- `bright follow on|off` 只切跟随开关，不动手动档位，便于来回对照。
- 写回配置文件时 `brightness_levels` 是整数数组 `[65,66,...]`；手写成 `"0x41"` 字符串也照样能读。
  早期版本只会写十六进制字符串、读的时候解析不了，整份配置会静默退回默认值（跟随开关被重置成 `true`
  就是这么来的），已修复并加了往返用例。

背光亮度跟随键盘背光：读 `/sys/class/leds/asus::kbd_backlight/brightness`
（该路径不存在时自动在 `/sys/class/leds` 下按 `kbd_backlight` 扫描，兼容不同分支的命名），
按 `current/max` 换算到 `0x41`..`0x48`（键盘 1..3 档 → 数字区 `0x43`/`0x46`/`0x48`，即 3/6/8 档），每 500 ms 轮询，
只在档位变化时才下发 FEATURE 报告，不刷 EC。`asus-numpad check` 会打印当前换算结果。

长按触摸板右上角图标区约 496 ms（62 帧 × 8 ms）切换数字小键盘；进入后同一区域按 5 行 4 列映射数字键；
双指在区域外滑动滚动页面；手势可触发计算器。

配置：`~/.config/asus-numpad/asus-numpad.json`（遵循 `XDG_CONFIG_HOME`），普通用户读写自己的配置，
不需要 sudo；root 手工运行时仍写系统级 `/etc/asus-numpad/asus-numpad.json`。

> 从旧版本升级时，若用户级配置还不存在，而 `/etc/asus-numpad/asus-numpad.json`、
> `/etc/asus-numpad.json`、`/etc/rog-numpad.json` 存在，启动时会把内容迁移到用户级路径
> （旧文件保留，可手工删除）；只读回落同样生效，因此升级后原有语言 / 亮度设置不会丢。

```json
{
  "numlock": "sync",
  "calc_gesture": true,
  "calc_gesture_mode": "long",
  "mixed_mode": "auto",
  "grab": true,
  "follow_keyboard_brightness": true
}
```

| 键 | 取值 | 说明 |
|---|---|---|
| `numlock` | `sync` / `toggle` / `off` | 与系统 NumLock 的联动；`sync` 只在 NumLock 已开时点灯、关闭时补按一次 |
| `calc_gesture` | `true` / `false` | 是否启用手势触发计算器 |
| `calc_gesture_mode` | `long` / `off` | 触发方式；`long` 为长按左上图标区（NUMADV 40 帧） |
| `mixed_mode` | `auto` / `on` / `off` | 数字键与光标共存，需要 `/dev/uinput`；`auto` 建不出虚拟设备时回落纯数字键盘 |
| `grab` | `true` / `false` | 抓取触摸板；关掉后数字区点击会穿透到桌面 |
| `led_kind` | `auto` / `hidraw` / `i2c` / `none` | 背光通道，默认 `auto` |
| `follow_keyboard_brightness` | `true` / `false` | 数字区亮度是否跟随键盘背光，默认 `true`（`bright follow on|off` 或 `--no-follow-brightness` 可改） |
| `manual_brightness_step` | `0` / `1` / `2` / `3` | 手动固定档：`0` 熄灭、`1` 暗、`2` 中、`3` 亮，默认 `3`；仅在 `follow_keyboard_brightness=false` 时参与点灯 |
| `keyboard_brightness_dir` | 路径 | 键盘背光 sysfs 目录，默认 `/sys/class/leds/asus::kbd_backlight` |
| `brightness_levels` | 数组 | 1..8 档对应的下发值，默认 `0x41`..`0x48` |
| `brightness_poll_ms` | 毫秒 | 轮询间隔，默认 `500`，仅在数字区点亮时轮询 |

未写入文件的键按官方默认值生效，`asus-numpad --dump-config` 可看到全部生效值。

### 多语言：简体中文 / 繁體中文 / English

```bash
asus-numpad lang                 # 看当前语言、语言来源、可用语言、lang 文件位置
asus-numpad lang zh-TW      # 切成繁體中文，写入配置，下次运行生效
asus-numpad lang zh-CN      # 切成简体中文
asus-numpad lang en         # 切成 English
asus-numpad lang auto       # 跟随系统 locale
asus-numpad --lang en check      # 只让这一次运行说英文，不改配置
ASUS_NUMPAD_LANG=zh-TW asus-numpad bright status   # 临时覆盖，优先级最高
```

语言解析优先级（从高到低）：

1. 环境变量 `ASUS_NUMPAD_LANG`
2. 命令行 `--lang`
3. 配置文件 `language`（`lang` 子命令写的就是这一项）
4. 系统 locale：`LC_ALL` > `LC_MESSAGES` > `LANG` > `LANGUAGE`

`zh`/`zh_CN`/`zh-Hans`/`cn`/`chs` 都算简体，`zh_TW`/`zh-HK`/`zh-Hant`/`tw`/`hk` 都算繁體，
其余（含 `C`、`POSIX`、空值）落到英文。

#### lang 文件（便于翻译）

三份文案随包安装，同时用 `include_str!` 编进二进制，删掉文件也不会出现空白提示：

| 文件 | 语言 |
| --- | --- |
| `/usr/share/asus-numpad/lang/zh_CN.lang` | 简体中文 |
| `/usr/share/asus-numpad/lang/zh_TW.lang` | 繁體中文 |
| `/usr/share/asus-numpad/lang/en.lang` | English |

搜索顺序 `~/.config/asus-numpad/lang/` → `/etc/asus-numpad/lang/` → `/usr/share/asus-numpad/lang/` → 内置，
同级再叠加 `ASUS_NUMPAD_LANG_DIR`。因此**整个文件替换**或**只放想改的几个键**都行，而且不需要 root：

```bash
mkdir -p ~/.config/asus-numpad/lang
cp /usr/share/asus-numpad/lang/zh_CN.lang ~/.config/asus-numpad/lang/
$EDITOR ~/.config/asus-numpad/lang/zh_CN.lang     # 改你关心的行，其余留空即回落内置
```

格式 `键 = 文案`，`#` 开头为注释；值里支持 `\n` 换行、`\t` 制表符、`\s` 空格（行尾空格会被裁掉）：

```
# 数值区亮度
bright.status_step = 手动固定档：{step}（0=灭，1=暗，2=中，3=亮）
```

`{step}`、`{table}` 这类花括号是占位符，翻译时保留原名即可（三语键集与占位符有自动化用例校验）。
缺键时回落顺序：外部文件 → 该语言内置 → 英文内置 → 直接显示键名，所以新加语言不会崩。
新增一门语言只需在 `lang/` 放 `xx.lang`（键可只写一部分），再在 `Lang` 枚举里登记。

## 构建安装包

```bash
./packaging/build.sh          # 按当前发行版自动出包，产物在 dist/
./packaging/build.sh deb rpm  # 指定目标
./packaging/build.sh --check  # 只做打包前自检
```

打包脚本全部是 Bash，和 asus-ctl 走同一套发行版原生工具（`dpkg-deb` / `rpmbuild` / `makepkg`），
没有 Python，也不需要 debhelper。详见 [`packaging/README.md`](packaging/README.md)。

## 跨机型支持（G634 系列及其他 ASUS 机型）

### 为什么同模组天然通用

NumberPad 的键位不是按整机型号区分的，而是按**触摸板模组 + HID 命名**区分的：
`/sys/class/input/*/name` 里的 `ASUE1416 / ASUF1416 / ASUP1416 / ELAN140x / ASCP140x …`
前缀是模组厂代码、后缀是模组序号。同一块 ELAN i2c-hid 触摸板装到不同机型上，
HID 名与 HID 描述符（含 FEATURE 报告 `0x0d`）都一样，所以驱动内部只维护
「HID 名 → 键表」映射（`src/layout.rs` 的 `HID_LAYOUTS`，共 44 条），与整机型号无关。

这 44 条不是拍脑袋列的，是逐个对齐官方驱动 `AsusNUMPADFilter.inf` 里
`Parameters\SupportList` 的 `PTP_0..45`（46 项中 2 项是无线的 `0B05:1879` /
`0B05:1866` 复合设备节点，不走模组名）。**官方名单里出现的每一个模组名都已收录**，
由单元测试 `covers_official_supportlist` 守住，少一个就编译期报错。

已知与 G634 家族同源的模组：

| HID 名 | 键表 | 支持级别 | 说明 |
| --- | --- | --- | --- |
| `ASUE1416` | `GH3XLNDZ` | **已验证** | 本项目真机（G634JY，ELAN 04F3:3275）走的就是这条 |
| `ASUF1416` | `GH3XLNDZ` | **已验证** | 与上者同一份键表、同一模组几何 |
| `ASUE1403` / `ELAN1403` | `GH2XLNDZ` | 同家族（需放行） | 结构相同、几何微差，未逐键实测 |
| `ASCP1400` / `ASCF1400` | `GH4XLNDZ` | 同家族（需放行） | 同上 |

其余 38 条 HID（映射到 `CH*` / `NH*` / `GV*` 等 21 张键表）与 `GH3XLNDZ` 结构不同，
直接套用会**按错键**，驱动会拒绝启动而不是乱按。

### 官方跨机型证据（三份驱动逐项比对）

| 驱动包 | 机型 | 版本 | exe MD5 前 12 位 |
| --- | --- | --- | --- |
| `PrecisionTouchPadNumberPad_ROG_ASUS_X_V17.0.0.27_41455_1.exe` | G533ZW / G614JI / G834JY | 17.0.0.27 | `28becdc340cc` |
| `PrecisionTouchPadNumberPad_ROG_ASUS_J_V17.0.0.27_41390_1.exe` | G615LR | 17.0.0.27 | `1ae02445f738` |
| `PrecisionTouchPadNumberPad_ROG_ASUS_X_V17.0.0.29_48348.exe` | G615LR | 17.0.0.29 | `b23042432cdd` |

三个不同二进制、跨 X/J 两条命名线，解包后 `AsusNUMPADFilter.inf` 的 487 条注册表参数
（`MainParameters` 5、`APSettingParameters` 6、`AlgoParameters` 425、`SupportList` 46、
`DriverParameters` 5）**逐项完全一致，差异 0 条**。这就是「一套算法参数支撑全部机型」
的直接证据，也解释了为什么本驱动只需按 HID 名切键表，不必按机型分叉。

### 支持级别闸门（默认保守）

| 级别 | 行为 |
| --- | --- |
| `verified` | 直接启动 |
| `family` | 默认拒绝启动，提示加 `--allow-unverified` |
| `unsupported` | 默认拒绝启动，提示反馈 `asus-numpad check` 输出 |

放行方式（只对 `family` 有意义，`unsupported` 不建议放行）：

```bash
asus-numpad --allow-unverified run        # 本次运行
# 或写进 /etc/asus-numpad/asus-numpad.json
{"allow_unverified_layout": true}
```

### 在新机型上自检（先做这一步）

```bash
asus-numpad check
```

`check` 会额外打印「背光候选节点」段，把每个 `/dev/hidraw*` 的
`vid:pid`、是否带 FEATURE `0x0d`、优先级分值 `rank`、以及驱动最终会选谁列出来，
并给出 i2c 兜底探测结果（`0x38` 优先、`0x15` 次之）：

```
背光候选节点（按优先级排序）:
  /dev/hidraw3  04f3:3275  FEATURE0x0d=是  rank=0  [驱动会选中]
  /dev/hidraw5  0b05:19b6  FEATURE0x0d=否  rank=4  [备选]
  /dev/hidraw4  0b05:1ace  FEATURE0x0d=否  rank=255  [跳过：无线接收器]
背光 i2c: 未发现 0x38/0x15 设备
```

判定顺序（`src/led_policy.rs`，纯逻辑、可跨平台测试）：

1. `0b05:1ace`（OMNI 无线接收器）**永久排除**，写它无效且有副作用；
2. ELAN `04f3:*` 且带 FEATURE `0x0d` → 最优先（`rank 0`）；
3. ASUS `0b05:*` 且带 FEATURE `0x0d` → 次优；
4. ELAN `04f3:*` 已知 NumberPad PID（`0x3275` 等）无 FEATURE → 再降一档；
5. 其他 ELAN / ASUS 节点 → 兜底；`rank ≥ 2` 提示“无描述符证据”，`rank = 4` 额外提示“回落到 ASUS 通用节点，可能点不亮”。

若某机型 NumberPad 触摸板是**新 PID**，把它加进 `src/led_policy.rs` 的
`KNOWN_NUMPAD_PIDS` 即可进入第 4 档；若是**新模组**，按
`evidence/drivers/*_unpacked/**/AsusNUMPADFilter.inf` 里的 `HID_DEVICE_NAME`
补一条 `HID_LAYOUTS` 映射，再按上游布局文件补一张键表。

### 真机验证记录（1.0.0-19，G634JY / Fedora 44）

真机：ROG Strix SCAR（G634JY），触摸板 `ASUE1416:00 04F3:3275`，背光节点 `/dev/hidraw4`。

**方法**：手按真触摸板就没法同时抓报文，因此用 `uinput` 造一块参数相同的虚拟触摸板
（同名 device、同 `04F3:3275`、同分辨率几何），交由驱动的状态机处理；同时在服务进程上挂
`ptrace` 拦截对 `/dev/hidraw4` 的 `write`，记录真正发到硬件的字节。

**结果**：

| 动作 | 观测到的事实 | 说明 |
|---|---|---|
| 长按图标区 (3511,379) 共 90 帧 | 状态机 `numlock → fired → enabled=true`；内核级抓到 `write(/dev/hidraw4, "0d 14 03 01 ad", 5) = 5` | 长按判定、FEATURE 下发链路通 |
| 同上（键盘背光 3 档时） | 抓到 `0d 14 03 48 ad` | 键盘背光 → 数字区 8 档归一化生效 |

**本次未覆盖**：同一注入脚本里的第二次长按没有被识别成关闭，没抓到 `0d 14 03 00 ad`；
`uinput` 造出的帧序列与真实手指仍有差异，因此「再次长按关闭 / 熄灯」这一路径没有自动化复现，
需人工在真机上长按确认（1.0.0-19 阶段手工长按观察到状态行显示「背光熄灭 / 数字键盘关闭」）。

**回读局限（如实披露）**：该 FEATURE 报告用 `HIDIOCGFEATURE` 回读恒为全 0，设备不提供回读，
所以「灯亮没亮」无法用设备侧回读作证，物证仅为上述写入报文与驱动状态行，最终视觉确认仍需人眼。
i2c 通道的老机型未做同类验证。

### 已验证的官方驱动同源证据

`evidence/drivers` 下 G533ZW / G834JY / G614JI / G615LR 四款机型的官方
NumberPad 驱动解包后，`Numberpad/AsusNUMPADFilter.inf` 的硬件 ID 列表与
`HID_LAYOUTS` 同构；G533ZW、G834JY、G614JI 的 exe 甚至同源（md5 相同）。
说明厂商侧同样是「一份驱动装全系」，本实现的通用化方向与官方一致。

## 已知差异（如实披露，不掩盖）

1. **背光通道已在真机闭合**（04F3:3275，Fedora 44）：FEATURE 报告 ID `0x0d`、
   负载 `14 03 <level> ad`，`level` 取 `0x41`..`0x48` 为 1..8 档、`0x00` 熄灭，
   走 `HIDIOCSFEATURE` 下发，无需先发 `0x60` 解锁。老机型若走 i2c 通道，仍按
   `i2c_payload` 配置，灯不亮时先用 `--led none` 确认状态机。
2. **亮度映射是实测近似**：键盘背光 3 档 → 数字区 1/4/8 档为线性归一化结果，
   数字区实际可分几档取决于面板，`brightness_levels` 可自行裁剪；未做逐档逐机型核对。
   键盘背光**关到 0** 时不再取中间档，固定保持 `brightness_levels[0]`（默认 `0x41`，即 1 档），
   亮度只降不升；仅在读不到 `max_brightness` 的兜底路径才用中间档。
3. **NumLock 长按阈值**：见上文 `iNumLockMaxVaildDist = 2741` 的语义冲突。
4. **图标子区语义未闭合**：官方布局里 icon 区与计算器热区的边界存在重叠描述，本实现按整格归入
   NumLock 触发区，未向外扩展死区。

## 许可

MIT，见 [LICENSE](LICENSE)。
