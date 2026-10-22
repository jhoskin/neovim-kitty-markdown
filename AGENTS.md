# Agent instructions

## Purpose

Two Claude Code sessions work in this repo: a dev agent and a QA agent.
Each session reads this file at start. Read the Roles section, find your
role, and follow the rules for it. The Purpose, Roles, Access rules,
Workflow, and Spec rules sections belong to Joh. The dev agent edits the
Architecture and Conventions sections only.

## Roles

Dev agent:

- Owns `SPEC.md`, `lua/`, `plugin/`, `doc/`, `helper/`, `tests/`, and the
  Architecture and Conventions sections of this file.
- Writes the spec, the implementation, and the unit tests.
- Reads `REVIEW.md` and fixes what it reports.

QA agent:

- Owns `held_out/` and `REVIEW.md`.
- Writes and runs the hidden tests and evals under `held_out/`.
- Checks the code against `SPEC.md` and the Conventions section.
- Writes `REVIEW.md`. Does not change `lua/`, `plugin/`, `doc/`,
  `helper/`, `tests/`, `SPEC.md`, or this file. A problem in one of those
  goes into `REVIEW.md`.

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
2. The dev agent implements the requirement in `lua/` or `helper/` and adds
   unit tests in `tests/` or `helper/tests/`. All unit tests pass before
   the work is done.
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
- Every requirement uses one EARS pattern:
  - Ubiquitous: "The `<system>` shall `<response>`."
  - Event-driven: "When `<trigger>`, the `<system>` shall `<response>`."
  - State-driven: "While `<state>`, the `<system>` shall `<response>`."
  - Unwanted behavior: "If `<condition>`, then the `<system>` shall
    `<response>`."
  - Optional feature: "Where `<feature>`, the `<system>` shall
    `<response>`."
- Requirement prose follows ASD-STE100 Simplified Technical English: one
  term for one thing, an instruction has at most 20 words, a descriptive
  sentence has at most 25 words, no contractions, approved vocabulary.

## Architecture

The product is `kitty-markdown.nvim`, a Neovim plugin that renders a
markdown buffer into a read-only preview. Text is rendered in terminal
cells. Images and mermaid diagrams are rendered to pixels by a Rust helper
and shown through the kitty graphics protocol.

### Components

Each component is defined once here. The rest of this file and `SPEC.md`
use these names.

- Parser: Neovim's built-in tree-sitter `markdown` and `markdown_inline`
  grammars walk the source buffer. Output: a block tree with these node
  kinds: heading, paragraph, list, table, code fence, blockquote, alert,
  image, mermaid fence, html, footnote, thematic break, front matter.
- Layout: pure Lua. Turns the block tree into the layout model: an array
  of lines with text, highlight spans, conceal ranges, and block
  descriptors for images and diagrams. A block descriptor carries the
  requested width in cells and, once the pixel size is known, the number
  of rows to reserve. The layout is deterministic and runs without a
  terminal through `require('kitty-markdown').layout(bufnr)`.
- Theme: GitHub light and dark markdown palettes, shipped as highlight
  groups named `KittyMarkdown*` and selected by `&background`.
- Preview: a scratch buffer and window placed as `split`, `tab`, or
  `float`, filled from the layout model with extmarks. Read-only. `q`
  closes it, `gx` opens the target under the cursor, and a split preview
  follows the source cursor. One preview per source buffer. Closing the
  source buffer closes its preview.
- Terminal transport: the kitty graphics protocol writer. It probes
  support, transmits images in chunks, makes virtual placements, writes
  Unicode placeholder cells as virtual lines, and deletes images by id when
  a block changes. Inside tmux it wraps each sequence in DCS passthrough.
- Helper client: Lua code that spawns `kitty-markdown-helper`, speaks JSON
  lines over stdio with request ids, applies timeouts, and restarts the
  helper after a crash. On first use it downloads the release binary that
  matches the plugin version, or falls back to a `cargo build` output.
- Helper: the Rust binary `kitty-markdown-helper`. It decodes and resizes
  images, rasterizes SVG with resvg, fetches remote images into the cache,
  and renders mermaid diagrams to PNG without a browser or node.
- Cache: files under `stdpath('cache')/kitty-markdown/` in the
  subdirectories `images`, `remote`, and `mermaid`. Keys are the SHA-256
  of the content plus the render parameters (width, theme, mermaid
  version). Garbage collection runs when the cache exceeds its size limit.

### Data flow

1. The user saves the source buffer or runs `:KittyMarkdown refresh`.
2. The parser builds the block tree.
3. The layout builds the layout model.
4. The preview fills its buffer with the text, highlights, and a loading
   box for each image or diagram block. The user sees text now.
5. For each block descriptor, the helper client sends one asynchronous
   request to the helper.
6. On each result, the terminal transport transmits the image, the layout
   reserves rows, and the preview replaces the loading box with
   placeholder cells. On an error, the preview shows a failure box.

Rendering never blocks editing in the source buffer.

### Terminal transport details

- Probe: send a kitty graphics query (`a=q`) followed by Primary Device
  Attributes. If the graphics reply arrives before the DA reply, images
  are on. Otherwise the preview runs in text-only mode and image and
  diagram blocks are bordered boxes with alt text or the diagram source.
- Transmit: PNG data, base64, chunks of 4096 bytes, `m=1` on every chunk
  but the last, `q=2` to suppress replies. Transmit once per image id.
- Place: a virtual placement `a=p,U=1,i=<id>,c=<cols>,r=<rows>`. The
  block rows are U+10EEEE cells with row and column diacritics. The image
  id travels in the foreground color, with a third diacritic for the high
  byte when the id needs it.
