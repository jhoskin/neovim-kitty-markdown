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

The project settings file `.claude/settings.json` denies Read and Edit
under `held_out/`. Every session that loads the project settings gets
that deny, so a plain `claude` session is the dev session. The deny rule
covers the file tools and the common shell file commands. This rule covers
everything else.

The QA session skips the project settings and loads
`.claude/settings.qa.json` instead:

```sh
claude --setting-sources user --settings .claude/settings.qa.json \
  --append-system-prompt 'Read AGENTS.md first and follow the QA agent role.'
```

A deny in any loaded settings file wins over every allow, so the QA
session must not load the project settings. Skipping them also skips the
automatic read of this file, which the system prompt restores. The QA
settings file denies Edit on every dev-owned path. The deny covers the
file tools and the file commands Claude Code recognizes in Bash. A script
that writes files on its own is not covered, so the QA agent does not run
one against a dev-owned path.

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
  `fetch`, `cache.gc`, `shutdown`. Params, results, and error codes are
  in the Conventions section under Protocol methods and errors.
- Results that carry pixels return a path to a PNG file in the cache, not
  inline bytes.
- CLI mode for direct evaluation:
  `kitty-markdown-helper render --mermaid in.mmd --width <px> --theme
  <name> -o out.png`, with `--image` and `--svg` equivalents.

### Mermaid pipeline

Mermaid rendering is a Rust reimplementation. No browser, no node at
runtime.

1. Lexer and parser per diagram type produce a diagram model.
2. Config resolution applies, in order: the base config (`layout`
   `dagre`, `look` `classic`, the theme the plugin selects), `%%{init:
   ...}%%` directives, and `---` front matter config inside the diagram.
   All five built-in themes exist: `default`, `dark`, `neutral`, `forest`,
   `base`. Mermaid 12 defaults (ELK, `neo`, `redux-color`) are replaced
   by the base config; a diagram that asks for them explicitly gets an
   `unsupported_feature` error.
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
`@mermaid-js/mermaid-cli` 12.0.0. The pinned install lives in
`helper/oracle/` (`package.json` with an npm override of `mermaid` to
12.2.0, a committed lockfile, `mermaid-config.json`, `puppeteer.json`);
`npx -p @mermaid-js/mermaid-cli@12.0.0` alone would float to the newest
mermaid 12.x. Every oracle run passes `-c helper/oracle/mermaid-config.json`
and `--size <longer side of the helper output>`; the dev agent uses the
same install through `just goldens` for `helper/tests/goldens/`. The
comparison is SSIM as defined in `SPEC.md` at the same pixel size and the
same fonts; the helper exposes the same formula as
`kitty-markdown-helper compare`. Mindmap and cynefin use the best of five
goldens. The oracle and the helper run on the same machine with the same
font set. A pin bump is a spec change and goes through Joh.

## Conventions

The dev agent owns this section. Record decisions here, not in chat.

### Language and toolchain

- Lua 5.1 (LuaJIT) for the plugin. Neovim 0.12 or later.
- Rust stable for the helper. The dev agent records the MSRV in
  `helper/Cargo.toml` under `rust-version`.
- Crates: `clap` 4 (derive), `serde`, `serde_json`, `thiserror` 2,
  `sha2` 0.10, `image` 0.25 (png, jpeg, gif, webp, bmp only), `resvg`
  and `usvg` 0.48 (they re-export `tiny-skia` and `fontdb`; do not pin
  those separately), `rustybuzz` 0.20 for text shaping, `ureq` 3 with
  rustls, `regex`, `indexmap` (everywhere JS insertion order matters),
  `serde_yaml_ng`, `jiff` (gantt dates), `log`. Dev: `insta`,
  `pretty_assertions`. Cargo features `phase2` to `phase5` gate the tests
  of later mermaid phases.
- Bundled assets: `helper/assets/fonts/OpenSans-Regular.ttf` and
  `OpenSans-Italic.ttf` (OFL), loaded with `include_bytes!` before the
  system fonts. `helper/assets/fonts/README.md` records the upstream
  version.
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
- `just goldens`: runs `npm ci --prefix helper/oracle`, then renders
  every `helper/tests/goldens/<type>/<case>.mmd` with `mmdc` and the
  pinned config at the size the helper reports.
- `just jison-tables`: regenerates `helper/grammars/*.tables.json` from
  the vendored `.jison` files with `jison@0.4.18`.
- Run lint, test, and build before a commit. Do not commit if one fails.

### Layout

