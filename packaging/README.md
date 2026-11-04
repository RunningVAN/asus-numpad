# 打包说明

打包层全部是 Bash：编译交给 cargo，装包交给发行版原生工具（`dpkg-deb` / `rpmbuild` / `makepkg`）。
没有 Python，不需要 debhelper / fpm。

## 目录

```
packaging/
├── build.sh          统一入口：编译一次，按目标出包
├── build-skus.sh     按 skus.tsv 批量出各机型包
├── install-sku.sh    本机装包辅助
├── lib/common.sh     共用函数（版本读取、目录布置、日志）
├── common/           service / udev 规则 / man page
├── deb/              dpkg-deb 直接打包（不走 dh）
├── rpm/              spec + tarball → rpmbuild -bb
└── arch/             PKGBUILD，无 makepkg 时手工拼同构包
```

## 用法

```bash
./packaging/build.sh                 # 按 /etc/os-release 自动判断
./packaging/build.sh deb             # 只出 .deb
./packaging/build.sh rpm arch        # 指定多目标
./packaging/build.sh --check         # 只自检文件齐全性与工具链
./packaging/build.sh --no-build deb  # 复用已有 target/release/asus-numpad
```

产物落在 `dist/`。也可单独调脚本：

```bash
./packaging/deb/make-deb.sh  --version 5.0.1 --no-build --out /tmp/out
./packaging/rpm/make-rpm.sh  --version 5.0.1 --out /tmp/out
./packaging/arch/make-arch.sh --version 5.0.1 --no-build --out /tmp/out
```

`--no-build` 语义不一致：`make-deb.sh` / `make-arch.sh` 表示跳过 cargo，复用已有二进制；
rpm 的编译在 spec 的 `%build` 里，`make-rpm.sh` 只有 `--check`（`rpmbuild --nobuild` 校验）。

## 约定

- 版本号单一来源：所有脚本从 `Cargo.toml` 读 `version`，spec 用 `@VERSION@` 占位，
  Arch 用 sed 改 `pkgver`，三处不会打架。
- rpm 源码走 tarball：`%autosetup` 需要 `%{name}-%{version}.tar.gz`，导出时排除
  `target/`、`dist/`、`.git`。
- Arch 降级路径：非 Arch 主机或以 root 运行时，用 bsdtar 手工拼 `.PKGINFO` + `.pkg.tar.zst`。
- 不动系统：包只装文件与 hooks，不改 `modprobe`、不碰 ACPI，卸载后行为与装前一致。

## 静态检查

```bash
shellcheck -x -S warning packaging/*.sh packaging/lib/common.sh \
    packaging/deb/make-deb.sh packaging/rpm/make-rpm.sh packaging/arch/make-arch.sh \
    packaging/arch/PKGBUILD packaging/arch/asus-numpad.install
shellcheck -s sh -S warning packaging/deb/postinst packaging/deb/prerm packaging/deb/postrm
```

## 安装后的文件布局

| 路径 | 内容 |
| --- | --- |
| `/usr/bin/asus-numpad` | 二进制 |
| `/usr/lib/systemd/user/asus-numpad.service` | 用户单元（随会话启动，免 root） |
| `/usr/lib/udev/rules.d/99-asus-numpad.rules` | 权限规则 |
| `/usr/share/man/man1/asus-numpad.1` | man |
| `/etc/asus-numpad/asus-numpad.json` | 系统级配置（rpm `%config(noreplace)`） |
| `~/.config/asus-numpad/asus-numpad.json` | 用户配置，日常读写的就是这份 |
| `/usr/share/doc/asus-numpad/` | README 与 copyright |
