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
License:        MIT
URL:            https://github.com/asus-numpad/asus-numpad
Source0:        asus-numpad-%{version}.tar.gz
BuildRequires:  rust >= 1.75
BuildRequires:  cargo
BuildRequires:  systemd-rpm-macros

%description
在 ASUS 笔记本的触摸板上模拟数字小键盘。参数取自官方 AsusNUMPADFilter.inf
的 AlgoParameters 原值（帧长 8ms、0.1mm 单位、手势 4838µm 等），通过 evdev/uinput
在用户态实现，不替换内核模块、不改 ACPI 表，卸载即恢复。

本包为机型：%{sku}。启动时会比对本机 DMI 型号，对不上直接拒绝启动（退出码 3），
避免装错机型后按键全无声。强制运行可加 --allow-sku-mismatch。

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
# 关键：hidraw 节点在安装时已存在，规则重载不会对既有节点重新应用，
# 必须显式 trigger 才会补上 uaccess ACL / 0660 input 权限。旧脚本漏了这一条，
# 真机上表现为装完仍 Permission denied，只能靠重启才恢复。
udevadm trigger --subsystem-match=hidraw --action=change >/dev/null 2>&1 || true
udevadm settle >/dev/null 2>&1 || true
# 开机自启：用户级单元默认只在登录会话里起。打开 linger 后 systemd --user
# 实例在开机时就拉起，不必先登录桌面。
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
* Wed Nov 04 2026 asus-numpad maintainers <maintainers@example.invalid> - @VERSION@-18
- 修复装完仍 Permission denied：%post 补 `udevadm trigger --subsystem-match=hidraw`。
  hidraw 节点在安装时已存在，reload-rules 不会对既有节点重放规则，此前只能靠重启碰巧恢复
- 修复没有开机自启：%post 对安装者执行 `loginctl enable-linger`，用户级服务在开机阶段
  即由 systemd --user 拉起，不必先登录桌面；升级与重装同样生效
- udev 规则加 `KERNEL=="hidraw*"` 限定，明确 uaccess 与 input 组两条授权路径
- 报错文案给出可执行的三步取证（规则是否安装 / 是否生效 / 会话是否在 seat 内）与
  `usermod -aG input` 兜底路径；run 抓取失败的提示改为先停用户级服务，避免两个进程 EBUSY
* Wed Nov 04 2026 asus-numpad maintainers <maintainers@example.invalid> - @VERSION@-17
- 背光候选探测区分「描述符里没有 FEATURE 0x0d」与「hidraw 无权限打开」：
  此前驱动器把 Permission denied 当成无 FEATURE 报告，真机上会误导排障方向。
  check 现在对无权限候选直接标「权限不足」并提示 udev reload/trigger，运行期告警同步改写。
- 帮助文本与实现对齐：bright 明示 up / down / cycle / next；--device 示例改为
  按厂商自动探测（义隆 04F3 / 原相 093A），不再写死单一模组 ID，避免 PixArt 机型误解
* Wed Nov 04 2026 asus-numpad maintainers <maintainers@example.invalid> - @VERSION@-15
- 日常命令免 sudo：配置改落 ~/.config/asus-numpad/asus-numpad.json（root 仍写 /etc），
  界面文案支持 ~/.config/asus-numpad/lang/ 用户级覆盖
- service 改为 systemd 用户单元（/usr/lib/systemd/user/），随登录会话启动，
  手工控制用 systemctl --user；升级时自动停掉旧的系统级服务，避免两个进程抢触摸板
- udev 规则放宽到按厂商（04F3 / 093A）匹配 event 与 hidraw，uaccess 生效后普通用户即可读写
- bright / lang 写盘与重启服务不再提示 root，输出文案三语同步更新
* Wed Nov 04 2026 asus-numpad maintainers <maintainers@example.invalid> - @VERSION@-11
- 按机型分包：asus-numpad-g533/g614/g615/g634/g635，包名与 Summary 写明机型
- 包内新增 /usr/share/asus-numpad/sku.json 自证机型；机型包之间互相 Conflicts
- 启动时比对 DMI product_name，与包机型不符即拒绝启动（退出码 3，--allow-sku-mismatch 可强制）
- 编译期由 build.rs 写入机型标记，出包脚本可 grep 二进制校验，防止「包名对、内容错」
* Wed Nov 04 2026 asus-numpad maintainers <maintainers@example.invalid> - @VERSION@-6
- 多语言：简体中文 / 繁體中文 / English，内置三份 lang 文案编进二进制（缺失也不空白）
- `lang` 子命令查看与切换语言，`lang auto` 跟随系统 locale；`--lang` 与
  ASUS_NUMPAD_LANG 可临时覆盖
- /usr/share/asus-numpad/lang/*.lang 可整份替换或单键覆盖，/etc/asus-numpad/lang/ 优先级更高；
  ASUS_NUMPAD_LANG_DIR 可指定试译目录
- 全部命令输出（run/check/dump/bright/lang 及报错、警告、自检报告）均已国际化

* Wed Nov 04 2026 asus-numpad maintainers <maintainers@example.invalid> - @VERSION@-3
- 背光亮度跟随键盘背光：读 /sys/class/leds/asus::kbd_backlight，1..8 档映射 0x41..0x48，
  每 500ms 轮询并仅在变化时下发 FEATURE 0x0d；可用 --no-follow-brightness 关闭
- `check` / `run` 启动横幅打印当前键盘背光档位与换算结果，便于排查

* Tue Nov 03 2026 asus-numpad maintainers <maintainers@example.invalid> - @VERSION@-2
- hidraw 背光通道改为 FEATURE 0x0d（0d 14 03 <v> ad），修复背光不亮
- 设备匹配按 04F3:3275 + 描述符，排除 0B05:1ACE OMNI 接收器

* Tue Nov 03 2026 asus-numpad maintainers <maintainers@example.invalid> - @VERSION@-1
- 首个正式 RPM 版本：官方 AlgoParameters 对齐、混合模式手势、NumLock 双向联动
