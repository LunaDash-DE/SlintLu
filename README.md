# SlintLu

用 [Slint](https://slint.dev) 語言打造完整的 Wayland 桌面 shell（bar、dock、launcher、通知、桌布、overlay…），概念類似 [Quickshell](https://quickshell.outfoxxed.me/)，但介面語言是 Slint、核心是 Rust。

> 狀態：早期開發中。目前完成的是 Slint 的 Wayland 平台層；載入 `.slint` 設定檔的 `slintlu` 執行程式正在開發。

## 特色

- **Wayland 原生**：直接透過 smithay-client-toolkit 與合成器溝通，不經過 winit。
  - `wlr-layer-shell`：bar / panel / 桌布 / overlay，支援 anchor、margin、exclusive zone、鍵盤模式、指定螢幕。
  - `xdg-shell`：一般應用程式視窗（設定視窗等）。
  - 多螢幕、熱插拔、整數縮放、`cursor-shape-v1`、按鍵重複。
- **三種 renderer，自動降級**

  | `--renderer` / `SLINTLU_RENDERER` | 實作 | 圖形 API |
  |---|---|---|
  | `vulkan` | FemtoVG on wgpu | Vulkan |
  | `opengl` | FemtoVG | OpenGL ES 2/3（EGL，執行期載入 `libEGL`） |
  | `software` | Slint 軟體 renderer | `wl_shm`，不需要 GPU |
  | `auto`（預設） | 依序嘗試 Vulkan → OpenGL → Software | |

  任何 GPU 初始化失敗都會退回下一個 renderer，shell 永遠能啟動。
- **相容性**：支援 layer-shell 的合成器（niri、Hyprland、Sway、river、KDE Plasma、Wayfire、labwc…）都能顯示 shell 元件；不支援 layer-shell 的合成器（如 GNOME）會退回一般視窗。

## 專案結構

```
src/
├── core/      Slint Platform、事件迴圈、共享 Wayland context
├── input/     seat、鍵盤（xkb → Slint 按鍵）、滑鼠／滾輪
├── output/    螢幕列舉與熱插拔
├── render/    software / opengl (EGL) / vulkan (wgpu) renderer 與自動降級
├── shell/     layer-shell 與 xdg-shell 角色、設定型別
└── window/    Slint WindowAdapter：生命週期、frame pacing
examples/      以 Rust 使用 slintlu 的範例
scripts/       headless sway 整合測試
```

## 開發

需要 Rust ≥ 1.92、[`just`](https://github.com/casey/just)、以及 `wayland`、`libxkbcommon`、`fontconfig` 開發套件。

```bash
just            # 列出所有指令
just test       # 單元測試（不需要合成器）
just example hello_bar vulkan   # 在目前的 Wayland session 開一條測試 bar
just ci         # 本機跑一次 CI 會跑的檢查
```

詳細測試方法見 [docs/TESTING.md](docs/TESTING.md)。

## 授權

GPL-3.0-only（與 Slint 的 GPL 授權選項相容）。
