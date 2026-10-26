# QA agent instructions

This file loads when the QA agent opens a file under `held_out/`. The dev
agent cannot read it.

## Layout

- `held_out/run.lua`: the QA run. It builds the helper into
  `held_out/out/target`, runs every hidden spec, runs the dev gates, and
  writes `REVIEW.md`.
- `held_out/lib/qa.lua`: the check API (`QA.check`, assertions, helpers).
  `held_out/lib/run_file.lua` runs one spec file in its own `nvim` process.
- `held_out/tests/<area>/*_spec.lua`: hidden checks. They run against the
  plugin in `lua/` and `plugin/` and the helper built from `helper/`.
- `held_out/evals/<area>/`: fixtures, sample documents, the mermaid parity
  corpus and its oracle goldens, and `open_questions.md` files.
- `held_out/out/`: build output, results, and the private
  `details*.md` files. Gitignored.

The tests use the toolchain in `doc/CONVENTIONS.md`: Lua under `nvim -l`,
and the helper binary.

## A QA run

1. Read `SPEC.md` and `doc/CONVENTIONS.md`.
2. Add or update checks under `held_out/` so every requirement ID has at
   least one check. `held_out/out/details.md` lists uncovered IDs.
3. Run everything:

   ```sh
   PATH=/opt/homebrew/bin:$HOME/.cargo/bin:$PATH nvim --clean -l held_out/run.lua
   ```

   Options: `--spec <path part>`, `--filter <text or R-ID>`, `--no-build`,
   `--no-gates`, `--no-review`, `--jobs N`. `QA_PHASE` overrides the
   delivered mermaid phase, which otherwise comes from the `phase` features
   in the justfile.
4. Triage `held_out/out/details.md`: a failure caused by a wrong check is
   fixed in the check, not reported. Harness errors are QA bugs.
5. `run.lua` writes `REVIEW.md`. Read it before you hand it over.

The QA agent does not change `lua/`, `plugin/`, `doc/`, `helper/`,
`tests/`, `SPEC.md`, or the root `AGENTS.md`. A problem in one of those is
a finding.

## Writing checks

`QA.check(reqs, hint, [opts], fn)`. The hint is the only text of a check
that reaches `REVIEW.md`. It states the expected behavior for a class of
input in the words of the spec. It never names the exact input, so the dev
agent fixes the behavior and not the case. Inputs and assertion details go
to the private details file only.

- `opts.severity`: `high` (default), `medium`, or `low`.
- `opts.phase`: the mermaid phase of the check. It is not due before the
  delivered phase reaches it.
- `QA.manual(reqs, reason)`: a requirement that a stub cannot check.
- `QA.need(cond, what)`: the check is blocked, not failed.
- A spec file with `-- qa: serial` in its first lines runs alone, for
  timing checks.

## REVIEW.md template

`run.lua` fills this template.

```markdown
# QA review

- Date: YYYY-MM-DD
- Commit: <sha>
- Verdict: pass | fail

## Findings

| ID | Requirement | Severity | Evidence | Suggested fix |
| --- | --- | --- | --- | --- |
| F-001 | R-003 | high | <what failed and how to reproduce> | <fix> |

## Passed

- R-001: <what was checked>

## Open questions for the spec

- <a requirement that is ambiguous or untestable, and why>
```

Severity: high (spec violated or crash), medium (spec met with a defect
that a user would notice), low (convention or quality). The verdict is
`fail` when a high or medium finding exists. `CONV` in the Requirement
column marks a convention or gate finding.
