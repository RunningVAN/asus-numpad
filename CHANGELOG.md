# 变更记录

版本号跟包版本一致；`-N` 后缀是发版前的构建号。

1.0.0 起版本号在 `asus-numpad` 组织下重新起算。1.2.4 及更早的记录来自旧仓库
[`RunningVAN/asus-numpad`](https://github.com/RunningVAN/asus-numpad)，见 git 历史。

## 1.0.2

### 机型兼容表按官方配置逐条核验

- 同一型号不是都有 NumberPad：G16 2023 官方 10 个配置里 2 个是 4 区 RGB 版（尾缀 `EACX`），
  G16 2025 的 25 个里 1 个，魔霸 6 五个里 3 个，这几台的触摸板与带硬件的配置完全同款。DMI 型号
  前缀也区分不出来——`G614JV13980-0EAGXHB8X20` 与 `…-0EACXHB8X20` 前缀同为 `G614JV`，差别只在
  国内后缀。所以键表照旧按触摸板模组选，不按型号代码推有没有硬件；硬件不存在时驱动不动键盘。
- `docs/machines.md` 新增「同一型号不是都有 NumberPad（按配置核验）」一节：口径取官方规格页
  「键盘与触控板」栏，逐 retail 配置判定，列出各代官方配置数、不带硬件的具体编码，并记下
  中国站与全球站列表不同的地方。三语 README 的支持机型段补了说明与指向。
- 机型表 `g533` / `g614` / `g615` 的备注补上配置级事实，`ux582` 那行标注为缺配置级核验证据。

### 一条命令加源并安装

- 新增 `repo/install.sh`：认出发行版与包管理器，导入公钥、写源、装包、跑一次版本自检一并做完，
  apt / dnf / pacman / apk 四套都走各自的签名校验路径，不带任何绕过开关。官网首页换成这一条
  命令；只写源不装加 `--repo-only`，换镜像或本地目录用 `ASUS_NUMPAD_BASE` 覆盖。
- 脚本跟软件源索引同处发布，不必再手抄源地址。

### 官网源码进入主仓

- 官网源码从独立站点仓库搬进 `site/`，与代码同仓维护；`tools/push-site.sh` 把 `site/` 同步到承载
  Pages 的 `asus-numpad/asus-numpad.github.io`。`docs/` 只留项目文档，`docs/index.html` 是旧项目
  站点地址的跳转页。机型配图收敛为 `site/assets/models/` 一份，`docs/images/` 随之删除。
- 三份 README 与官网按官方 INF 校正手势参数：启动手势 `NUMPAD_07` 7000 µm、滑动阈值
  `NUMPADGESTURE_05` 4838 µm、双指滚动间距 `NUMPADGESTURE_08` 40 mm、NumLock 漂移 `NUMPAD_09`
  2741 µm、点击间隔 `NUMPADGESTURE_03/04` 350 ms；原先写的「两指间隙」并不是独立参数。
- 机型年份与型号名对齐官方规格页：`gx550` 为 2020–2023 的 Duo 15 / Duo 15 SE / Duo 16，`ux582`
  为 2020–2021 的 Zenbook Duo 14 / Pro Duo 15 OLED。中文页面用国内机型名（枪神 / 魔霸 / 冰刃 /
  灵耀），繁体页面沿用台湾市场名（Strix SCAR / Zephyrus Duo 西風之神）。
- 官网版本标签与页脚版本改为 1.0.2；三语界面文案改为陈述事实，man 页的语言范围写明只有英文。