- `lua/kitty-markdown/`: plugin modules. `init.lua` exposes `setup`,
  `open`, `close`, `toggle`, `refresh`, and `layout`. One module per
  component: `parser.lua`, `layout.lua`, `theme.lua`, `preview.lua`,
  `transport.lua`, `helper.lua`, `cache.lua`, `config.lua`. Support
  modules: `version.lua` (the release tag without `v`), `icons.lua`
  (Nerd Font glyphs), `emoji.lua` (GitHub shortcode table), `math.lua`
  (TeX to Unicode), `download.lua` (release binary fetch).
- `plugin/kitty-markdown.lua`: defines the `:KittyMarkdown` command and
  autocommands. No work at load time beyond that.
- `doc/kitty-markdown.txt`: vimdoc.
- `helper/`: the Cargo crate for `kitty-markdown-helper`. `helper/src/`
  holds the code, `helper/tests/` the Rust tests with `insta` snapshots
  and SSIM tests against PNGs generated by the oracle. `helper/oracle/`
  holds the pinned mermaid-cli install, `helper/grammars/` the vendored
  jison grammars and generated tables, `helper/assets/` the bundled
  fonts.
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

### Lua module contracts

Signatures the packages agree on. A change here is a change to every
caller, so make it in one commit.

- `config`: `defaults` (the README table verbatim), `setup(user)`,
  `get()`.
- `theme`: `palette(name)`, `apply(name)` (defines `KittyMarkdown*` with
  `nvim_set_hl` and `default = true`, so user groups win), `current()`,
  `mermaid_theme()`.
- `parser`: `parse(bufnr)` returns a block list. Block kinds: `heading`
  (level, inlines, slug), `paragraph`, `list` (ordered, start, items with
  checked, inlines, children), `table` (align, header, rows), `code`
  (lang, lines), `mermaid` (source), `blockquote`, `alert` (type),
  `image` (src, alt, attrs), `html`, `footnote_def`, `rule`,
  `front_matter`. Inline types: `text`, `emph`, `strong`, `strike`,
  `code`, `link`, `autolink`, `image`, `footnote_ref`, `math`, `html`,
  `emoji`, `linebreak`. An inline image is lifted into an `image` block
  after its paragraph.
- `layout`: `build(blocks, opts)` (pure) and `slug(text)` (GitHub
  slug). `init.layout(bufnr, opts)` is `build(parser.parse(bufnr), opts)`.
- `math`: `render(tex)` returns a string, or nil when the span cannot be
  approximated.
- `emoji`: `lookup(shortcode)`.
- `transport`: `probe(cb)`, `cell_size()`, `alloc_id()`,
  `transmit(id, png_path)`, `place(id, cols, rows)`,
  `placeholder_rows(id, cols, rows)`, `hl_group(id)`, `delete(id)`,
  `wrap_tmux(seq)`, `set_writer(fn)`, `set_winsize_reader(fn)` (the last
  two are test hooks).
- `helper`: `request(method, params, cb)` with `cb(err, result)`,
  `ensure(cb)`, `shutdown(timeout_ms)`, `is_running()`, `state()`.
- `download`: `target()`, `url(version, target)`, `install_dir()`,
  `fetch(version, cb)`.
- `cache`: `root()`, `dirs()`, `key_image(...)`, `key_remote(url)`,
  `key_mermaid(...)`, `note_bytes(n)`, `keep_set()`, `gc_if_needed()`.
- `preview`: `open(src_buf, placement)`, `close(src_buf)`,
  `toggle(src_buf, placement)`, `get(src_buf)`, `render(src_buf, opts)`,
  `render_all()`, `in_use_paths()`, `retheme()`.

### Layout model

`layout(bufnr, opts)` returns plain tables:

- `width`, `text_width` (`min(width - 4, config.text_width)`), `theme`.
- `lines`: preview lines with a 2-cell left margin of spaces, no
  markdown markers.
- `highlights`: `{ line, col_start, col_end, group }`, line 1-based,
  byte columns 0-based, end exclusive.
- `conceals`: same shape; empty while the layout omits markers.
- `blocks`: `{ kind, index, line, src_line, source, alt, key,
  width_cells, attrs, width_px, height_px, rows, image_id, text_only }`.
  `line` is the anchor line. `key` is the block key (SPEC.md
  Definitions) computed with `vim.fn.sha256`. Pixel fields come from
  `opts.sizes[key]` and `opts.cell`; `rows = ceil(height_px /
  cell.height_px)`.
- `code_regions`: `{ line_start, line_end, lang }`; the preview runs
  tree-sitter on these, the layout never does.
