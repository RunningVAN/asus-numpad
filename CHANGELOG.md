# 变更记录

版本号跟包版本一致；`-N` 后缀是发版前的构建号。

## 1.1.0

### 协议

- 许可证由 MIT 改为 **GPL-3.0-or-later**。改协议的原因是本项目的键表与几何换算来自
  官方驱动逆向与厂商 INF 的实测结论，这些结论的再分发条件与 copyleft 一致；同时避免
  下游厂商把本项目直接闭源塞进固件工具链后不回馈。原作者版权声明保留在 `LICENSE` 中。

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
