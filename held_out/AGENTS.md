# QA agent instructions

This file loads when the QA agent opens a file under `held_out/`. The dev
agent cannot read it.

## Layout

- `held_out/tests/`: hidden tests. They run against `src/`.
- `held_out/evals/`: evals, fixtures, and sample documents.

The tests use the language and toolchain recorded in the Conventions
section of the root `AGENTS.md`. When that section is empty, there is
nothing to test yet. Report that in `REVIEW.md`.

## A QA run

1. Read `SPEC.md` and the Conventions section of the root `AGENTS.md`.
2. Add or update tests under `held_out/` so every requirement ID has at
   least one check.
3. Run everything under `held_out/` and the dev agent's own `tests/`.
4. Compare the behavior with each requirement. Check that the code follows
   the Conventions section.
5. Write `REVIEW.md` in the repo root with the template below. Overwrite the
   previous file.

The QA agent does not change `src/`, `tests/`, `SPEC.md`, or the root
`AGENTS.md`. A problem in one of those is a finding.

## REVIEW.md template

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
that a user would notice), low (convention or quality).
