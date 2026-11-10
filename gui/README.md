---

# asus-numpad-gui

ASUS 触摸板数字小键盘（NumberPad）的 **背光控制图形界面**，Windows 与 Linux 共用同一份界面代码。

## 定位与边界

只负责背光（LED）逻辑：向设备的 HID FEATURE 报文写入亮度命令字，并提供一个用来调它的窗口。输入侧一律不碰，两个平台都是这样：

| 平台 | 数字键与手势由谁处理 | 本程序怎么点灯 |
| --- | --- | --- |
| Windows | ASUS 官方驱动（`AsusNUMPADFilter` 等） | 用户态 HID（`setupapi` + `hid.dll`）→ `HidD_SetFeature` 写 FEATURE 0x0d |
| Linux | 本仓库的 systemd 用户单元 `asus-numpad`（evdev + uinput） | 写 `/dev/hidrawX` 的 FEATURE 0x0d |

- 不装内核驱动，不接管键盘输入，不改变原有手感。
- 界面的设备列表、档位、报文编码、设备排序判据全部复用根 crate 的 `led_proto` / `led_policy` / `brightness`，两个平台与命令行驱动共用一套规则：同一台机器在 Windows 上选"第 2 档"、在 Linux 上选"第 2 档"，写出去的字节完全一致。
- Linux 上界面与驱动是两个独立进程：驱动管输入和跟随键盘背光，界面只发一次性的亮度命令。界面里也能启停驱动服务、开关自启，省得切终端。

## 协议

FEATURE report 长度 5 字节：

```
0d 14 03 <level> ad
```

| `level` | 含义 |
| --- | --- |
| `0x00` | 熄灭 |
| `0x01` | 普通点亮（保持当前档） |
| `0x41` … `0x48` | 亮度档（由暗到亮） |

该报告 write-only，写完不能回读，所以界面的"当前档位"以本地记录为准，不反映其它程序（键盘快捷键、`asus-numpad` 服务）刚做过的改动——下次点灯时会被界面覆盖回来，这是预期行为。

一台机器上通常有多个 HID 接口，只有带 FEATURE 0x0d 的那个能点灯。已知受支持的 PID 见 `src/led_policy.rs` 的 `KNOWN_NUMPAD_PIDS`（`0x3275` 等 ELAN 触摸板）。

## 构建

两个平台都是同一个 crate：

```bash
cargo build --release -p asus-numpad-gui
```

Windows（本机）：产物 `target\release\asus-numpad-gui.exe`，无需 MinGW；首次构建要联网拉 `eframe` 与 `windows`。

Linux：

```bash
sudo dnf install gcc pkgconf          # Fedora
sudo apt install build-essential pkg-config   # Debian/Ubuntu
cargo build --release -p asus-numpad-gui      # 产物 target/release/asus-numpad-gui
```

`eframe` 的 x11 / wayland 后端通过 dlopen 访问系统库，构建期不需要 X11 头文件；运行期需要 OpenGL（Mesa 或厂商驱动）、`libxkbcommon`，以及一个 X11 或 Wayland 会话。界面从系统字体里找中文字体（Noto Sans CJK / 文泉驿微米黑等），都没有时中文会显示成方块，功能不受影响。

在别的平台上只做类型检查：

```bash
rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-gnu
cargo check -p asus-numpad-gui --target x86_64-unknown-linux-gnu
cargo check -p asus-numpad-gui --target x86_64-pc-windows-gnu
```

`check` 不链接，因此不需要目标平台的头文件和链接器，适合在 macOS / Linux 上验证另一端的代码是否类型正确。

## 使用

直接运行二进制即为图形界面。界面分区：

| 分区 | 内容 |
| --- | --- |
| 设备接口 | 枚举到的 HID 接口，标注是否带 FEATURE 0x0d，`pick_best` 预先选好一个 |
| 灯光 | 档位滑条（1 档为熄灭）、点亮 / 熄灭、按报文写入 |
| 系统状态 | **仅 Linux**：配置文件路径、识别出的机型、触摸板布局、背光通道是否存在、`asus-numpad` 服务状态，并可启动 / 停止 / 重启 |
| 日志 | 每次写入的时间、接口、5 字节报文与返回值 |
| 设置 | 档位数（4 / 8）、跟随 NumLock、轮询间隔、启动时立即应用、开机自启 |

Linux 的"系统状态"区就是排查入口：灯不亮时先看这里——机型没认出来会在提示里说清楚是配置没写还是 DMI 没命中，背光通道缺失会提示检查 udev 规则。

命令行：

| 参数 | 作用 |
| --- | --- |
| `--selftest` | 打印平台状态、接口列表、档位表与每档对应报文，不写硬件 |
| `--on` | 点亮到默认档位 |
| `--off` | 熄灭 |
| `--level <n>` | 指定档位，十进制或 `0x` 十六进制均可 |
| `--apply-startup` | 供开机自启调用，静默写入一次灯光状态 |

设置文件（只存界面自己的选择，与驱动的 `asus-numpad.toml` 无关）：

| 平台 | 路径 |
| --- | --- |
| Windows | `%APPDATA%\asus-numpad-gui\settings.json` |
| Linux | `~/.config/asus-numpad-gui/settings.json` |

字段：档位数、当前档位、是否跟随 NumLock、轮询间隔（100–5000 ms）、上次选中的接口路径、启动时是否立即应用。

开机自启：

- Windows：写 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 下的字符串值 `AsusNumpadGui`，命令行是当前 exe 加 `--apply-startup`。不需要管理员权限，删掉该值即关闭。
- Linux：不写 autostart 文件，界面里的"开机自启"就是 `systemctl --user enable asus-numpad`——开机要点的是**驱动**的灯，界面本身不必常驻。想连界面一起启动，自己在桌面环境的自启动项里加 `asus-numpad-gui --apply-startup`。

## 常见问题

- **Linux 上"未找到可用接口"**：先确认 `/dev/hidraw*` 里能看到触摸板，再确认 udev 规则已生效（`packaging/` 里那份 70- 前缀规则，`udevadm control --reload-rules && udevadm trigger --subsystem-match=hidraw`）。规则没生效时普通用户打不开 hidraw，界面会提示权限不足。
- **Linux 上写入返回成功但灯不变**：多半选错了接口，改选列表里 `feature=true` 的那一项；也可能是 `asus-numpad` 服务正按键盘背光联动，两边在抢同一个通道，先停服务再试。
- **Windows 上写入成功但灯不变**：同上，改选带 FEATURE 0x0d 的接口。
- **跟随 NumLock 无效**：Windows 走 `GetKeyState(VK_NUMLOCK)` 轮询，Linux 读 `/sys/class/leds/*::numlock/brightness`；外接键盘或 OEM 定制布局可能改写语义，此时关掉联动手动选档。Linux 上如果更在意"跟随键盘背光"，用驱动服务自带的联动，不要开这个开关。
- **界面中文是方块**：系统里没装中文字体，装 `fonts-noto-cjk`（Debian/Ubuntu）或 `google-noto-sans-cjk-fonts`（Fedora）后重启界面。

*（内容由AI生成，仅供参考）*
