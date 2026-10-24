# 測試指南

## 1. 準備

```bash
# Arch Linux
sudo pacman -S --needed rust just wayland libxkbcommon fontconfig
# 選用：headless 整合測試、截圖、靜態檢查
sudo pacman -S --needed sway grim
cargo install --locked cargo-deny typos-cli
```

## 2. 不需要合成器的測試

```bash
just test        # 單元測試 + doctest
just lint        # clippy（含 / 不含 GPU renderer 兩種 feature 組合）
just doc         # 文件建置
just ci          # 以上全部 + 格式檢查（等同 CI）
```

## 3. 在你目前的 Wayland session 實測

在 niri / Hyprland / Sway 等支援 layer-shell 的合成器內執行：

```bash
just example hello_bar software
just example hello_bar opengl
just example hello_bar vulkan
```

應該會在每個螢幕頂端出現一條 bar，右側顯示實際使用的 renderer，點擊 `clicks` 按鈕數字會增加、滑過會有動畫。
其他視窗應被 exclusive zone 往下推。`Ctrl+C` 結束。

`RUST_LOG=slintlu=debug` 會顯示 renderer 選擇與降級的過程，例如：

```
DEBUG slintlu::render] window uses the vulkan renderer
INFO  slintlu::render::gpu] Vulkan adapter: NVIDIA GeForce RTX 3050 (DiscreteGpu, driver NVIDIA)
```

## 4. Headless 整合測試（與 CI 相同）

不會影響你目前的桌面：在獨立的 headless sway 中執行，用 grim 截圖並檢查頂端是否畫出 bar。

```bash
just headless software
just headless vulkan       # 需要 Vulkan 驅動（CI 上使用 Mesa lavapipe）
KEEP_ARTIFACTS=1 just headless opengl   # 保留截圖與 log 以便除錯
```
