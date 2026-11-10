# asus-numpad-win

ASUS 触摸板数字小键盘（NumberPad）的 **Windows 背光控制工具**。

## 定位与边界

本程序只负责背光（LED）逻辑，即向设备的 HID FEATURE 报文写入亮度命令字，并提供图形界面调整。

- **不安装内核驱动**，**不接管键盘输入**。数字键、手势、防误触、NumLock 语义全部由 ASUS 官方驱动（`AsusNUMPADFilter` 等）处理，因此原有手感与行为不发生任何变化。
- 硬件访问只使用 Windows 自带的用户态 HID 接口（`setupapi` + `hid.dll`），通过 `HidD_SetFeature` 写入 `FEATURE report 0x0d`。
- 目标系统：Windows 10（1809 及以上）/ Windows 11，x86_64。

Linux 侧的完整用户态驱动位于仓库根目录（`src/linux`），两者共用同一套与平台无关的逻辑：`led_proto`（报文编码）、`led_policy`（设备接口排序判据）、`brightness`（档位映射）。Windows 端不重复实现这些规则，档位表由 `linear_levels_from_off` 生成，与 Linux 的 `manual_brightness_levels` 完全一致——同一台机器在两端选择"第 2 档"，亮度相同。

## 协议

FEATURE report 长度 5 字节，报文格式：

```
0d 14 03 <level> ad
```

| `level` | 含义 |
| --- | --- |
| `0x00` | 熄灭 |
| `0x01` | 普通点亮 |
| `0x41` … `0x48` | 亮度档（由暗到亮） |

该报告是 write-only，写入后不能回读，因此界面的"当前档位"以本地设置为准，不反映外部程序（如键盘快捷键）的改动。

设备存在多个 HID 接口，只有带 FEATURE 0x0d 的那个可点灯；不带该报告的接口即使 VID/PID 相同也无法写亮度。已知受支持 PID 见 `src/led_policy.rs` 中的 `KNOWN_NUMPAD_PIDS`（`0x3275` 等 ELAN 触控板）。

## 构建

Windows 本机（推荐）：

```powershell
cargo build --release -p asus-numpad-win
# 产物：target\release\asus-numpad-win.exe
```

在其它平台只做类型检查（不需要链接器）：

```bash
rustup target add x86_64-pc-windows-gnu
cargo check --target x86_64-pc-windows-gnu -p asus-numpad-win
```

若要在非 Windows 平台交叉**链接**出 exe，还需安装 MinGW 工具链（`x86_64-w64-mingw32-gcc`）并配置 `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER`。

依赖（`eframe` / `windows`）在首次构建时需要联网拉取。

## 使用

图形界面：直接运行 exe。界面包含设备接口列表、亮度档位（1 档为熄灭）、跟随 NumLock 联动、写入报文日志和设置项。

命令行：

| 参数 | 作用 |
| --- | --- |
| `--selftest` | 打印接口列表、档位表与每种档位对应的报文，不写硬件 |
| `--on` | 点亮到默认档位 |
| `--off` | 熄灭 |
| `--level <n>` | 指定档位，十进制或 `0x` 十六进制均可 |
| `--apply-startup` | 供开机自启调用，静默写入一次灯光状态 |

设置保存在 `%APPDATA%\asus-numpad-win\settings.json`：档位数（4 或 8）、当前档位、是否跟随 NumLock、轮询间隔、上次选中的设备路径、启动时是否立即应用。

开机自启写入注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`，值名 `AsusNumpadWin`，命令行为当前 exe 加 `--apply-startup`。删除该值即关闭自启，不需要管理员权限。

## 常见问题

- **界面显示"未找到可用接口"**：确认触摸板已开启 NumberPad 模式、官方驱动已安装；再检查是否被其它软件独占。普通用户权限即可写入 FEATURE 报文，无需管理员。
- **写入返回成功但灯不变**：多半是选错了接口（选中了不带 FEATURE 0x0d 的那个），在设备列表里改选 `feature=true` 的项。
- **跟随 NumLock 无效**：NumLock 状态通过 `GetKeyState(VK_NUMLOCK)` 轮询，部分外接键盘或 OEM 定制布局会改写该键语义，此时请关闭联动、手动选择档位。
