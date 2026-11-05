# 变更记录

版本号跟包版本一致；`-N` 后缀是发版前的构建号。

## 1.2.3

### 亮度

- **每个档位可以指定任意一个硬件支持的亮度**：新增 `manual_brightness_levels` 的逐档编辑
  入口 `asus-numpad levels <档> <值>`，值只接受 `0x00`（熄灭）或 `0x41`..`0x48`（硬件 8 个
  亮度刻度），档号 `1..=当前档数`。改完写盘（写前留 `.bak`）并重启服务，`bright N` 立即
  按新表下发。
- **默认行为不变**：不配置时仍是按 `brightness_levels` 线性等分的四档
  `0x00 / 0x43 / 0x45 / 0x48`；`levels reset` 一键回落这套默认。
- 档表长度即档数（上限 8）：想只要三档就写 3 项，想 8 档全开就写 8 项。硬件没有中间灰度，
  相邻档写成同一个值是允许的（只是看不出变化），但**不允许后一档比前一档暗**——
  否则 `bright up` 会越按越黑，越序直接拒绝。
- 整表非法（有空值、或全部越界）回落到默认四档并给出提示；超长表截断到 8 档；只改单档时
  不会把其余档位弄丢（`null` 会先落成当前生效的实表再改）。

### 机型识别

- **修掉 ProArt P16 的错误归类**：`H7606` 此前被并进 `g615`，是错的——那台机器的触控板是
  ASUS **DialPad 智能旋钮**（官方规格写 `support DialPad`），根本没有 NumberPad 虚拟数字键
  盘。现已从机型号召剔除，并新增 `NON_NUMPAD_MODELS` 显式拒绝：识别到就直说「这台不需要
  本驱动」并给出原因，退出码 `4`，不再含糊地报「机型不匹配，建议改装 asus-numpad-xxx」。
  18 英寸 Plus（`G834` / `G835` / `G814` / `G815` / `G813`，机身自带物理数字键）一并走这条
  新结论，`install-sku.sh` 同步。
- **魔霸6 15 英寸归位**：`g533` 包吸收 `G513`（模组 ASUE1403 / ELAN1403 → 键表 `GH2XLNDZ`，
  与枪神6 同代同平台）；官方规格里 `G513` 只有部分配置写 `Support NumberPad`，没这块触摸板的
  机器由硬件探测兜底。`g533` 的展示名、`skus.tsv`、`install-sku.sh` 型号匹配同步更新，
  `g513` 作为被合并 id 由单测守住。
- **17 英寸 G713 不并入**：`G713` 机身自带物理数字键区，官方规格 `NumberPad` 一栏是 `N/A`，
  触控板没有虚拟数字键盘。已归入 `NON_NUMPAD_MODELS` 与 `install-sku.sh` 的拒绝分支
  （退出码 `4`），并有单测确认任何机型包都不会把它认成 NumberPad 机器。
- 判定顺序：本包白名单命中 → `NotNumberPad`（明确没有 NumberPad）→ 非 ASUS 厂商 →
  机型不匹配。`NotNumberPad` 不给「改装哪个包」的建议，因为装哪个都没用。

## 1.2.2

### 出包

- **deb / rpm 不再按发行版重复出包**：从 4 个 deb + 3 个 rpm 收敛成各 1 个通用包，
  构建任务从 9 个降到 4 个（deb / rpm / arch / alpine）。deb 在 Ubuntu 22.04
  （glibc 2.35）里编一份，装得上 Ubuntu 22.04 / 24.04、Debian 12 / 13、Mint 21+；
  rpm 在 AlmaLinux 9（glibc 2.34）里编一份，装得上 RHEL / Rocky / Alma 9~10、
  Fedora、openSUSE。此前每个发行版各编一份，几份二进制内容其实一样、只差编译环境的
  C 库下限，等于同一份东西存了 7 遍。
- **「一份包通用」改由 CI 证明**：安装验证仍是 9 个容器，但 Fedora 与 openSUSE
  现在装的就是 el9 那份 rpm，Debian 12/13 与 Ubuntu 22.04/24.04 装的就是同一份
  deb。哪条线不通用会直接红，不再靠挑包绕过。
- 发版前增加唯一性硬校验：`dist/` 里 deb 或 rpm 若出现第二份，publish 直接失败。
- Arch 与 Alpine 保留各自原生包：滚动版与 musl 体系没有「跨版本通用」可言。

