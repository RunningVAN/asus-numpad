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

## 同一型号不是都有 NumberPad（按配置核验）

**同一模具、甚至同一型号代号，也有不带 NumberPad 的配置。** 华硕规格页把每个零售配置
（SKU）列成一列，「键盘与触控板」栏逐列不同：单键 RGB 版带 `Support NumberPad`，
4 区 RGB 版只有 `Touchpad`。所以「型号能识别」不等于「这台有硬件」。

核验口径：取官方规格页 `ProductGroup` 里每个 variant 的「键盘与触控板」栏，逐配置判定。
下表为 2026-11 核验结果（来源：`rog.asus.com.cn` 与 `rog.asus.com` 各代 `/spec/` 页）：

| 机型行 | 官方页 | 配置数 | 带 NumberPad | 不带（4 区 RGB） |
| --- | --- | --- | --- | --- |
| `g614`（G16 2023） | `rog-strix-g16-2023-series` | 10（全球站）/ 4（中国站） | 8 / 2 | `90NR0D51-M004W0`、`90NR0D52-M004X0`；中国站 `0EACXHB8X20` 两条 |
| `g614`（G16 2024、SCAR 16 2024） | `rog-strix-g16-2024`、`rog-strix-scar-16-2024` | 5 + 2 | 7 | 无 |
| `g615`（G16 2025） | `rog-strix-g16-2025` | 25 | 24 | `90NR0LJ1-M005P0` |
| `g615`（SCAR 16 2025） | `rog-strix-scar-16-2025` | 3 | 3 | 无 |
| `g533`（SCAR 15 2022） | `rog-strix-scar-15-2022-series` | 1 | 1 | 无 |
| `g533`（G15 2022，`G513R*`） | `rog-strix-g15-2022-series` | 5 | 2 | `90NR0846-M00A70`、`90NR0845-M00LL0`、`90NR0845-M00JD0` |
| `gx550`（DUO 16 2022 / 2023） | `rog-zephyrus-duo-16-2022/2023-series` | 2 + 4 | 6 | 无 |

从这张表能读出的三条：

* **DMI 型号前缀区分不出来**：`G614JV13980-0EAGXHB8X20` 与 `G614JV13980-0EACXHB8X20`
  的型号前缀同为 `G614JV`，差别只在国内后缀 `EAGX`（单键 RGB，带）/ `EACX`（4 区 RGB，不带）。
  全球零售编码（`90NR…`）又是另一套，与后缀无对应关系。
* **地区配置不同**：同一个 G16 2023 页面，中国站列 4 个配置，全球站列 10 个。核验必须按
  官网当前在售列表逐条看，不能照抄别国列表，也不能凭型号后缀硬编码。
* **代际内也混装**：2025 代 25 个配置里有 1 个不带；2022 代魔霸 6 五个配置里三个不带。

### 驱动怎么处理

* 型号行只负责**选参数与键表**，不当成「有硬件」的证据；键表仍按触摸板 HID 模组名选。
* 4 区 RGB 配置上，触摸板没有图标区，长按图标区不会触发数字模式；DMI 仍会命中 `g614`
  这类行（前缀相同），所以驱动不会把它判成「表外机器」，只是没有硬件响应可做。
* 这类配置的触摸板模组名是否仍报 `ASUE1416` 尚未实测确认。要在不带 NumberPad 的配置上
  验证，跑 `asus-numpad check` 看「本机触摸板模组」与背光候选段：模组名在、FEATURE `0x0d`
  在，说明按键链路可用，缺的只是图标区；两者都没有，就是这块硬件压根没配。

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
DMI 写坏、写成占位串的机器靠这一层仍能认出机型。机型表之外的型号没有参数可套，
启动闸门以退出码 `3` 拒绝启动并打印支持列表——不再落一套通用配置硬跑。键表仍按模组选，
不受整机型号影响。

新 PID 加进 `src/led_policy.rs` 的 `KNOWN_NUMPAD_PIDS`；新模组在
`evidence/drivers/*_unpacked/**/AsusNUMPADFilter.inf` 里查 `HID_DEVICE_NAME`
补一条 `HID_LAYOUTS`，再按上游布局文件补键表。