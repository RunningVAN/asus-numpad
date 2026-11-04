# 机型分包 spec：默认出「全机型兼容包」，加 --define 'sku g634' 就出该机型专属包。
#   rpmbuild -bb asus-numpad.spec
#   rpmbuild -bb --define 'sku g634' asus-numpad.spec
# 实际出包走的是 tools/rpmgen/build_rpm.py（Mac/Linux 都能跑，不需要 rpmbuild），
# 这份 spec 保持与之等价，供发行版官方构建使用。
%global all_skus g533 g614 g615 g634 g635 gx550 gx650 ux582 h7606
%{!?sku:%global sku universal}

%{!?version:%global version @VERSION@}

%if "%{sku}" == "universal"
Name:           asus-numpad
Summary:        ASUS 触摸板数字小键盘（NumberPad）用户态驱动
%else
Name:           asus-numpad-%{sku}
Summary:        ASUS 触摸板数字小键盘（NumberPad）用户态驱动 — %{sku} 机型包
# 机型包互斥：一台机器只装一个机型包，装错时 dnf 直接报冲突而不是静默覆盖
%for s %all_skus
%if "%{s}" != "%{sku}"
Conflicts:      asus-numpad-%{s}
%endif
%endfor
%endif

Version:        %{version}
Release:        12%{?dist}
License:        GPL-3.0-or-later
URL:            https://github.com/RunningVAN/asus-numpad
Source0:        asus-numpad-%{version}.tar.gz
BuildRequires:  rust >= 1.75
BuildRequires:  cargo
BuildRequires:  systemd-rpm-macros

%description
ASUS 触摸板数字小键盘（NumberPad）用户态驱动。参数取自官方 AsusNUMPADFilter.inf
的 AlgoParameters 原值，evdev/uinput 实现，不替换内核模块、不改 ACPI 表。

本包机型：%{sku}。启动比对 DMI，对不上拒绝启动（退出码 3）；强制运行加 --allow-sku-mismatch。

%prep
%autosetup -n asus-numpad-%{version}

%build
# 发行版构建环境默认禁网：有 Cargo.lock 才用 --locked，没有就让 cargo 自行解析
if [ -f Cargo.lock ]; then
    ASUS_NUMPAD_SKU=%{sku} cargo build --release --locked --offline || \
        ASUS_NUMPAD_SKU=%{sku} cargo build --release --locked
else
    ASUS_NUMPAD_SKU=%{sku} cargo build --release
fi

%install
install -Dm0755 target/release/asus-numpad %{buildroot}%{_bindir}/asus-numpad
install -Dm0644 packaging/common/asus-numpad.service \
    %{buildroot}%{_userunitdir}/asus-numpad.service
install -Dm0644 packaging/common/99-asus-numpad.rules \
    %{buildroot}%{_udevrulesdir}/99-asus-numpad.rules
install -Dm0644 packaging/common/asus-numpad.1 \
    %{buildroot}%{_mandir}/man1/asus-numpad.1
install -d %{buildroot}%{_datadir}/asus-numpad/lang
install -m0644 lang/zh_CN.lang lang/zh_TW.lang lang/en.lang \
    %{buildroot}%{_datadir}/asus-numpad/lang/

install -d %{buildroot}%{_sysconfdir}/asus-numpad
cat > %{buildroot}%{_sysconfdir}/asus-numpad/asus-numpad.json <<'JSON'
{
  "numlock": "sync",
  "calc_gesture": true,
  "calc_gesture_mode": "long",
  "mixed_mode": "auto",
  "grab": true,
  "follow_keyboard_brightness": true,
  "language": "auto"
}
JSON

# 机型自证：装完 rpm -ql 就能看到这包是给哪台机器的
cat > %{buildroot}%{_datadir}/asus-numpad/sku.json <<SKUJSON
{
  "sku": "%{sku}",
  "package": "%{name}",
  "version": "%{version}-%{release}"
}
SKUJSON

