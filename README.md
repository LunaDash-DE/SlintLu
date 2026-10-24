# SlintLu

Build complete Wayland desktop shells — bars, docks, launchers, notifications, wallpapers,
lock screens, overlays — in the [Slint](https://slint.dev) language. Similar in spirit to
[Quickshell](https://quickshell.outfoxxed.me/), with Slint as the UI language and Rust at the
core.

> **Status:** early development. The Wayland platform and the `slintlu` runtime work; system
> services are being added (see [Roadmap](#roadmap)).

## Features

- **Write the shell in Slint, run it with `slintlu`.** Files are compiled at runtime and
  reloaded on save; a broken edit keeps the running shell and prints diagnostics.
- **Native Wayland** through smithay-client-toolkit, no winit:
  - `wlr-layer-shell` panels with anchors, margins, exclusive zones, keyboard modes, per-screen
    instances and show/hide for popups and drawers;
  - `xdg-shell` windows for settings dialogs and the like;
  - multi-monitor with hot-plug, fractional scaling (`fractional-scale-v1` + viewporter),
    clipboard and primary selection, `cursor-shape-v1`, key repeat, virtual keyboards.
- **Three renderers with automatic fallback**

  | `--renderer` / `SLINTLU_RENDERER` | Implementation | Graphics API |
  |---|---|---|
  | `vulkan` | FemtoVG on wgpu | Vulkan |
  | `opengl` | FemtoVG | OpenGL ES 2/3 via EGL (`libEGL` loaded at runtime) |
  | `software` | Slint software renderer | `wl_shm`, no GPU required |
  | `auto` (default) | first that works: Vulkan → OpenGL → Software | |

- **Compatibility:** every compositor implementing layer-shell (niri, Hyprland, Sway, river,
  KDE Plasma, Wayfire, labwc, …) shows shell panels; others (e.g. GNOME) fall back to normal
  windows.
- **Also a Rust library:** use the platform directly with compiled Slint (`slint::slint!`) —
  see [`examples/hello_bar.rs`](examples/hello_bar.rs).

## Quick start

```bash
cargo install --path .
slintlu init        # writes ~/.config/slintlu/{shell,bar,launcher,lock}.slint
slintlu             # runs it; edit the files and watch them reload
```

A minimal panel:

```slint
import { PanelWindow, Shell } from "@slintlu";

export component Bar inherits PanelWindow {
    anchor-top: true;
    anchor-left: true;
    anchor-right: true;
    preferred-height: 32px;
    background: #1e1e2e;
    Text { text: "Hello from " + Shell.screen.name; color: white; }
}
```

Run `slintlu library-path` and add the printed directory as the `slintlu` library path in your
editor's Slint extension to get completion for `@slintlu`.

## Command line

| Command | Purpose |
|---|---|
| `slintlu [-c PATH] [-r RENDERER] [--no-reload]` | run a shell (default config: `~/.config/slintlu/shell.slint`) |
| `slintlu check [PATH]` | compile a configuration and print diagnostics |
| `slintlu init [--dir DIR] [--force]` | create a starter configuration |
| `slintlu outputs` | list monitors |
| `slintlu library-path` | print where the `@slintlu` library lives |
| `slintlu msg …` | control the running shell: `reload`, `quit`, `get`, `set`, `toggle`, `call` ([IPC](docs/IPC.md)) |

## Project layout

```
src/
├── core/         Slint platform, event loop, shared Wayland context
├── input/        seat, keyboard (xkb → Slint keys), pointer and scrolling
├── output/       monitors and hot-plug
├── render/       software / OpenGL (EGL) / Vulkan (wgpu) renderers and fallback
├── shell/        layer-shell and xdg-shell roles
├── window/       the Slint window adapter: lifecycle and frame pacing
├── compositors/  compositor detection; niri, Hyprland and Sway IPC
├── ipc/          the shell's control socket: protocol and client
├── dbus/         notification daemon, MPRIS, NetworkManager, logind, desktop portal
├── i18n/         .po parsing, .mo writing, locale detection
├── icons/        freedesktop icon theme lookup
├── idle/         ext-idle-notify and idle inhibition
├── clipboard/    copy / paste and primary selection
├── launcher/     desktop entries, search, launch history, launching
├── security/     session lock and PAM authentication
├── audio/        volume and devices (PipeWire / PulseAudio via pactl)
├── system/       CPU, memory, disks, temperatures, batteries, network, host
├── time/         clock formatting
├── util/         human readable sizes, rates and durations
├── ui/           the `@slintlu` Slint library
├── scripting/    interpreter bridge: compiling, instantiating, globals   (runtime)
├── config/       configuration paths and hot reload                      (runtime)
└── cli/          the `slintlu` command line                              (runtime)
examples/         starter configuration and Rust examples
scripts/          headless sway integration test
```

## Roadmap

- [x] Wayland platform: layer-shell, xdg-shell, outputs, input, three renderers
- [x] Runtime: interpreter, per-screen panels, hot reload, CLI
- [x] Services: clock, system stats, battery, network traffic, audio, workspaces and windows (niri / Hyprland / Sway)
- [x] IPC: control a running shell (`slintlu msg …`) from keybindings and scripts — see [docs/IPC.md](docs/IPC.md)
- [x] D-Bus: notification daemon, media players (MPRIS), NetworkManager, power (logind), dark mode and accent colour (portal)
- [x] i18n: `@tr()` with `.po` files compiled at runtime and live language switching — see [docs/I18N.md](docs/I18N.md)
- [x] Applications and icons: desktop entries, fuzzy search ranked by usage, launching (terminal apps, actions), freedesktop icon themes
- [x] Lock screen: ext-session-lock, PAM, logind lock / suspend integration — see [docs/LOCKSCREEN.md](docs/LOCKSCREEN.md)
- [x] Clipboard and primary selection, idle detection / inhibition / auto-lock, fractional scaling
- [ ] System tray, brightness, Bluetooth

## Development

Requires Rust ≥ 1.92, [`just`](https://github.com/casey/just), and the `wayland`,
`libxkbcommon`, `fontconfig` and `pam` development packages.

```bash
just           # list recipes
just test      # unit and integration tests, no compositor needed
just run       # run the starter shell in the current session
just ci        # what CI runs
```

See [docs/TESTING.md](docs/TESTING.md) for all ways to test and
[CONTRIBUTING.md](CONTRIBUTING.md) for the pull request process.

## License

GPL-3.0-only (compatible with Slint's GPL licensing option).
