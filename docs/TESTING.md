# Testing

## 1. Setup

```bash
# Arch Linux
sudo pacman -S --needed rust just wayland libxkbcommon fontconfig
# Optional: headless integration test and screenshots
sudo pacman -S --needed sway grim
# Optional: the same static checks CI runs
cargo install --locked cargo-deny typos-cli
```

On Debian/Ubuntu: `sudo apt install just pkg-config libwayland-dev libxkbcommon-dev libfontconfig-dev`.

## 2. Without a compositor

```bash
just test        # unit tests, CLI integration tests, doctests
just lint        # clippy with and without the GPU renderers
just doc         # API documentation
just check       # compile the starter shell and print diagnostics
just ci          # everything above plus the format check (same as CI)
just deny        # licences and security advisories
just typos       # spelling
```

## 3. In your Wayland session

Inside a compositor with layer-shell (niri, Hyprland, Sway, …):

```bash
just run examples/starter software
just run examples/starter opengl
just run examples/starter vulkan
```

A bar appears at the top of every screen showing the compositor, the screen and the renderer
actually in use; other windows move down to make room. Edit `examples/starter/bar.slint` while
it runs — it reloads on save. Introduce a syntax error: the diagnostics are printed and the
running bar stays. Press `Ctrl+C` to quit.

More logging: `just run examples/starter auto -vv` or `RUST_LOG=slintlu=debug`:

```
INFO  slintlu::cli::run] compositor: niri, renderer: auto
INFO  slintlu::render::gpu] Vulkan adapter: NVIDIA GeForce RTX 3050 (DiscreteGpu, driver NVIDIA)
```

The Rust library example: `just example hello_bar vulkan`.

## 4. Headless integration test (same as CI)

Runs the shell in a separate headless sway, takes a screenshot with grim and checks that a bar
was drawn along the top edge. It does not touch your desktop.

```bash
just headless software
just headless vulkan                     # needs a Vulkan driver (CI uses Mesa lavapipe)
KEEP_ARTIFACTS=1 just headless opengl    # keep the screenshot and logs for debugging
```

## 5. Pull requests

Opening a pull request runs CI plus the PR review workflow (title check, dependency review,
inline clippy comments, coverage summary, labels). See [CONTRIBUTING.md](../CONTRIBUTING.md).