%files
%doc README.md
%license LICENSE
%{_bindir}/asus-numpad
%{_userunitdir}/asus-numpad.service
%{_udevrulesdir}/99-asus-numpad.rules
%{_mandir}/man1/asus-numpad.1
%dir %{_datadir}/asus-numpad
%dir %{_datadir}/asus-numpad/lang
%{_datadir}/asus-numpad/lang/zh_CN.lang
%{_datadir}/asus-numpad/lang/zh_TW.lang
%{_datadir}/asus-numpad/lang/en.lang
%{_datadir}/asus-numpad/sku.json
%dir %{_sysconfdir}/asus-numpad
%config(noreplace) %{_sysconfdir}/asus-numpad/asus-numpad.json

%post
# 用户级服务：装完不用 root 就随会话自启
systemctl --global enable asus-numpad.service >/dev/null 2>&1 || :
# 从旧版 system 级服务升级：停掉旧的，避免两个进程抢同一块触摸板
systemctl --no-reload disable --now asus-numpad.service >/dev/null 2>&1 || :
udevadm control --reload-rules >/dev/null 2>&1 || true
udevadm trigger --subsystem-match=input --action=change >/dev/null 2>&1 || true
udevadm trigger --subsystem-match=misc --action=change >/dev/null 2>&1 || true
# hidraw 节点装包时已存在，规则重载不会重放，必须显式 trigger 才补上 uaccess ACL
udevadm trigger --subsystem-match=hidraw --action=change >/dev/null 2>&1 || true
udevadm settle >/dev/null 2>&1 || true
# 开 linger：用户级单元在开机阶段即被拉起，不必先登录桌面
_installer="${SUDO_USER:-}"
if [ -z "$_installer" ]; then
    _installer=$(loginctl list-users --no-legend 2>/dev/null | awk '$1 >= 1000 {print $2; exit}')
fi
if [ -n "$_installer" ] && [ "$_installer" != "root" ]; then
    loginctl enable-linger "$_installer" >/dev/null 2>&1 || :
fi
for u in $(loginctl list-users --no-legend 2>/dev/null | awk '{print $1}'); do
    uid=$(id -u "$u" 2>/dev/null) || continue
    [ -d "/run/user/$uid" ] || continue
    su "$u" -s /bin/sh -c "XDG_RUNTIME_DIR=/run/user/$uid systemctl --user daemon-reload; XDG_RUNTIME_DIR=/run/user/$uid systemctl --user enable --now asus-numpad.service" >/dev/null 2>&1 || :
done

%preun
if [ $1 -eq 0 ]; then
    for u in $(loginctl list-users --no-legend 2>/dev/null | awk '{print $1}'); do
        uid=$(id -u "$u" 2>/dev/null) || continue
        [ -d "/run/user/$uid" ] || continue
        su "$u" -s /bin/sh -c "XDG_RUNTIME_DIR=/run/user/$uid systemctl --user disable --now asus-numpad.service" >/dev/null 2>&1 || :
    done
    systemctl --global disable asus-numpad.service >/dev/null 2>&1 || :
    systemctl --no-reload disable --now asus-numpad.service >/dev/null 2>&1 || :
fi

%postun
if [ $1 -ge 1 ]; then
    for u in $(loginctl list-users --no-legend 2>/dev/null | awk '{print $1}'); do
        uid=$(id -u "$u" 2>/dev/null) || continue
        [ -d "/run/user/$uid" ] || continue
        su "$u" -s /bin/sh -c "XDG_RUNTIME_DIR=/run/user/$uid systemctl --user daemon-reload; XDG_RUNTIME_DIR=/run/user/$uid systemctl --user try-restart asus-numpad.service" >/dev/null 2>&1 || :
    done
fi
udevadm control --reload-rules >/dev/null 2>&1 || true

%changelog
* Thu Nov 05 2026 asus-numpad maintainers <maintainers@example.invalid> - @VERSION@-22
- 手动三档与跟随映射对齐：bright 1/2/3 → 0x43/0x46/0x48，切换 follow 不再跳变
- 精简程序内提示文案与文档

* Wed Nov 04 2026 asus-numpad maintainers <maintainers@example.invalid> - @VERSION@-18
- %post 补 hidraw trigger 与 linger：装完即生效、开机自启
- udev 规则加 KERNEL=="hidraw*" 限定
