# asus-numpad

English | [简体中文](README.zh-CN.md)

Linux userspace driver for the ASUS NumberPad. The touchpad is grabbed through evdev, contacts
are mapped to numpad cells, key events are injected through uinput. No kernel module is replaced,
no ACPI table is patched; uninstalling restores stock behaviour.

Timing and geometry constants come from `AlgoParameters` in the vendor file `AsusNUMPADFilter.inf`.

## FAQ

### Why is support limited to ASUS models?

The driver does not depend on ASUS-specific interfaces: input comes from the touchpad's evdev node,
keys go out through uinput, and no vendor kernel module or ACPI object is involved, so any multitouch
pad follows the same code path. A port means replacing the model entry (DMI `product_name` plus the
touchpad HID name) and the layout constants (cell centres, thresholds).

What cannot be ported is the hardware. The NumberPad panel provides two things other OEM touchpads
generally do not:

- **Addressable backlight.** Cell position and number-mode state are indicated by an LED pattern
  driven through HID FEATURE report `0x0d` (`[0d 14 03 <v> ad]`, `0x41`..`0x48` select the brightness
  step). Without it, keys are still emitted, but nothing indicates which cell a finger is on or
  whether number mode is active.
- **A factory calibration table.** `AlgoParameters` in `AsusNUMPADFilter.inf` fixes frame length
  (8 ms), geometry unit (0.1 mm), tap window (350 ms), NumLock long press (62 frames), swipe
  threshold (4838 um) and two-finger gap (40 mm). Without it those values have to be fitted on a
  panel that was never tuned for numpad use, and tap and swipe recognition will not match the
  Windows driver.

Support is therefore limited to models that have both, and `src/sku.rs` is the only place where that
is decided. A panel that has both can be added by editing that table and the layout constants.
Patches are welcome.

### Why a userspace driver instead of a kernel module?

- The panel is attached through ELAN i2c-hid (`04F3:3275` / `ASUE1416`), not through the ASUS `0B05`
  keyboard controller, so no existing kernel module covers it and none has to be replaced to add
  support.
- Backlight control is a HID FEATURE report on the touchpad's own hidraw node, which a userspace
  process can issue without kernel changes.
- Timing, geometry and threshold values are taken from the vendor INF verbatim, so state machine
  behaviour matches the Windows driver, and per-model differences stay in one table.
- The boot path is untouched. Uninstalling removes the service unit and the udev rule and the pad
  returns to stock behaviour; kernel upgrades cannot regress it.

### Will this be merged into asusctl?

No.

- Only a small number of ASUS models ship this touchpad.
- Once the package for the matching model is installed there is little left to adjust: the numpad
  works as it is, so adding it to asusctl offers little to the rest of its users.
- The maintainer does not have the time to adapt it to asusctl's architecture and release cycle.

Contributions are welcome either way: patches to this project, or an implementation of these features
in asusctl.

## How it works

### Data path

```
touchpad --evdev(/dev/input/eventN)--> frame buffer (8 ms) --> state machine --uinput(/dev/uinput)--> key events
                                                                   |
                                                                   `--> hidraw FEATURE report --> numpad backlight
