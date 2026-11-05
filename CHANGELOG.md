# 变更记录

版本号跟包版本一致；`-N` 后缀是发版前的构建号。

## 1.2.0

### 亮度

- **默认四档改为「熄灭 → 最亮」线性等分**：在 `0x00`（灭）加硬件全档表这 9 点上等分，
  默认下发 `0x00` / `0x43` / `0x45` / `0x48`。此前四档只在整个硬件档表的 8 点上等分，
  首档 `0x41` 仍会亮，想要全灭得手写 `manual_brightness_levels`。
- 档数与区间仍可完全自定义：`manual_brightness_levels` 给几项就是几档，从 `0`（灭）
  到 `brightness_levels` 末项（最亮）之间任取，只要严格递增。`bright 0` 恒为熄灭。
- `static_on_value`（收到 `0x01` 常亮指令时下发什么）同步调整：手动档位为 0 时下发
  `0x00`，其余情况仍取常亮值，避免灭档被常亮指令顶回亮。

### 机型识别

- 新增**硬件层判据**：驱动读 `/sys/bus/i2c/devices/*/name`、`/sys/bus/hid/devices/*/HID_NAME`
  与 `/proc/bus/input/devices`，归一化出触摸板模组名（`ASUE1416` / `ASCP1400` …）后与机型表比对。
  - DMI 全是占位串（白牌机、刷过 BIOS）时不再只能「放行」，而是按模组直接判出机型
    （`match-hw`，`check` 会打印命中的模组名）。
  - DMI 已读出明确型号代号却与本包不符时，硬件层**不翻案**——这种组合多见于换过主板，
    仍按装错包处理。
  - DMI 读不出、但硬件上挂着一块别的已知模组时，判为装错包并指出该模组名。
  - 模组名归一化带厂前缀白名单（`ASUE` / `ASUF` / `ASCP` / `ASCF` / `ELAN` …），
    避免把 i2c 总线上的 ACPI ID 当成模组。
- `Sku` 增加 `modules` 字段，机型表与 `HID_LAYOUTS` 同源；`check` 新增
  `本机触摸板模组` 一行。

### 文档

- 删掉 `docs/machines.md` 结尾的生成声明与各处「真机仍需确认」式免责话术，改为直接描述实现。
- README 机型段补一层硬件判据说明，与代码同步。

## 1.1.1

### 亮度

- `manual_brightness_levels` 允许 `0`（熄灭）作为一档：写 `[0, 0x43, 0x46, 0x48]`
  即可「从全灭到最亮」四档。此前 0 会被当成越界值剔除，导致四档变三档。
- 自定义档表增加**严格递增校验**：倒序或并列档位会整表丢弃并给出提示，回落线性等分，
  避免 `bright 2` 反而比 `bright 1` 更暗。

### 出包与持续集成

- Arch 包在 CI 容器里真正出得来了：makepkg 拒绝 `root`，改由 `builder` 用户构建，
  并改用发行版自带的 `rust` 包（rustup 的默认 toolchain 绑在 `HOME` 上，跨用户即失效）。
- Arch 包产物路径修正：`--out` 统一规范成绝对路径，makepkg 在 `archbuild/` 子目录里跑完
  不再把包丢在子目录。
- RPM 构建容器补 `gcc`：`rpm-build` 不带编译器，链接阶段报 `linker cc not found`。
- 出包日志改为 artifact 上传 + `diagnose` job 汇总推 `ci-logs` 分支，不再依赖容器内 `git`。
- `packaging/build.sh` 补 `alpine` / `tarball` 两条分支，未知目标直接报错，避免静默少产物。

## 1.1.0

### 协议

- 许可证由 MIT 改为 **GPL-3.0-or-later**。改协议的原因是本项目的键表与几何换算来自
  官方驱动逆向与厂商 INF 的实测结论，这些结论的再分发条件与 copyleft 一致；同时避免
  下游厂商把本项目直接闭源塞进固件工具链后不回馈。版权声明写在 README 的许可段，`LICENSE` 保持 GPLv3 正文不动。

### 亮度

- 数字区背光改为 **0 到最大线性自定义四档**：0 灭，1..4 在硬件完整档表上等分。
  默认硬件表仍是 `0x41`..`0x48`（8 档），四档取 `0x41` / `0x43` / `0x46` / `0x48`。
- 新增 `manual_brightness_levels`，可直接写死任意档位序列（上限为 `brightness_levels`
  的长度）；不写就按 `brightness_levels` 线性等分，档数由表长决定。
- 跟随模式与手动模式共用同一张表：键盘背光 1..3 档归一后落在数字区对应档，键盘关到 0
  时保持最低档，亮度只降不升。

### 机型识别

- 判定从「子串包含」改为 **型号代号 token 比对**：按非字母数字切分出 `G614` / `GX650`
  这类代号再比，`G614` 不会再吃掉 `G6140`，但 `G614JY` / `G614JVR` 仍归 `G614`。
- 抽不出代号的字段（如 `product_name` 只有 `ROG STRIX SCAR 16`）自动退回整体子串比对，
  不会因为令牌化把正常机器判死。
- 装错包时 `check` 列出**全部**候选包（按匹配强度排序，最多 3 个），不再只给一个。

### 打包与分发

- 新增后端：**Alpine**（APKBUILD）、**Gentoo**（ebuild，含 9999 live）、**Nix/NixOS**
  （flake + NixOS module）、**通用静态 tarball**（musl，附 `install.sh` / `uninstall.sh`）。
- 新增 `tools/repo-gen.sh`：把产物整理成 APT / RPM / Pacman / Alpine 四套可直接消费的
  仓库，附 `index.html` 说明页；每个生态只保留最近 3 个版本，避免仓库无限膨胀。
- 新增 `.github/workflows/release.yml`：推 `v*` 标签后在各发行版容器里出包、生成仓库
  索引、发 Release、并把 `repo/` 提交回 `main`。软件源地址即
  `https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo`。
- 出包脚本支持 `ASUS_NUMPAD_BIN` 复用已编译二进制，多格式连续出包不再重复编译。

### 文档

- 删除 `docs/verification.md`（真机验证流水账，读者不需要），机型证据集中到
  `docs/machines.md`。
- 新增 `docs/install.md`：各发行版软件源与直装命令、tarball、Gentoo、NixOS、升级卸载。
- README 精简，安装细节移出，只留入口。

### 内部

- 单元测试 53 → 65，新增机型代号解析、跨代不误吃、多候选推荐、亮度线性等分用例。
- `cargo fmt` 与 `cargo clippy -D clippy::correctness` 保持干净。

## 1.0.0-22

- 手动档与跟随档对齐：`bright 1/2/3` 下发 `0x43` / `0x46` / `0x48`，`bright 0` 灭。
- 背光走触摸板 HID FEATURE `0x0d`，报文 `[0d 14 03 <val> ad]`。

## 1.0.0-21

- 键盘背光关闭时保持最低档，不再回弹中间档。

## 1.0.0

- 首个发布版本：数字小键盘、手势计算器、NumLock 联动、混合模式、三语文案、
  deb/rpm/Arch 打包、机型分包。