### AUR

- **补上 AUR 包 `asus-numpad-rs`**：新增 `packaging/aur/PKGBUILD.in`、
  `gen-aur.sh` 与 `.install`。源码取 GitHub tag tarball，`makepkg` 现场编译，
  装出来是本机工具链的原生二进制。AUR 上 `asus-numpad` 已被另一个 Python 实现
  占用（0.4.1-7），故包名加 `-rs`；二进制名不变，用 `provides` / `conflicts`
  声明替代关系。
- CI 新增 `aur` job：在 archlinux 容器里用渲染好的 PKGBUILD 真编、`pacman -U`
  真装、跑 `--version` 与 `--help`，并由 `makepkg --printsrcinfo` 生成
  `.SRCINFO`（该文件只能由 makepkg 生成，手写会被 AUR 拒收）。
- PKGBUILD 与 `.SRCINFO` 随 Release 发布：AUR 尚未同步时可直接下载这两个文件
  `makepkg -si`。

## 1.2.1

### 出包

- **同时为 7 个发行版出包**：apt 线四个（Ubuntu 22.04 / 24.04、Debian 12 / 13），
  rpm 线三个（AlmaLinux 9 / Fedora / openSUSE Tumbleweed），加 Arch 与 Alpine。
- **软件源只收「基线包」**：取编译环境 glibc 最老的那一份——apt 收 22.04 编的、
  rpm 收 el9 编的。此前 apt 与 rpm 仓库里都是同名包互相覆盖，最后留下哪个发行版编的
  全看 artifact 下载顺序，`Sources` 里看着正常、在老系统上却可能装不上。
  其余发行版的原生包照发，只是挂在 Release 而不进源。
- deb 版本号支持 `~distro` 标签（`1.2.1~jammy`），rpm 的 `Release` 从写死的 `12`
  改为可传参（`--release`，默认 1），两者都只影响文件名与版本序，包名不变，
  升级链不断。
- **修好 Alpine 出包**：两处都是硬性检查——APKBUILD 必须摆在
  `aports/community/<pkgname>/` 这种以包名命名的目录里，且执行 abuild 的用户必须
  在 `abuild` 组里，否则第一步装依赖就报 `User builder is not a member of group abuild`。
  第三处是源码摆放：`source` 里写裸文件名时 abuild 只认 APKBUILD 同级目录，把
  tarball 放进 SRCDEST 会让它报 `sha512sum FAILED`。此前这三条都没满足，Alpine
  包从没出成功过。顺带把 `find -printf` 换成 busybox 也认的写法（Alpine 的 find
  没有这个动作），并在 abuild 跑完后把 `PKGDEST` 与 `REPODEST` 两处都扫一遍收回
  `.apk`——不同 abuild 版本对这两个变量的优先级不一样，只认一处会出现「abuild
  成功了但 dist 是空的」。
- **修掉脚本里「变量后面紧跟中文标点」的坑**：`$VERSION，` 这种写法会被 bash
  当成名为 `VERSION，` 的变量，脚本开了 `set -u` 就直接 `unbound variable` 退出
  （`repo-gen.sh` 因此在 CI 里失败），没开 `set -u` 的则静默吞掉内容、日志显示空白。
  全仓统一改成 `${VERSION}` 形式。
- **Alpine 源索引去掉 `--rewrite-arch`**：apk-tools 3 删了这个选项，带上它
  `apk index` 直接报错，仓库生成整步失败；包架构本来就在 `.PKGINFO` 里。
- **发版前在每个目标发行版里真装一遍**：9 个容器（Debian 12 / 13、Ubuntu 22.04 / 24.04、
  AlmaLinux 9、Fedora、openSUSE、Arch、Alpine）各自 `apt/dnf/zypper/pacman/apk` 装包，
  再跑 `asus-numpad --version` 与 udev 规则落地检查；任一个装不上就不发版。基线包跨发行版
  装到更老的系统这一步，正是 glibc 没兜住时报 `GLIBC_2.xx not found` 的地方。
- 删掉 tag 重发时，旧 Release 会失去 tag 关联从网页上消失（GitHub 把它退化成草稿）。
  发布流程现在会把它重新绑回 tag 并置为已发布，不用手动去网页点。
- 出包失败的容器日志现在直接贴在 Actions 输出里，不用再下载 artifact 才能复盘。

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
