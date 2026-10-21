# Agent instructions

## Purpose

Two Claude Code sessions work in this repo: a dev agent and a QA agent.
Each session reads this file at start. Read the Roles section, find your
role, and follow the rules for it. Sections 1 to 5 belong to Joh. The dev
agent edits the Conventions section only.

## Roles

Dev agent:

- Owns `SPEC.md`, `src/`, `tests/`, and the Conventions section of this
  file.
- Writes the spec, the implementation, and the unit tests.
- Reads `REVIEW.md` and fixes what it reports.

QA agent:

- Owns `held_out/` and `REVIEW.md`.
- Writes and runs the hidden tests and evals under `held_out/`.
- Checks the code against `SPEC.md` and the Conventions section.
- Writes `REVIEW.md`. Does not change `src/`, `tests/`, `SPEC.md`, or this
  file. A problem in one of those goes into `REVIEW.md`.

## Access rules

The dev agent must not read `held_out/` by any route:

- Not with the Read, Edit, Write, Glob, or Grep tools.
- Not with shell commands such as `cat`, `ls`, `find`, or `grep`.
- Not through git: no `git show`, `git log -p`, or `git diff` on paths under
  `held_out/`, and no `git grep`.
- Not through a subagent, a script, or a test runner pointed at it.

The dev session starts with `.claude/settings.dev.json`, which denies Read
and Edit under `held_out/`. The deny rule covers the file tools and the
common shell file commands. This rule covers everything else.

The QA agent reads every folder.

## Workflow

1. The dev agent writes or updates `SPEC.md`. Every requirement has a stable
   ID in the form `R-001`, one testable statement each. IDs are never
   reused or renumbered.
2. The dev agent implements the requirement in `src/` and adds unit tests
   in `tests/`. All unit tests pass before the work is done.
3. The QA agent runs everything under `held_out/`, compares the behavior
   with each requirement in `SPEC.md`, and writes `REVIEW.md`.
4. The dev agent reads `REVIEW.md` and fixes each finding, starting with
   the highest severity. A finding that is wrong gets a reply in the
   spec or the code comments, not a silent skip.
5. Repeat.

`REVIEW.md` is gitignored. The QA agent overwrites it on each run. The dev
agent does not delete or edit it.

## Spec rules

- `SPEC.md` is the source of truth. Code and tests follow it.
- When behavior changes, change the spec first, then the code.
- A requirement states what the user sees, not how the code does it.
- `REVIEW.md` cites requirements by ID.

## Conventions

The dev agent owns this section and fills it when it picks the stack.
Record decisions here, not in chat.

### Language and toolchain

Not chosen yet.

### Commands

Build, lint, and test commands. Not defined yet.

### Layout

How `src/` and `tests/` are organized. Not defined yet.

### Code style

Not defined yet.
