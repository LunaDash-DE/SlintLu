# Contributing to SlintLu

1. Fork and create a branch: `git switch -c feat/my-change`.
2. Run `just ci` before pushing; it runs the same format, lint, test and doc checks as CI.
3. Open a pull request with a [Conventional Commits](https://www.conventionalcommits.org/)
   title such as `feat(audio): add per-application volume`.

Every pull request is reviewed automatically:

| Check | What it does |
|---|---|
| CI | format, clippy (all feature sets), docs, tests on stable and the MSRV, cargo-deny, typos |
| Headless sway | renders the starter shell with each renderer and checks a screenshot |
| PR review | conventional title, dependency review, clippy findings as inline comments, coverage summary, path labels |

A maintainer (see `.github/CODEOWNERS`) must approve before merging.

## Code layout

Each directory under `src/` owns one domain; keep files focused and small. Library modules
(`core`, `render`, `shell`, `system`, …) must not depend on the runtime (`cli`, `config`,
`scripting`), which is only built with the `runtime` feature.