- Rows: placeholder rows are extmark virtual lines in the preview buffer,
  so the image scrolls with the text.
- Delete: `a=d,d=I,i=<id>` when a block changes or the preview closes.
- tmux: when `$TMUX` is set and the probe answers, every sequence is
  wrapped in `ESC P tmux; ... ESC \` with inner ESC bytes doubled.
- zmx: best effort. The probe decides. A reattach or `:KittyMarkdown
  refresh` re-probes and re-transmits.
- Cell pixel size comes from the terminal, so retina displays get the
  right pixel density.

### Helper protocol

One helper process per Neovim instance, started on first need and stopped
on `VimLeavePre`. The transport is JSON lines over stdin and stdout.

- Request: `{"id": <int>, "method": "<name>", "params": {...}}`.
- Response: `{"id": <int>, "result": {...}}` or
  `{"id": <int>, "error": {"code": "<string>", "message": "<string>"}}`.
- Methods: `probe`, `image.prepare`, `svg.rasterize`, `mermaid.render`,
  `fetch`, `cache.gc`, `shutdown`. Final names are the dev agent's.
- Results that carry pixels return a path to a PNG file in the cache, not
  inline bytes.
- CLI mode for direct evaluation:
  `kitty-markdown-helper render --mermaid in.mmd --width <px> --theme
  <name> -o out.png`, with `--image` and `--svg` equivalents.

### Mermaid pipeline

Mermaid rendering is a Rust reimplementation. No browser, no node at
runtime.

1. Lexer and parser per diagram type produce a diagram model.
2. Theme resolution applies, in order: the built-in theme, `%%{init:
   ...}%%` directives, and `---` front matter config inside the diagram.
   All five built-in themes exist: `default`, `dark`, `neutral`, `forest`,
   `base`.
3. Layout: a layered layout compatible with dagre (port of dagre-d3-es
   7.0.14 behavior) for graph diagrams; per-type layouts for sequence,
   gantt, pie, and the others.
4. SVG scene generation.
5. resvg rasterizes the scene to PNG at the target pixel size, with fonts
   from fontdb and mermaid's default family chain `"trebuchet ms", verdana,
   arial, sans-serif`.

Diagram types arrive in five phases, ordered by usage. See `SPEC.md`.

### Parity oracle

The QA agent generates golden PNGs with mermaid 12.2.0 through
`@mermaid-js/mermaid-cli` 12.0.0, run with `npx` on the QA machine. It
compares the helper output with the golden using SSIM at the same pixel
size and the same fonts. The threshold is in `SPEC.md`. The oracle and the
helper run on the same machine with the same font set. The dev agent bumps
the pin on purpose and records the new version in `SPEC.md`.

## Conventions

The dev agent owns this section. Record decisions here, not in chat.

### Language and toolchain

- Lua 5.1 (LuaJIT) for the plugin. Neovim 0.11 or later.
- Rust stable for the helper. The dev agent records the MSRV in
  `helper/Cargo.toml` under `rust-version`.
- Crates expected: `resvg`, `usvg`, `tiny-skia`, `fontdb`, `image`,
  `serde`, `serde_json`, `ureq`, `sha2`.
- Task runner: `just`. Lint: `stylua`, `selene`, `rustfmt`, `clippy`.
- Platforms: macOS arm64, Linux x86_64, Linux arm64.
- Release binaries are built by a GitHub Actions workflow on tag for
  `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, and
  `aarch64-unknown-linux-gnu`. There is no lint or test workflow.

### Commands

- `just lint`: runs `stylua --check`, `selene`, `cargo fmt --check`, and
  `cargo clippy -- -D warnings` in `helper/`.
- `just test`: runs the Lua tests with `nvim -l tests/run.lua` (busted,
  no plenary) and `cargo test` in `helper/`.
- `just build`: runs `cargo build --release --manifest-path
  helper/Cargo.toml`.
- Run all three before a commit. Do not commit if one fails.

### Layout

- `lua/kitty-markdown/`: plugin modules. `init.lua` exposes `setup`,
  `open`, `close`, `toggle`, `refresh`, and `layout`. One module per
  component: `parser.lua`, `layout.lua`, `theme.lua`, `preview.lua`,
  `transport.lua`, `helper.lua`, `cache.lua`, `config.lua`.
- `plugin/kitty-markdown.lua`: defines the `:KittyMarkdown` command and
  autocommands. No work at load time beyond that.
- `doc/kitty-markdown.txt`: vimdoc.
- `helper/`: the Cargo crate for `kitty-markdown-helper`. `helper/src/`
  holds the code, `helper/tests/` the Rust tests with `insta` snapshots
  and SSIM tests against PNGs generated by the oracle.
- `tests/`: Lua tests. `tests/run.lua` is the busted entry point. Fixture
  markdown lives in `tests/fixtures/`.
- `justfile` at the repo root.

### Code style

- Lua: `stylua` defaults with 2-space indent, `selene` with the Neovim
  standard library. No globals. Every module returns a table. Use
  `vim.api` and `vim.uv` directly; no compatibility layer.
- Rust: `rustfmt` defaults, `clippy` with warnings denied. Errors are
  typed per module and carry a stable `code` string for the protocol.
- Comments say why, not what. No commented-out code.
- Commits: Conventional Commits, `type(scope): description`, one logical
  change per commit, each commit builds and passes tests. No issue tracker
  footer; this repo has no Linear project.
- Markdown files pass `markdownlint-cli2` with MD013 off.
