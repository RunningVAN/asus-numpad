# 跨机型支持

## 为什么同模组天然通用

NumberPad 的键位按**触摸板模组 + HID 命名**区分，与整机型号无关。
`/sys/class/input/*/name` 里的 `ASUE1416 / ASUF1416 / ASUP1416 / ELAN140x / ASCP140x`
前缀是模组厂代码、后缀是模组序号。

同一块 ELAN i2c-hid 触摸板装到不同机型上，HID 名与描述符（含 FEATURE 报告 `0x0d`）都一致，
因此驱动只维护「HID 名 → 键表」映射（`src/layout.rs` 的 `HID_LAYOUTS`，共 44 条）。

这 44 条对应官方 `AsusNUMPADFilter.inf` 中 `Parameters\SupportList` 的 `PTP_0..45`
（46 项中 2 项为无线 `0B05:1879` / `0B05:1866` 复合设备，不走模组名）。
官方名单出现的每个模组名都已收录，由单测 `covers_official_supportlist` 守住。

| HID 名 | 键表 | 支持级别 | 说明 |
| --- | --- | --- | --- |
| `ASUE1416` | `GH3XLNDZ` | 已收录 | ELAN 04F3:3275，`GH3XLNDZ` 键表基准 |
| `ASUF1416` | `GH3XLNDZ` | 已收录 | 与 `ASUE1416` 同一模组几何 |
| `ASUE1403` / `ELAN1403` | `GH2XLNDZ` | 同族（需放行） | 结构相同、几何微差 |
| `ASCP1400` / `ASCF1400` | `GH4XLNDZ` | 同族（需放行） | 结构相同、几何微差 |

其余 38 条 HID（映射到 `CH*` / `NH*` / `GV*` 等 21 张键表）与 `GH3XLNDZ` 结构不同，
直接套用会按错键，驱动选择拒绝启动而不是乱按。

## 官方同源证据

| 驱动包 | 机型 | 版本 | exe MD5 前 12 位 |
| --- | --- | --- | --- |
| `PrecisionTouchPadNumberPad_ROG_ASUS_X_V17.0.0.27_41455_1.exe` | G533ZW / G614JI / G834JY | 17.0.0.27 | `28becdc340cc` |
| `PrecisionTouchPadNumberPad_ROG_ASUS_J_V17.0.0.27_41390_1.exe` | G615LR | 17.0.0.27 | `1ae02445f738` |
| `PrecisionTouchPadNumberPad_ROG_ASUS_X_V17.0.0.29_48348.exe` | G615LR | 17.0.0.29 | `b23042432cdd` |

三个不同二进制、跨 X/J 两条命名线，解包后 `AsusNUMPADFilter.inf` 的 487 条注册表参数
（`MainParameters` 5、`APSettingParameters` 6、`AlgoParameters` 425、`SupportList` 46、
`DriverParameters` 5）**逐项完全一致，差异 0 条**。

这解释了为什么本驱动按 HID 名切键表即可，不必按机型分叉算法。

## 支持级别闸门

| 级别 | 行为 |
| --- | --- |
| `verified` | 直接启动（模组名在 `HID_LAYOUTS` 中且键表已收录） |
| `family` | 默认拒绝，提示加 `--allow-unverified` |
| `unsupported` | 默认拒绝，提示反馈 `check` 输出 |

```bash
asus-numpad --allow-unverified run
# 或配置 {"allow_unverified_layout": true}
```

## 新机型自检

```bash
asus-numpad check
```

会额外打印背光候选段，列出每个 `/dev/hidraw*` 的 `vid:pid`、是否带 FEATURE `0x0d`、
优先级 `rank` 与最终选中项：

```
背光候选节点（按优先级排序）:
  /dev/hidraw3  04f3:3275  FEATURE0x0d=是  rank=0  [驱动会选中]
  /dev/hidraw5  0b05:19b6  FEATURE0x0d=否  rank=4  [备选]
  /dev/hidraw4  0b05:1ace  FEATURE0x0d=否  rank=255  [跳过：无线接收器]
```

`check` 里的 `本机触摸板模组` 一行就是硬件层读数：驱动会扫
`/sys/bus/i2c/devices/*/name`、`/sys/bus/hid/devices/*/HID_NAME` 与
`/proc/bus/input/devices`，把 `ASUE1416` 这类模组名归一化后与机型表比对。
DMI 写坏、写成占位串的机器靠这一层仍能认出实测机型。机型表之外的型号不参与机型判定，
一律走通用配置——键表按模组选，不受整机型号影响。

新 PID 加进 `src/led_policy.rs` 的 `KNOWN_NUMPAD_PIDS`；新模组在
`evidence/drivers/*_unpacked/**/AsusNUMPADFilter.inf` 里查 `HID_DEVICE_NAME`
补一条 `HID_LAYOUTS`，再按上游布局文件补键表。