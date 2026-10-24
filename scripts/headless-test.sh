#!/usr/bin/env bash
# Runs a SlintLu shell inside a headless sway, takes a screenshot with grim and checks that a
# bar was actually drawn along the top edge.
#
# Usage: scripts/headless-test.sh [renderer] [-- command ...]
#   renderer  auto | software | opengl | vulkan   (default: software)
#   command   what to launch (default: the starter shell)
#
# Needs: sway, grim, python3.
set -euo pipefail

# Run on a private session bus so the notification daemon can be tested without touching the
# user's own (and so CI, which has no session bus, gets one).
if [[ -z "${SLINTLU_PRIVATE_BUS:-}" ]] && command -v dbus-run-session >/dev/null; then
    export SLINTLU_PRIVATE_BUS=1
    exec dbus-run-session -- "$0" "$@"
fi

renderer="${1:-software}"
shift || true
if [[ "${1:-}" == "--" ]]; then shift; fi
if [[ $# -gt 0 ]]; then
    cmd=("$@")
else
    cmd=(target/debug/slintlu --no-reload --config examples/starter)
fi

work="$(mktemp -d)"
export XDG_RUNTIME_DIR="$work/runtime"
mkdir -m 700 "$XDG_RUNTIME_DIR"
pids=()
cleanup() {
    for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
    wait 2>/dev/null || true
    if [[ "${KEEP_ARTIFACTS:-0}" != 1 ]]; then rm -rf "$work"; else echo "artifacts kept in $work"; fi
}
trap cleanup EXIT

cat > "$work/sway.conf" <<CONF
output HEADLESS-1 resolution 1280x720 background #000000 solid_color
default_border none
CONF

echo "::group::starting headless sway"
env -u WAYLAND_DISPLAY -u DISPLAY \
    WLR_BACKENDS=headless WLR_RENDERER="${WLR_RENDERER:-pixman}" WLR_LIBINPUT_NO_DEVICES=1 \
    sway --unsupported-gpu -c "$work/sway.conf" >"$work/sway.log" 2>&1 &
pids+=($!)
for _ in $(seq 100); do
    socket="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -name 'wayland-*' ! -name '*.lock' -print -quit)"
    [[ -n "$socket" ]] && break
    sleep 0.1
done
if [[ -z "${socket:-}" ]]; then
    cat "$work/sway.log"; echo "sway did not start" >&2; exit 1
fi
export WAYLAND_DISPLAY="$(basename "$socket")"
# Only talk to the headless sway, never to the compositor this script was started from.
unset NIRI_SOCKET HYPRLAND_INSTANCE_SIGNATURE I3SOCK
export XDG_CURRENT_DESKTOP=sway
for _ in $(seq 50); do
    SWAYSOCK="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -name 'sway-ipc.*.sock' -print -quit)"
    [[ -n "$SWAYSOCK" ]] && break
    sleep 0.1
done
export SWAYSOCK
echo "sway is listening on $WAYLAND_DISPLAY (IPC: ${SWAYSOCK:-none})"
echo "::endgroup::"

echo "::group::launching ${cmd[*]} with the $renderer renderer"
SLINTLU_RENDERER="$renderer" RUST_LOG="${RUST_LOG:-info}" "${cmd[@]}" >"$work/app.log" 2>&1 &
app=$!
pids+=($app)
sleep "${SETTLE_SECONDS:-4}"
if ! kill -0 "$app" 2>/dev/null; then
    cat "$work/app.log"; echo "the shell exited early" >&2; exit 1
fi
cat "$work/app.log"
echo "::endgroup::"

# Exercise the IPC socket when testing the slintlu runtime.
if [[ "$(basename "${cmd[0]}")" == slintlu ]]; then
    echo "::group::IPC"
    "${cmd[0]}" msg ping
    "${cmd[0]}" msg windows
    available="$("${cmd[0]}" msg get Workspaces.available)"
    echo "Workspaces.available = $available"
    if [[ -n "${SWAYSOCK:-}" && "$available" != true ]]; then
        echo "sway workspaces were not picked up" >&2; exit 1
    fi
    "${cmd[0]}" msg get Workspaces.all
    if [[ "$("${cmd[0]}" msg get Notifications.available)" == true ]] && command -v notify-send >/dev/null; then
        notify-send -a headless-test "Hello" "from CI"
        sleep 0.5
        count="$("${cmd[0]}" msg get Notifications.count)"
        echo "Notifications.count = $count"
        [[ "$count" == 1 ]] || { echo "the notification did not arrive" >&2; exit 1; }
        "${cmd[0]}" msg call Notifications.dismiss-all
    fi
    actual="$("${cmd[0]}" msg get Shell.renderer)"
    echo "Shell.renderer = $actual"
    if [[ "$renderer" != auto && "$actual" != "$renderer" ]]; then
        echo "asked for the $renderer renderer but got $actual" >&2; exit 1
    fi
    echo "::endgroup::"
fi

grim -t ppm "$work/shot.ppm"
cp "$work/shot.ppm" "${SCREENSHOT:-/dev/null}" 2>/dev/null || true

python3 - "$work/shot.ppm" <<'PY'
import sys

with open(sys.argv[1], "rb") as f:
    data = f.read()
# Binary PPM: "P6\n<w> <h>\n<max>\n" followed by RGB triples.
parts = data.split(maxsplit=4)
assert parts[0] == b"P6", parts[0]
width, height = int(parts[1]), int(parts[2])
pixels = parts[4]

def row_coverage(y):
    row = pixels[y * width * 3:(y + 1) * width * 3]
    lit = sum(1 for x in range(width) if row[3 * x] or row[3 * x + 1] or row[3 * x + 2])
    return lit / width

top = [row_coverage(y) for y in range(4, 28)]
bottom = [row_coverage(y) for y in range(height // 2, height // 2 + 24)]
print(f"top rows covered: {min(top):.0%}..{max(top):.0%}, middle rows: {max(bottom):.0%}")
if min(top) < 0.9:
    sys.exit("no bar found along the top edge")
if max(bottom) > 0.05:
    sys.exit("unexpected content in the middle of the screen")
print("headless render check passed")
PY
