# 打包说明

打包层全部是 **Bash**，与 asus-ctl 的做法一致：编译交给 cargo，装包交给各发行版原生工具
（`dpkg-deb` / `rpmbuild` / `makepkg`）。没有 Python，也不需要 debhelper / fpm 这类中间层。

## 目录

```
packaging/
├── build.sh                  统一入口：编译一次，按目标出包
├── lib/common.sh             共用函数（版本读取、目录布置、临时目录、日志）
├── common/                   三个发行版共用的数据文件
│   ├── asus-numpad.service    systemd 用户单元（systemd --user，免 root）
│   ├── 99-asus-numpad.rules   udev 权限规则
│   └── asus-numpad.1          man page
├── deb/
│   ├── make-deb.sh           dpkg-deb 直接打包（不走 dh）
│   └── postinst/prerm/postrm POSIX sh 维护脚本
├── rpm/
│   ├── asus-numpad.spec       @VERSION@ 由 make-rpm.sh 替换
│   └── make-rpm.sh           导出 tarball → rpmbuild -bb
└── arch/
    ├── PKGBUILD
    ├── asus-numpad.install    pacman 钩子
    └── make-arch.sh          有 makepkg 走 makepkg，没有就手工拼同构包
```

## 用法

```bash
./packaging/build.sh                 # 按 /etc/os-release 自动判断出哪种包
./packaging/build.sh deb             # 只出 .deb
./packaging/build.sh rpm arch        # 出两种（需要本机有对应工具链）
./packaging/build.sh --check         # 只自检：文件齐全性 + 工具链
./packaging/build.sh --no-build deb  # 复用已有 target/release/asus-numpad
```

产物统一落在 `dist/`。

也可以单独调某个脚本：

```bash
./packaging/deb/make-deb.sh  --version 5.0.1 --no-build --out /tmp/out
./packaging/rpm/make-rpm.sh  --version 5.0.1 --out /tmp/out   # 编译在 spec 的 %build 里
./packaging/arch/make-arch.sh --version 5.0.1 --no-build --out /tmp/out
```

注意 `--no-build` 的语义在两个脚本里不一样，别混：

| 脚本 | 选项 | 含义 |
|---|---|---|
| `make-deb.sh` / `make-arch.sh` | `--no-build` | 跳过 `cargo build`，复用 `target/release/asus-numpad`（Arch 走 makepkg 时会提示仍会编译） |
| `make-rpm.sh` | `--check` | 只跑 `rpmbuild --nobuild` 校验 spec，不出包；rpm 的编译由 rpmbuild 自己完成，因此没有“跳过编译”选项 |

## 静态检查

三个建包脚本、PKGBUILD、pacman 钩子按 Bash 检查，deb 维护脚本按 POSIX sh 检查，
本地与 CI 用的同一条命令：

```bash
shellcheck -x -S warning packaging/build.sh packaging/lib/common.sh \
    packaging/deb/make-deb.sh packaging/rpm/make-rpm.sh packaging/arch/make-arch.sh \
    packaging/arch/PKGBUILD packaging/arch/asus-numpad.install
shellcheck -s sh -S warning packaging/deb/postinst packaging/deb/prerm packaging/deb/postrm
```

## 安装后的文件布局

| 路径 | 内容 |
|---|---|
| `/usr/bin/asus-numpad` | 二进制 |
| `/usr/lib/systemd/user/asus-numpad.service` | 用户单元（随登录会话启动，免 root） |
| `/usr/lib/udev/rules.d/99-asus-numpad.rules` | 权限规则 |
| `/usr/share/man/man1/asus-numpad.1` | man |
| `/etc/asus-numpad/asus-numpad.json` | 系统级配置（deb conffiles / rpm `%config(noreplace)` / pacman backup） |
| `~/.config/asus-numpad/asus-numpad.json` | 用户配置，普通用户实际读写的就是这份（免 sudo） |
| `/usr/share/doc/asus-numpad/` | README 与 copyright |

## 设计取舍

1. **deb 不走 debhelper**。包里只有一个二进制加两个数据文件，走 `dh` 要拉一整条构建依赖链，
   还得维护 `debian/rules`、`install`、`changelog` 一堆空壳。`dpkg-deb --root-owner-group`
   一条命令就能产出合规包，脚本可读性反而更好。
2. **Arch 有降级路径**。`makepkg` 只在 Arch 上有，且明确拒绝 root 构建（容器/CI 就是这样）。
   非 Arch 主机或以 root 运行时，用 bsdtar 手工拼 `.PKGINFO` + `.pkg.tar.zst`，
   pacman 一样能装（少 `.MTREE`/`.BUILDINFO`，不做完整性校验）。
3. **rpm 源码走 tarball**。`%autosetup` 需要 `%{name}-%{version}.tar.gz`，因此 make-rpm.sh
   先导出 tarball 再 rpmbuild；导出时排除 `target/`、`dist/`、`.git`，避免把几百 MB 塞进包。
4. **版本号单一来源**。所有脚本都从 `Cargo.toml` 读 `version`，`--version` 可覆盖；
   spec 里用 `@VERSION@` 占位，Arch 用 sed 改 `pkgver`，不存在三处版本号打架。
5. **不自动改内核参数**。包只装文件与 hooks，不动 `modprobe` 配置、不动 ACPI 表；
   卸载后触摸板行为与装之前完全一致。

## 需要在真机上确认的点

- udev 规则按 `ATTRS{name}=="ASUF*|ASUE*|ASUP*"` 匹配，若新机型命名变了需要补一条。
- 背光走 hidraw/i2c 通用通道，首次使用若灯不亮按 README「已知差异」第 1 条处理。
- service 以登录用户身份运行（`systemd --user`）：触摸板 `grab`、`/dev/uinput`、
  hidraw 背光都靠 udev 的 `TAG+="uaccess"` 授权，因此不再需要 root；
  代价是用户单元拿不到实时调度类 / OOM 权重调整（都要 CAP_SYS_NICE）。
