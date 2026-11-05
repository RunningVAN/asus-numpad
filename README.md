# asus-numpad

English | [简体中文](README.zh-CN.md)

A userspace Linux driver that turns the ASUS NumberPad into a numeric keypad. The driver
grabs the touchpad through evdev, maps contact positions to numpad cells and injects the
matching key events through uinput. No kernel module is replaced and no ACPI table is
patched, so removing the package restores the stock behaviour.

Timing and geometry constants follow `AlgoParameters` in the vendor file `AsusNUMPADFilter.inf`.

## How it works

### Data path

```
touchpad --evdev(/dev/input/eventN)--> frame buffer (8 ms) --> state machine --uinput(/dev/uinput)--> key events
                                                                   |
                                                                   `--> hidraw FEATURE report --> numpad backlight
```

The touchpad is grabbed exclusively. Contacts are converted to cells, cells to keycodes, and
the rest of the system sees an ordinary keyboard.

### Timing and geometry

| Parameter | Value | Meaning |
| --- | --- | --- |
| Frame length | 8 ms | Time base of the state machine; a long press is 62 frames (496 ms) |
| Length unit | 0.1 mm | Unit for all layout geometry and thresholds |
| Tap duration | 350 ms | Separates a tap from a swipe |
| NumLock long press | 62 frames | Toggles number mode from the icon area |
| Swipe threshold | 4838 µm | Movement below this is not treated as a swipe |
| Two-finger gap | 40 mm | Detects two-finger scrolling |
| Launch gesture | 7000 µm | Distance that triggers the calculator hot zone |

### Key layout

Keymaps are selected by touchpad module and HID name, not by laptop model. The `ASUE1416` and
`ASCF1400` prefixes in `/sys/class/input/*/name` are module vendor codes, and a given module
reports the same descriptor on every model it ships in. The driver therefore keeps one
HID-name-to-keymap table (`HID_LAYOUTS`, 44 entries, covering every module in the vendor
`SupportList`).

The numpad occupies the top-right corner of the touchpad as 5 rows by 4 columns. Cell centres
are the coordinates given by `NUMPADLAYOUT_GH3XLNDZ_00..14` in 0.1 mm units. Coordinate to cell
to keycode conversion is self-contained and covered by unit tests that run without hardware.

### Backlight

The numpad backlight is a separate channel from key events:

```
report: 0d 14 03 <level> ad   # 6 bytes, HID FEATURE report ID 0x0d
level:  0x41..0x48            # steps 1..8
        0x00                  # off
        0x01                  # on, keep current step
write:  HIDIOCSFEATURE        # no 0x60 unlock required
```

`bright follow on` reads `current` and `max` from
`/sys/class/leds/asus::kbd_backlight/brightness`, maps the ratio linearly onto steps 1..8, polls
every 500 ms and writes only when the step changes. Manual steps and follow mode share the same
step table, so toggling follow does not cause a brightness jump.

Node selection (`led_policy.rs`) scores candidates. The OMNI wireless receiver `0b05:1ace` is
always excluded, ELAN `04f3:*` with FEATURE `0x0d` comes first, then ASUS `0b05:*`, and i2c
(`0x38` / `0x15`) last.

### Permissions

Root is not required. The udev rules tag hidraw and event nodes with `TAG+="uaccess"`, the
daemon runs as a `systemd --user` unit, and `/dev/uinput` is granted by the session.

## Source layout

| Path | Purpose |
| --- | --- |
| `src/main.rs` | Command line parsing and wiring |
| `src/engine.rs` | Vendor algorithm state machine, no system calls |
| `src/layout.rs` | Layout geometry, 44 HID to keymap entries, coordinate conversion |
| `src/config.rs` | Defaults, load and save, migration of old paths |
| `src/brightness.rs` | Keyboard backlight to numpad step mapping |
| `src/led_policy.rs` | Backlight node scoring and selection |
| `src/sku.rs` | Model gate, refuses to start on a mismatched machine |
| `src/i18n.rs` | Three languages, overridable with external `.lang` files |
| `src/linux/` | evdev, uinput, hidraw and i2c backends |
| `lang/` | Simplified Chinese, Traditional Chinese and English strings |
| `packaging/` | deb, rpm, Arch, Alpine, Gentoo, Nix and tarball scripts, plus repository tools |

## Installation

Add the repository once and the package follows system upgrades:

```bash
# Debian / Ubuntu
echo "deb [trusted=yes] https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/asus-numpad.list
sudo apt update && sudo apt install asus-numpad

# Fedora / RHEL / openSUSE
sudo tee /etc/yum.repos.d/asus-numpad.repo >/dev/null <<'EOF'
[asus-numpad]
name=asus-numpad
baseurl=https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/rpm/$basearch
enabled=1
gpgcheck=0
EOF
sudo dnf install asus-numpad

# Arch / Manjaro
sudo tee -a /etc/pacman.conf >/dev/null <<'EOF'

[asus-numpad]
SigLevel = Optional TrustAll
Server = https://raw.githubusercontent.com/RunningVAN/asus-numpad/main/repo/arch/$arch
EOF
sudo pacman -Sy asus-numpad

# Arch / Manjaro, source package asus-numpad-rs
# Put PKGBUILD and asus-numpad-rs.install from the release page in one directory:
makepkg -si
# Once the package is on AUR:
yay -S asus-numpad-rs
```

There is one package per ecosystem. The deb is built on Ubuntu 22.04 and covers 22.04 to 24.04
plus Debian 12 and 13. The rpm is built on AlmaLinux 9 and covers RHEL, AlmaLinux and Rocky 9 to
10, Fedora and openSUSE.

Packages can also be downloaded from [Releases](https://github.com/RunningVAN/asus-numpad/releases)
in the native format (deb, rpm, pkg.tar.zst, PKGBUILD with .SRCINFO, apk, static tarball).
Alpine, Gentoo, NixOS, AUR, manual tarball install, upgrade and removal are documented in
[docs/install.md](docs/install.md).

Before a release, CI installs every package in a clean container for each target distribution
and runs `--version`; a package that fails to install blocks the release.

Packages are split per model and refuse to start on the wrong machine (exit code 3). Check the
model code first:

```bash
cat /sys/class/dmi/id/product_name      # e.g. G634JY, use the g614 package
```

If you are unsure, install `asus-numpad`, which skips the model check.

The only runtime dependency is glibc, or musl on Alpine; no Rust runtime is pulled in.

## Usage

```bash
asus-numpad check          # read-only self test: devices, layout conversion, backlight candidates
asus-numpad dump           # raw event stream
asus-numpad run            # run in the foreground, this is what the systemd unit starts
asus-numpad --dump-config  # effective configuration
```

Brightness:

```bash
asus-numpad bright 0|1|2|3      # off, low, medium, high (fixed step, disables follow)
asus-numpad bright up|down|cycle
asus-numpad bright follow on|off
asus-numpad bright status
```

Language:

```bash
asus-numpad lang [zh-CN|zh-TW|en|auto]
asus-numpad --lang en check     # applies to this invocation only
```

Precedence: `ASUS_NUMPAD_LANG`, then `--lang`, then `language` in the configuration file, then
the system locale.

## Configuration

`~/.config/asus-numpad/asus-numpad.json`, or `/etc/asus-numpad/` when run by root.

| Key | Value | Meaning |
| --- | --- | --- |
| `numlock` | `sync` / `toggle` / `off` | Follow the system NumLock state |
| `calc_gesture` | `true` / `false` | Gesture launches the calculator |
| `calc_gesture_mode` | `long` / `off` | How the gesture is triggered |
| `mixed_mode` | `auto` / `on` / `off` | Keys and pointer together, needs `/dev/uinput` |
| `grab` | `true` / `false` | Grab the touchpad; when off, clicks reach the desktop |
| `led_kind` | `auto` / `hidraw` / `i2c` / `none` | Backlight channel |
| `follow_keyboard_brightness` | `true` / `false` | Mirror the keyboard backlight, default `true` |
| `manual_brightness_step` | `0`..`4` | Current manual step (0 = off), used when follow is disabled |
| `manual_brightness_levels` | array | Step to value table, default `[0, 0x43, 0x45, 0x48]`. Each entry may be `0` or `0x41`..`0x48`; length 1..8 sets the number of steps; equal neighbours are allowed; a decreasing table is discarded and the default is used |
| `brightness_levels` | array | Hardware steps, default `0x41`..`0x48` |
| `brightness_poll_ms` | milliseconds | Poll interval, default `500` |
| `allow_unverified_layout` | `true` / `false` | Accept an unlisted layout of a known family |

Keys that are not written fall back to the vendor defaults.

## Supported models

| Package | Models |
| --- | --- |
| `g533` | Strix SCAR 15, Strix G15 (2022): `G533`, `G513` |
| `g614` | Strix SCAR 16, Strix G16 (2023): `G614`, `G634` |
| `g615` | Strix SCAR 16, Strix G16 (2025 and later): `G615`, `G635` |
| `gx550` | Zephyrus Duo 15 and 16: `GX550`, `GX551`, `GX650` |
| `ux582` | Zenbook Duo: `UX582`, `UX482` |

The model code in DMI is authoritative. Regional marketing names differ and are only given for
orientation.

A package serves one touchpad module family, not one machine SKU. Old package names that were
merged are kept in `Obsoletes` so upgrades replace them automatically, and the model packages
declare `Conflicts` against each other.

### Models without a NumberPad

These machines have no virtual numpad in the touchpad, so no package helps. The driver and
`packaging/install-sku.sh` refuse with exit code `4` and an explanation.

| Model | Reason |
| --- | --- |
| ProArt P16 `H7606` | The touchpad is the ASUS DialPad. The specification lists DialPad support; it uses the knob HID usage rather than the numpad backlight FEATURE `0x0d` |
| Strix G17 `G713`, 17-inch | Full physical numpad on the keyboard; the specification lists `NumberPad` as `N/A` |
| 18-inch Plus, `G834`, `G835`, `G814`, `G815`, `G813` | Full physical numpad on the keyboard |

### Model detection

Detection uses the union of the DMI fields `product_name`, `board_name` and `product_family`.
BIOS placeholder strings are treated as missing and do not block startup. A vendor field
(`sys_vendor`, `board_vendor`) that is clearly not ASUS blocks startup. When the wrong package is
installed, `check` prints the package name that matches the longest common model prefix.

Hardware is a second signal. The driver reads the touchpad module name, such as `ASUE1416`, from
i2c-hid and HID. It is used when DMI says nothing useful. When DMI gives a model code that is
known but does not belong to the package, the hardware signal does not override it.

Module and keymap per model: [docs/machines.md](docs/machines.md).

## Building

```bash
./packaging/build.sh                        # package for the current distribution
./packaging/build.sh deb rpm tarball        # explicit targets (deb/rpm/arch/alpine/tarball)
./packaging/build-skus.sh g614              # single model
./tools/repo-gen.sh                         # dist/ to local repositories in repo/ (apt/rpm/pacman/apk)
```

Static build for distribution across distributions:

```bash
ASUS_NUMPAD_SKU=g614 RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes" \
  cargo build --target x86_64-unknown-linux-musl --release
```

Packaging details: [packaging/README.md](packaging/README.md). Pushing a `v*` tag runs
`.github/workflows/release.yml`, which builds in per-distribution containers, generates the four
repository indexes, publishes the release and commits `repo/` back to `main`.

## Known limitations

1. **Manual steps are sampled from a hardware table.** `brightness_levels` lists every hardware
   step (`0x41`..`0x48` by default). `manual_brightness_levels` maps the manual steps onto that
   axis and defaults to four steps, `[0, 0x43, 0x45, 0x48]`, from off to maximum. Entries may be
   `0` or `0x41`..`0x48`, the table length sets the step count (1 to 8), and neighbours may be
   equal; a decreasing table is discarded. Follow mode normalises the keyboard backlight onto the
   same table, and the numpad never gets brighter than the keyboard.
2. **NumLock long press threshold.** The vendor value `iNumLockMaxVaildDist = 2741` is 0.27 mm in
   0.1 mm units, which conflicts with "press and hold without moving". The value is used as given;
   if a long press is recognised as a swipe, start here.
3. **Icon area boundary.** The vendor layout describes the icon cell and the calculator hot zone
   with an overlap. The whole cell is assigned to the NumLock trigger.
4. **The backlight cannot be read back.** `HIDIOCGFEATURE` always returns zeros, so the light state
   is known only from the written report and the driver status line.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

Copyright (C) 2026 RunningVAN
