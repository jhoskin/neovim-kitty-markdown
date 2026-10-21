# neovim-kitty-markdown

A markdown renderer with image and mermaid diagram support, using the kitty
graphics protocol.

## Development process

Two Claude Code sessions work on this repo, each with its own role. Both
read `AGENTS.md` at start, which holds the full rules.

- Dev agent: owns `SPEC.md`, writes the code in `src/` and the unit tests in
  `tests/`, and keeps the architecture and conventions in `AGENTS.md`. It
  has no access to `held_out/`.
- QA agent: owns `held_out/`, which holds hidden tests and evals. On each
  run it executes them, checks the code against `SPEC.md`, and writes its
  findings to `REVIEW.md` in the repo root. It has access to every folder.

The loop: the dev agent writes or updates the spec, implements it, and runs
the unit tests. The QA agent runs `held_out/` and writes `REVIEW.md`. The
dev agent reads `REVIEW.md`, fixes what it reports, and the loop repeats.

### Launch

Run both from the repo root. The dev session loads an extra settings file
that denies reads and edits under `held_out/`.

```sh
# Dev agent: no access to held_out/
claude --settings .claude/settings.dev.json

# QA agent: full access
claude
```

## Layout

- `SPEC.md`: the specification. Source of truth for behavior.
- `AGENTS.md`: roles, access rules, workflow, and conventions.
- `src/`: the implementation.
- `tests/`: unit tests written by the dev agent.
- `held_out/`: hidden tests and evals, QA agent only.
- `REVIEW.md`: QA findings from the latest run. Generated and gitignored.
