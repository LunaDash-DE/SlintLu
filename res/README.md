# Shared assets

Files here ship inside SlintLu and are extracted next to the `@slintlu` library, so every shell
can use them without copying them into its own repository.

- `icons/` — 24×24 stroke icons (white strokes, meant to be tinted). Use them through the
  library's `Icon` component:

  ```slint
  import { Icon } from "@slintlu";
  Icon { name: "wifi"; color: Theme.accent; width: 18px; height: 18px; }
  ```

  Any other file is reachable at runtime under `Shell.resources`, e.g.
  `Files.image(Shell.resources + "/icons/wifi.svg")`.

Add only generic, freely licensed assets (no branding). Everything under `res/` is embedded by
`build.rs` automatically; after adding or removing an icon, run `scripts/gen-icons.py` to
regenerate the `Icon` component (CI checks that it is up to date).