- `links`: `{ line, col_start, col_end, kind, target }`, kind `url`,
  `file`, `anchor`, or `image`.
- `anchors`: slug to preview line. `source_lines`: preview line to source
  line, non-decreasing. `footnotes`: `{ label, number }`.

`opts`: `width` (default `config.text_width + 4`), `cell`, `sizes`,
`theme`, `images` (false marks every block `text_only`), `dir` (base for
relative paths; tests pass a fixture dir). The layout reads no clock, no
random source, and no environment.

### Highlight groups

`KittyMarkdownH1` to `H6`, `Text`, `Emph`, `Strong`, `Strike`, `Code`,
`CodeBg`, `CodeLang`, `CodeBorder`, `Link`, `Url`, `Bullet`, `Number`,
`Checked`, `Unchecked`, `TableBorder`, `TableHeader`, `Quote`,
`QuoteBar`, `AlertNote`, `AlertTip`, `AlertImportant`, `AlertWarning`,
`AlertCaution`, `Rule`, `Muted`, `Math`, `BoxBorder`, `BoxText`,
`Loading`, `Failure`, `Footnote`, `Html`, and `KittyMarkdownImg<id>` per
placed image (`fg = id % 2^24`).

### Terminal facts

- Writer: `vim.api.nvim_ui_send`. In tmux, `transport.wrap_tmux` wraps
  graphics sequences only; the DA1 request is sent bare.
- Probe: APC reply matching `^\27_G.*i=31` means images on; a DA1 reply
  with no APC means off; the 500 ms timer means off. Callbacks run
  through `vim.schedule`.
- Cell size: LuaJIT FFI `ioctl(1, TIOCGWINSZ)`, request `0x5413` on
  Linux and `0x40087468` on macOS; `width_px = xpixel / col`,
  `height_px = ypixel / row`; cached, cleared on `VimResized`. XTWINOPS
  replies never reach Lua, so there is no escape-sequence route.
- Placeholder cell: U+10EEEE, then the row diacritic, then the column
  diacritic, then the high-byte diacritic when `id >= 2^24`. The 297
  diacritics come from kitty's `gen/rowcolumn-diacritics.txt`
  (`tests/fixtures/diacritics.txt`); the first are U+0305, U+030D,
  U+030E, U+0310, U+0312, U+033D. Rows and columns above 297 are clamped
  with the reason `too tall`.
- Sequences: transmit `a=T,f=100,t=d,i=<id>,q=2,m=1;<4096 base64 bytes>`
  with `m=0` on the last chunk; place `a=p,U=1,i=<id>,c=<cols>,r=<rows>,q=2`;
  delete `a=d,d=I,i=<id>,q=2`.

### Protocol methods and errors

JSON lines over stdio. Request `{"id", "method", "params"}`; response
`{"id", "result"}` or `{"id", "error": {"code", "message", "data"}}`.

| Method | Params | Result |
| --- | --- | --- |
| `probe` | `plugin_version`, `cache_dir` | `helper_version`, `mermaid_version`, `cache_bytes`, `fonts`, `diagram_types` |
| `image.prepare` | `path`, `width_px`, `max_width_px?`, `max_height_px?` | `path`, `width_px`, `height_px`, `bytes`, `cached` |
| `svg.rasterize` | `path`, `width_px` | as `image.prepare` |
| `fetch` | `url`, `timeout_ms`, `max_bytes` | `path`, `bytes`, `cached` |
| `mermaid.render` | `source`, `width_px`, `theme`, `background` | `path`, `width_px`, `height_px`, `bytes`, `cached`, `diagram_type` |
| `cache.gc` | `max_bytes`, `keep` (paths) | `deleted_bytes`, `cache_bytes` |
| `shutdown` | none | empty, then exit 0 |

Error codes and the failure box reason the preview shows:

| Code | `data` | Reason |
| --- | --- | --- |
| `timeout` | | `timeout` |
| `too_large` | `bytes`, `max_bytes` | `too large` |
| `unsupported_format` | `format` | `unsupported format` |
| `parse_error` | `line`, `column`, `diagram_type` | `Parse error on line <line>` |
| `unsupported_feature` | `feature` | `unsupported: <feature>` |
| `unsupported_diagram_type` | `diagram_type` | `unsupported diagram type` |
| `fetch_failed` | `status` | `fetch failed` |
| `io` | `path` | `cannot read file` |
| `internal`, `invalid_params`, `unknown_method` | | `helper error` |

