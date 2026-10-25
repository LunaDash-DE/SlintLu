# Contributing to SlintLu

1. Fork and create a branch: `git switch -c feat/my-change`.
2. Run `just ci` (format, lint, tests, docs) and `just headless` before pushing: CI only builds.
3. Open a pull request with a [Conventional Commits](https://www.conventionalcommits.org/)
   title such as `feat(audio): add per-application volume`.

Every pull request is checked automatically:

| Check | What it does |
|---|---|
| CI | builds on stable and the MSRV, without GPU renderers, and the Nix package |
| PR review | conventional title, dependency review, path labels |

A maintainer (see `.github/CODEOWNERS`) must approve before merging.

## Code layout

Each directory under `src/` owns one domain; keep files focused and small. Library modules
(`core`, `render`, `shell`, `system`, …) must not depend on the runtime (`cli`, `config`,
`scripting`), which is only built with the `runtime` feature.
