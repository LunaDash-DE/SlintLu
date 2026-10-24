# IPC

A running `slintlu` listens on a Unix socket so scripts, compositor keybindings and other
programs can control it. The socket is `$XDG_RUNTIME_DIR/slintlu/$WAYLAND_DISPLAY.sock`
(override with `SLINTLU_SOCKET`).

## From the command line

```bash
slintlu msg ping                         # {"version": "0.1.0"}
slintlu msg reload
slintlu msg quit
slintlu msg windows                      # components and the output they are on
slintlu msg outputs
slintlu msg get Audio.output             # read any exported global property
slintlu msg set Launcher.open true       # write it (values are JSON; bare words are strings)
slintlu msg toggle Launcher.open         # flip a bool, prints the new value
slintlu msg call Audio.set-volume 0.5    # invoke a global callback or function
slintlu msg call Launcher.search firefox
```

The exit status is non-zero when the request fails.

## Exposing your own commands

Any global your shell exports is reachable. A launcher that a keybinding can open:

```slint
import { PanelWindow } from "@slintlu";

export global Launcher {
    in-out property <bool> open;
    callback search(string) -> string;
    search(text) => { /* ... */ return text; }
}

export component LauncherPanel inherits PanelWindow {
    layer: overlay;
    keyboard-mode: exclusive;
    shown: Launcher.open;
    // ...
}
```

niri (`~/.config/niri/config.kdl`):

```kdl
binds {
    Mod+D { spawn "slintlu" "msg" "toggle" "Launcher.open"; }
}
```

Hyprland: `bind = SUPER, D, exec, slintlu msg toggle Launcher.open`
Sway: `bindsym $mod+d exec slintlu msg toggle Launcher.open`

Globals the shell defines itself exist once per window (Slint globals are per component
instance); `set`, `toggle` and `call` apply to every window, `get` reads the first one.
Callbacks implemented by SlintLu (e.g. `Audio.set-volume`) run once.

## Protocol

One JSON request per line, answered by one JSON line. Requests carry a `cmd` tag:

```json
{"cmd": "ping"}
{"cmd": "reload"}
{"cmd": "quit"}
{"cmd": "windows"}
{"cmd": "outputs"}
{"cmd": "get", "target": "Audio.output"}
{"cmd": "set", "target": "Launcher.open", "value": true}
{"cmd": "toggle", "target": "Launcher.open"}
{"cmd": "call", "target": "Audio.set-volume", "args": [0.5]}
```

Responses: `{"ok": true, "result": ...}` or `{"ok": false, "error": "..."}`.

```bash
echo '{"cmd":"get","target":"Clock.time"}' | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/slintlu/$WAYLAND_DISPLAY.sock
```

Rust programs can use `slintlu::ipc::{Client, Request}` directly.