```

### Timing and geometry

| Parameter | Value | Meaning |
| --- | --- | --- |
| Frame length | 8 ms | Time base of the state machine; a long press is 62 frames (496 ms) |
| Length unit | 0.1 mm | Unit for layout geometry and thresholds |
| Tap duration | 350 ms | Separates a tap from a swipe |
| NumLock long press | 62 frames | Toggles number mode from the icon area |
| Swipe threshold | 4838 µm | Movement below this is not a swipe |
| Two-finger gap | 40 mm | Detects two-finger scrolling |
| Launch gesture | 7000 µm | Distance that triggers the calculator hot zone |

### Key layout

Keymaps are selected by touchpad module and HID name, not by laptop model. `HID_LAYOUTS` holds 44
HID-name-to-keymap entries and covers every module in the vendor `SupportList`; module prefixes
such as `ASUE1416` and `ASCF1400` appear in `/sys/class/input/*/name`.

The numpad sits in the top-right corner of the touchpad, 5 rows by 4 columns. Cell centres are
the coordinates of `NUMPADLAYOUT_GH3XLNDZ_00..14` in 0.1 mm units. Coordinate to cell to keycode
conversion has no hardware dependency and is unit tested.

### Backlight

```
report: 0d 14 03 <level> ad   # 6 bytes, HID FEATURE report ID 0x0d
level:  0x41..0x48            # steps 1..8
        0x00                  # off
        0x01                  # on, current step kept
write:  HIDIOCSFEATURE        # no 0x60 unlock required
```

`bright follow on` reads `current` and `max` from `/sys/class/leds/asus::kbd_backlight/brightness`,
maps the ratio onto steps 1..8, polls every 500 ms and writes only on a step change. Manual steps
and follow mode share one step table.

Node selection (`led_policy.rs`): the OMNI receiver `0b05:1ace` is always excluded, ELAN `04f3:*`
with FEATURE `0x0d` takes priority, then ASUS `0b05:*`, then i2c (`0x38` / `0x15`).

### Permissions

No root. udev tags hidraw and event nodes with `TAG+="uaccess"`, the daemon runs as a
`systemd --user` unit, `/dev/uinput` is granted by the session.

## Source layout

| Path | Purpose |
| --- | --- |
| `src/main.rs` | CLI parsing and wiring |
| `src/engine.rs` | Vendor algorithm state machine |
| `src/layout.rs` | Layout geometry, 44 HID entries, coordinate conversion |
| `src/config.rs` | Defaults, load and save, migration of old paths |
| `src/brightness.rs` | Keyboard backlight to numpad step mapping |
| `src/led_policy.rs` | Backlight node scoring |
| `src/sku.rs` | Model gate |
| `src/i18n.rs` | Three languages, overridable with external `.lang` files |
| `src/linux/` | evdev, uinput, hidraw and i2c backends |
| `lang/` | zh-CN, zh-TW, en strings |
| `packaging/` | deb, rpm, Arch, Alpine, Gentoo, Nix, tarball scripts and repo tools |

## Installation

```bash
# Debian / Ubuntu
echo "deb [trusted=yes] https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/asus-numpad.list
sudo apt update && sudo apt install asus-numpad

# Fedora / RHEL / Rocky / Alma
sudo tee /etc/yum.repos.d/asus-numpad.repo >/dev/null <<'EOF'
[asus-numpad]
name=asus-numpad
baseurl=https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/rpm/$basearch
enabled=1
gpgcheck=0
EOF
sudo dnf install asus-numpad

# openSUSE Leap / Tumbleweed, same rpm-md repo, zypper reads it directly
sudo zypper addrepo --no-gpgcheck --refresh \
  "https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/rpm/$(uname -m)" asus-numpad
sudo zypper install asus-numpad

# Arch / Manjaro
sudo tee -a /etc/pacman.conf >/dev/null <<'EOF'

[asus-numpad]
SigLevel = Optional TrustAll
Server = https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/arch/$arch
EOF
sudo pacman -Sy asus-numpad

# Arch / Manjaro, source package asus-numpad-rs
# PKGBUILD and asus-numpad-rs.install from the release page in one directory:
makepkg -si
```

One package per ecosystem. The deb is built on Ubuntu 22.04 and covers 22.04 to 24.04 plus Debian
12 and 13. The rpm is built on AlmaLinux 9 and covers RHEL, AlmaLinux, Rocky 9 to 10, Fedora,
openSUSE Leap 16 and Tumbleweed: the spec uses standard macros only, runtime dependencies are glibc
and systemd, and the repository is rpm-md, which zypper reads directly. Nothing is rebuilt per
distribution. Native packages are also on
[Releases](https://github.com/RunningVAN/asus-numpad/releases): deb, rpm, pkg.tar.zst, PKGBUILD with
.SRCINFO, apk, static tarball. Alpine, Gentoo, NixOS and manual tarball steps are in
[docs/install.md](docs/install.md).

CI installs every package in a clean container for each target distribution and runs `--version`;
a package that fails to install blocks the release.

There is one package. The model is detected at runtime: the same binary reads DMI fields and the
touchpad module, then picks the matching keymap, so there is no package to choose and nothing to get
wrong. If the hardware cannot be read reliably (virtual machines, whitebox chassis, BIOS placeholder
strings), pin the model by hand, see [Model detection](#model-detection). Runtime dependency: glibc,
or musl on Alpine.

## Usage

```bash
asus-numpad check          # read-only self test: devices, layout conversion, backlight candidates
asus-numpad dump           # raw event stream
asus-numpad run            # foreground, started by the systemd unit
asus-numpad --dump-config  # effective configuration
```

```bash
asus-numpad bright 0|1|2|3      # off, low, medium, high (fixed step, follow disabled)
asus-numpad bright up|down|cycle
asus-numpad bright follow on|off
asus-numpad bright status
```

```bash
asus-numpad lang [zh-CN|zh-TW|en|auto]
asus-numpad --lang en check     # this invocation only
```

Language precedence: `ASUS_NUMPAD_LANG`, `--lang`, `language` in the config, system locale.

## Configuration

`~/.config/asus-numpad/asus-numpad.json`, or `/etc/asus-numpad/` when run by root. Unwritten keys
use vendor defaults.

| Key | Value | Meaning |
| --- | --- | --- |
| `numlock` | `sync` / `toggle` / `off` | Follow the system NumLock state |
| `calc_gesture` | `true` / `false` | Gesture launches the calculator |
| `calc_gesture_mode` | `long` / `off` | Trigger type |
| `mixed_mode` | `auto` / `on` / `off` | Keys and pointer together, needs `/dev/uinput` |
| `grab` | `true` / `false` | Grab the touchpad; when off, clicks reach the desktop |
| `led_kind` | `auto` / `hidraw` / `i2c` / `none` | Backlight channel |
| `follow_keyboard_brightness` | `true` / `false` | Mirror the keyboard backlight, default `true` |
| `manual_brightness_step` | `0`..`4` | Current manual step (0 = off), used when follow is off |
| `manual_brightness_levels` | array | Step to value table, default `[0, 0x43, 0x45, 0x48]`; entries `0` or `0x41`..`0x48`; length 1 to 8 sets the step count; equal neighbours allowed; a decreasing table is discarded |
| `brightness_levels` | array | Hardware steps, default `0x41`..`0x48` |
| `brightness_poll_ms` | ms | Poll interval, default `500` |
| `allow_unverified_layout` | `true` / `false` | Accept an unlisted layout of a known family |
| `sku` | model id / `null` | Pin a model and skip detection; `null` means auto |

## Supported models

| Model id | Models |
| --- | --- |
| `g533` | Strix SCAR 15 / Strix G15 (2022): `G533`, `G513` |
| `g614` | Strix SCAR 16 / Strix G16 (2023): `G614`, `G634` |
| `g615` | Strix SCAR 16 / Strix G16 (2025 and later): `G615`, `G635` |
| `gx550` | Zephyrus Duo 15 and 16: `GX550`, `GX551`, `GX650` |
| `ux582` | Zenbook Duo: `UX582`, `UX482` |
| `universal` | Anything else: vendor defaults, keymap picked by touchpad module |

Model ids are there to switch parameters by hand (`asus-numpad sku <id>`) and to show what
auto-detection picked; they never block startup. A machine that matches no row simply runs on
`universal`, with the keymap still chosen by the actual touchpad module. Only `G634` has been on a
physical machine; the other rows map from touchpad module data and keymap tables.

The model code in DMI is authoritative; marketing names are for orientation only. The old per-model
package names (`asus-numpad-g614` and friends) stay in `Obsoletes`, so upgrades move to `asus-numpad`.

### Model detection

Detection uses the union of the DMI fields `product_name`, `board_name` and `product_family`. BIOS
placeholder strings count as missing and do not block startup; a vendor field that is clearly not
ASUS blocks startup.

Touchpad module name (such as `ASUE1416`, read from i2c-hid and HID) is the second signal, used
when DMI names no model. Neither signal pointing at a model means `universal`. Module and keymap per
model: [docs/machines.md](docs/machines.md).

When the hardware cannot be read reliably, pin the model:

```bash
asus-numpad sku            # what was detected, and on what evidence
asus-numpad sku list       # available model ids
asus-numpad sku g614       # pin a model, persisted to the config
asus-numpad sku auto       # back to auto-detection
```

`sku` in the config wins over detection; `--sku g614` applies to one invocation only.

## Building

```bash
./packaging/build.sh                        # current distribution
./packaging/build.sh deb rpm tarball        # explicit targets (deb/rpm/arch/alpine/tarball)
./tools/repo-gen.sh                         # dist/ to local repositories in repo/ (apt/rpm/pacman/apk)
```

```bash
RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes" \
  cargo build --target x86_64-unknown-linux-musl --release
```

Packaging details: [packaging/README.md](packaging/README.md). A `v*` tag runs
`.github/workflows/release.yml`, which builds in per-distribution containers, generates four
repository indexes, publishes the release and commits `repo/` back to `main`.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

Copyright (C) 2026 RunningVAN
*（内容由AI生成，仅供参考）*