Plugin-side codes: `remote_images_off` (`remote images off`),
`helper_exit` (`helper exited`), `no_helper` (text-only box). The
`message` of a `parse_error` is the full oracle text. `:messages` gets
`kitty-markdown: <kind> <alt>: <code>: <message>`.

### Cache keys

- `images/<K>.png`, `K = sha256(sha256(bytes) | width_px | max_h | max_w)`
  with `0` for an absent limit.
- `remote/<sha256(url)>`, raw bytes, no extension; decoders sniff.
- `mermaid/<K>.png`, `K = sha256(sha256(source) | width_px | theme |
  mermaid_version)`.
- LRU by mtime; the helper touches an entry on every hit and write.
  `cache.gc` deletes oldest first, skipping `keep`, until under
  `max_bytes`. The plugin sends one `cache.gc` per render batch when its
  estimate (`cache_bytes` from `probe` plus `bytes` of each uncached
  result) exceeds `cache.max_bytes`.

### Helper client lifecycle

- `vim.uv.spawn` with `serve` as the argument; stdout split on newlines,
  `vim.json.decode` per line; stderr kept in a 50-line ring buffer.
- Locate order: `helper.path` option, install dir, download (when
  `helper.download`), `<plugin root>/helper/target/release/`, none.
- One pending-request table keyed by id with a `vim.uv` timer each.
- Version check after `probe`; a downloaded binary with the wrong version
  is replaced once; a user path or local build gets one WARN.
- Exit without `shutdown`: fail pending requests with `helper_exit`,
  WARN notice, restart on next request; three exits in 60 s stop
  restarts until `:KittyMarkdown refresh`.

### Rust module tree

```text
helper/src/
  main.rs cli.rs protocol.rs error.rs ssim.rs
  server/        JSON-lines loop: reader thread, worker pool, shared writer
  cache/         keys, LRU gc with keep set
  image/ svg/ fetch/
  text/          fonts.rs (fontdb + bundled Open Sans), measure.rs (SVG
                 getBBox mode and HTML 1.5em mode via rustybuzz), wrap.rs,
                 markdown.rs
  mermaid/
    mod.rs preprocess.rs directive.rs detect.rs raster.rs edges.rs
    config/      defaults from config.schema.yaml, merge, appearance
    theme/       khroma port, base, default, dark, forest, neutral
    styles/      getStyles ports resolved to attributes (no CSS in SVG)
    scene/       scene model, deterministic SVG serializer, curves, markers
    jison/       LALR driver over vendored tables, lexer, error formatter
    layout/      dagre/ (dagre-d3-es 7.0.14 port), unified.rs, d3/,
                 cose_bilkent/, fcose/, swimlane/
    shapes/      one module per mermaid shape, intersect/
    diagrams/    one module per diagram type with parse and render
```

Rules: `IndexMap` wherever mermaid relies on JS object order; port
mermaid's theme bugs as they are; read mermaid 12.2.0 source for
defaults, not the docs; jison error text comes from the real LALR tables.

### Tests and parity

- `tests/run.lua`: busted subset (`describe`, `it`, `before_each`,
  `after_each`, `pending`, `assert.are.same`, `assert.are.equal`,
  `assert.is_true`, `assert.is_false`, `assert.is_nil`,
  `assert.is_not_nil`, `assert.has_error`, `assert.matches`). It finds
  `tests/**/*_spec.lua`, takes a substring filter as `arg[1]`, and exits
  1 on any failure.
- `tests/fixtures/fake_helper.lua` is a JSON-lines helper run with
  `nvim -l` for the client tests, with scripted delays, errors, and a
  crash mode.
- `helper/tests/text_metrics.rs` calibrates the text measurer against
  `helper/oracle/measure.mjs` output at 0.5 px; it is the first parity
  gate. `dagre_parity.rs` matches node dumps from dagre-d3-es to 1e-6.
  `parity_<type>.rs` holds insta SVG snapshots and SSIM tests per type;
  later phases carry `#[cfg_attr(not(feature = "phaseN"), ignore)]`.
- A golden runner skips with a clear message when goldens are absent.

### Subagents

- Coder agents run in git worktrees on branch `wp/<package>`, own a
  disjoint file list, and commit with `--no-gpg-sign`. The main session
  rebases with `git rebase --exec 'git commit --amend --no-edit -S' main`
  and runs the gates before merging.
- `SPEC.md`, `README.md`, and `AGENTS.md` are locked by the project
  settings. A spec gap goes into the agent's final report as proposed
  wording for Joh.
- No agent reads `held_out/` by any route.
