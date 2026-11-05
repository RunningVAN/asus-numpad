/* ============================================================
   asus-numpad — site script
   ============================================================ */
(function () {
  "use strict";

  /* ---------------- i18n ---------------- */
  var ZH = {
    "nav.features": "特性",
    "nav.how": "原理",
    "nav.models": "机型",
    "nav.install": "安装",
    "nav.faq": "问答",

    "hero.eyebrow": "用户态驱动 · GPL-3.0",
    "hero.title1": "数字小键盘，",
    "hero.title2": "在 Linux 上。无需改内核。",
    "hero.lede": "通过 evdev 抓取触摸板，把触点映射到数字键盘格位，再经 uinput 发出按键。原厂内核、原厂 ACPI —— 卸载后机器回到出厂状态。",
    "hero.cta1": "开始安装",
    "hero.cta2": "下载安装包",
    "hero.note": "软件源只需添加一次，具体一条命令见<a href=\"#install\">安装</a>。",
    "copy": "复制",

    "strip.label": "已打包适配",

    "feat.title": "尽量不打扰系统",
    "feat.sub": "不装内核模块、不需要每次更新后重编 dkms、也不改动任何固件表。",
    "feat.1.t": "不动系统组件",
    "feat.1.d": "evdev 收，uinput 发。不替换厂商模块、不覆盖 ACPI 对象，内核升级不会失效，<code>卸载</code>后即为原有行为。",
    "feat.2.t": "一个包，覆盖整条发行版线",
    "feat.2.d": "deb 在 glibc 2.35 上编译，覆盖 Ubuntu 22.04–24.04 与 Debian 12/13；rpm 在 el9 上编译，覆盖 RHEL 9–10、Fedora 与 openSUSE。发版前 CI 会把同一份包装进每个目标系统实测。",
    "feat.3.t": "运行期识别机型",
    "feat.3.d": "由 DMI 字段与触摸板模组决定格位中心与键表，因此不存在“装错包”这回事。<code>asus-numpad sku</code> 会说明识别结果和依据。",
    "feat.4.t": "四挡背光与跟随",
    "feat.4.d": "默认通过 hidraw FEATURE <code>0x0d</code> 跟随键盘背光；挡位值可在 0x41–0x48 内改写，手动模式提供 <code>bright up|down|cycle</code>。",
    "feat.5.t": "布局门禁",
    "feat.5.d": "44 个触摸板模组映射到厂商 INF 里的键表，每份标注 verified / family / unsupported。遇到不认识的模组宁可拒绝启动，也不发出错误的键。",
    "feat.6.t": "英文与两种中文",
    "feat.6.d": "界面、提示与 man 页均提供 en、zh-CN、zh-TW，取值顺序为 <code>ASUS_NUMPAD_LANG</code>、<code>--lang</code>、配置文件、系统语言环境。",

    "how.title": "工作原理",
    "how.sub": "一个小进程里的三个阶段，内核一行不改。",
    "how.1.t": "抓取触摸板",
    "how.1.d": "打开并 grab 触摸板的多点触控 evdev 节点，触点被读走，桌面指针不会跟着乱动。",
    "how.2.t": "触点映射格位",
    "how.2.d": "格位中心、按下阈值与 62 帧长按参数取自厂商文件 <code>AsusNUMPADFilter.inf</code> 的 <code>AlgoParameters</code>，按代际微调。",
    "how.3.t": "注入按键",
    "how.3.d": "通过 uinput 建立虚拟键盘并发出 KP_* 事件；背光字节随后经 hidraw 或 i2c 写出。",

    "models.title": "支持机型",
    "models.sub": "机型 id 只用于手动切换参数；一行都不匹配时走 <code>universal</code>。",
    "models.th1": "机型 id",
    "models.th2": "对应机器",
    "models.th3": "说明",
    "models.g533": "2021 款不带 NumberPad，不会被匹配",
    "models.g614": "已在 <code>G634JY</code> 真机验证",
    "models.g615": "2026 款沿用同一代号",
    "models.gx": "侧置触摸板，数字模式由键盘专用键切换",
    "models.ux": "侧置触摸板，数字模式由键盘专用键切换",
    "models.uni": "其它所有机器",
    "models.unid": "使用厂商默认参数，键表由触摸板模组决定",
    "models.fine": "DMI 里的机型代号为准，宣传名称仅供参考。硬件读不准时用 <code>asus-numpad sku g614</code> 手动指定，<code>asus-numpad sku auto</code> 恢复自动识别。",

    "inst.title": "安装",
    "inst.sub": "软件源加一次，之后随包管理器升级。仓库未做 GPG 签名，完整性校验值都在索引里。",
    "inst.apt.n": "Debian 12/13、Ubuntu 22.04–24.04、Mint 21+",
    "inst.dnf.n": "Fedora、RHEL / Rocky / Alma 9–10",
    "inst.zypper.n": "openSUSE Leap 16 与 Tumbleweed 直接读同一个 rpm-md 源",
    "inst.pacman.n": "Arch、Manjaro、EndeavourOS —— AUR 源码包名为 <code>asus-numpad-rs</code>",
    "inst.apk.n": "Alpine 3.19+（musl 构建）",
    "inst.nix.n": "Flake，无需添加源",
    "inst.other1.t": "Gentoo 与静态 tarball",
    "inst.other1.d": "Releases 里有适合 ebuild 的静态 tarball，也有 deb、rpm、pkg.tar.zst、apk 供离线安装。",
    "inst.other1.a": "前往 Releases",
    "inst.other2.t": "装完先自检",
    "inst.other2.d": "只读自检：设备、机型、布局支持级别、背光通道一目了然。",

    "use.title": "常用命令",
    "use.sub": "守护进程由 systemd 拉起，命令行用于排查与调参。",
    "use.a.t": "运行与自检",
    "use.b.t": "背光",
    "use.c.t": "布局与语言",
    "use.fine": "配置位于 <code>~/.config/asus-numpad/asus-numpad.json</code>，以 root 运行时读 <code>/etc/asus-numpad/</code>。未写的键沿用厂商默认值 —— <code>numlock</code>、<code>mixed_mode</code>、<code>grab</code>、<code>follow_keyboard_brightness</code>、<code>allow_unverified_layout</code> 以及两张亮度表。",

    "faq.title": "常见问题",
    "faq.1.q": "会替代 asusctl 或内核的 hid-asus 吗？",
    "faq.1.d": "不会，它只是并行存在：抓取触摸板 evdev 节点，另建一个虚拟键盘。asusctl 对键盘背光的控制照常可用，卸载本包即去掉数字键盘这一层。",
    "faq.2.q": "触摸板的指针功能会失效吗？",
    "faq.2.d": "仅在数字模式开启期间被 grab。把 <code>grab</code> 关掉，点击仍能到达桌面；开启 <code>mixed_mode</code> 且 <code>/dev/uinput</code> 可用时，数字键与指针可同时工作。",
    "faq.3.q": "我的机型不在表里，能用吗？",
    "faq.3.d": "不匹配的机器走 <code>universal</code>，使用厂商默认参数，键表由触摸板模组选定。若模组也不认识，布局门禁会拒绝而不是猜测，<code>asus-numpad check</code> 会打印需要反馈的信息。",
    "faq.4.q": "为什么不给安装包做 GPG 签名？",
    "faq.4.d": "单包仓库做密钥轮换的成本高于收益，索引里带有逐文件哈希。若一定要校验签名，请从 Releases 下载安装包而不是走软件源。",
    "faq.5.q": "冰刃双屏、灵耀双屏这类机型呢？",
    "faq.5.d": "支持，但有差异：这类机器的触摸板是键盘右侧的竖条，数字模式由键盘专用键切换，而不是右上角的 62 帧长按。可用 <code>numlock</code> 固定模式。",

    "foot.note": "ASUS 触摸板数字小键盘（NumberPad）的 Linux 用户态驱动。GPL-3.0-or-later。",
    "foot.project": "项目",
    "foot.machines": "机型说明",
    "foot.docs": "文档",
    "foot.install": "安装指南",
    "foot.verify": "验证记录",
    "foot.right": "© 2026 RunningVAN · GPL-3.0-or-later"
  };

  var I18N = { en: {}, zh: ZH };
  var zhNodes = document.querySelectorAll("[data-i18n]");
  // snapshot the English source text once
  zhNodes.forEach(function (el) { I18N.en[el.dataset.i18n] = el.innerHTML; });

  var langBtn = document.getElementById("lang-toggle");
  var stored = null;
  try { stored = localStorage.getItem("an-lang"); } catch (e) {}
  var lang = stored || ((navigator.language || "en").toLowerCase().indexOf("zh") === 0 ? "zh" : "en");

  function applyLang(next) {
    lang = next;
    var dict = I18N[next] || {};
    document.documentElement.lang = next === "zh" ? "zh-CN" : "en";
    zhNodes.forEach(function (el) {
      var v = dict[el.dataset.i18n];
      if (v != null) el.innerHTML = v;
    });
    langBtn.textContent = next === "zh" ? "EN" : "中";
    langBtn.title = next === "zh" ? "Switch to English" : "切换到中文";
    try { localStorage.setItem("an-lang", next); } catch (e) {}
  }
  langBtn.addEventListener("click", function () { applyLang(lang === "zh" ? "en" : "zh"); });
  applyLang(lang);

  /* ---------------- theme ---------------- */
  var themeBtn = document.getElementById("theme-toggle");
  var theme = null;
  try { theme = localStorage.getItem("an-theme"); } catch (e) {}
  document.documentElement.setAttribute("data-theme", theme === "light" ? "light" : "dark");
  themeBtn.addEventListener("click", function () {
    var next = document.documentElement.getAttribute("data-theme") === "dark" ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", next);
    try { localStorage.setItem("an-theme", next); } catch (e) {}
  });

  /* ---------------- numpad visual ---------------- */
  var keys = ["NumLk", "/", "*", "-", "7", "8", "9", "+", "4", "5", "6", "+", "1", "2", "3", "Ent", "0", "00", ".", "Ent"];
  var grid = document.getElementById("pad-grid");
  if (grid) {
    keys.forEach(function (k, i) {
      var d = document.createElement("div");
      d.className = "cell" + (k.length > 1 ? " op" : "");
      d.textContent = k;
      d.dataset.i = i;
      grid.appendChild(d);
    });
    var cells = grid.children;
    var seq = [0, 4, 9, 13, 17, 18, 15, 11, 6, 2];
    var step = 0;
    if (!window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setInterval(function () {
        for (var i = 0; i < cells.length; i++) cells[i].classList.remove("lit");
        cells[seq[step % seq.length]].classList.add("lit");
        step++;
      }, 620);
    } else {
      cells[4].classList.add("lit");
    }
  }

  /* ---------------- copy buttons ---------------- */
  function copyText(text, btn) {
    var done = function () {
      var old = btn.querySelector("span") ? btn.querySelector("span").textContent : btn.textContent;
      btn.classList.add("done");
      if (btn.querySelector("span")) btn.querySelector("span").textContent = lang === "zh" ? "已复制" : "Copied";
      else btn.textContent = lang === "zh" ? "已复制" : "Copied";
      setTimeout(function () {
        btn.classList.remove("done");
        if (btn.querySelector("span")) btn.querySelector("span").textContent = old;
        else btn.textContent = old;
      }, 1600);
    };
    if (navigator.clipboard && window.isSecureContext) {
      navigator.clipboard.writeText(text).then(done, function () { fallback(text, done); });
    } else {
      fallback(text, done);
    }
  }
  function fallback(text, done) {
    var ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand("copy"); done(); } catch (e) {}
    document.body.removeChild(ta);
  }

  var quickBody = document.querySelector(".quick-body");
  if (quickBody) {
    quickBody.querySelector(".copy-btn").addEventListener("click", function () {
      var shown = quickBody.querySelector(".qcode:not(.hidden)");
      if (shown) copyText(shown.textContent.trim(), this);
    });
  }

  var pane = document.querySelector(".install");
  if (pane) {
    pane.querySelector(".copy-pane").addEventListener("click", function () {
      var shown = pane.querySelector(".pane:not(.hidden)");
      if (shown) copyText(shown.querySelector("pre").textContent.replace(/\s+$/, ""), this);
    });
  }

  /* ---------------- quick tabs ---------------- */
  var qtabs = document.querySelectorAll(".qtab");
  qtabs.forEach(function (t) {
    t.addEventListener("click", function () {
      qtabs.forEach(function (x) { x.classList.toggle("active", x === t); });
      document.querySelectorAll("[data-qt-body]").forEach(function (b) {
        b.classList.toggle("hidden", b.dataset.qtBody !== t.dataset.qt);
      });
    });
  });

  /* ---------------- install tabs ---------------- */
  var tabs = document.querySelectorAll(".tab");
  tabs.forEach(function (t) {
    t.addEventListener("click", function () {
      tabs.forEach(function (x) { x.classList.toggle("active", x === t); });
      document.querySelectorAll("[data-pane]").forEach(function (p) {
        p.classList.toggle("hidden", p.dataset.pane !== t.dataset.tab);
      });
    });
  });

  /* ---------------- nav shadow ---------------- */
  var nav = document.getElementById("nav");
  var onScroll = function () {
    nav.classList.toggle("scrolled", window.scrollY > 8);
  };
  window.addEventListener("scroll", onScroll, { passive: true });
  onScroll();

  /* ---------------- reveal ---------------- */
  var reveals = document.querySelectorAll(".reveal");
  if ("IntersectionObserver" in window) {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) {
          e.target.classList.add("in");
          io.unobserve(e.target);
        }
      });
    }, { rootMargin: "0px 0px -8% 0px", threshold: .08 });
    reveals.forEach(function (el, i) {
      el.style.transitionDelay = (Math.min(i % 6, 5) * 45) + "ms";
      io.observe(el);
    });
  } else {
    reveals.forEach(function (el) { el.classList.add("in"); });
  }
})();
