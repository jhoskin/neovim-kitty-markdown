# kitty-markdown.nvim

A read-only markdown preview for Neovim in the kitty terminal. Text renders
in terminal cells with GitHub's light and dark palettes. Images and mermaid
diagrams render to pixels and appear inline through the kitty graphics
protocol. Mermaid renders in a Rust helper with no browser and no node at
runtime.

Screenshots: to come.

## Requirements

- Neovim 0.11 or later, with the `markdown` and `markdown_inline`
  tree-sitter parsers.
- kitty, for inline images and diagrams. The text preview works in any
  terminal.
- tmux and zmx: best effort. If the multiplexer passes the graphics
  protocol through, images work. If not, image and diagram blocks show as
  bordered boxes with alt text or the diagram source.
- The helper binary `kitty-markdown-helper`. The plugin downloads a release
  build for macOS arm64, Linux x86_64, or Linux arm64 on first use. Without
  a matching release, or for development, `cargo build --release` builds
  it locally.

## Install

With lazy.nvim:

```lua
{
  "jhoskin/neovim-kitty-markdown",
  ft = "markdown",
  build = "cargo build --release --manifest-path helper/Cargo.toml",
  opts = {},
}
```

The `build` step is a fallback. When a release binary exists for your
platform, the plugin downloads it on first use and the local build is not
needed.

## Usage

```vim
:KittyMarkdown open [split|tab|float]
:KittyMarkdown close
:KittyMarkdown toggle [split|tab|float]
:KittyMarkdown refresh
```

The preview re-renders on each save of the source buffer and on `refresh`.
It never re-renders on a keystroke. `refresh` also re-probes the terminal
and re-transmits all images, which repairs the preview after a reattach.

Keys in the preview:

| Key  | Action                                                     |
|------|------------------------------------------------------------|
| `q`  | Close the preview.                                         |
| `gx` | Open the URL, file, or image under the cursor.             |

When the preview is a split, it scrolls to keep the block under the source
cursor in view.

Lua API: `require('kitty-markdown')` exposes `setup`, `open`, `close`,
`toggle`, `refresh`, and `layout`.

## Configuration

Defaults shown. Pass overrides in `opts` or to `setup()`.

```lua
require('kitty-markdown').setup({
  placement = 'tab',          -- 'split' | 'tab' | 'float'
  theme = 'auto',             -- 'auto' | 'light' | 'dark'
  text_width = 100,           -- max text width in cells
  scroll_sync = true,         -- split preview follows the source cursor
  remote_images = {
    enabled = true,
    timeout_ms = 10000,
    max_bytes = 20 * 1024 * 1024,
  },
  cache = {
    dir = vim.fn.stdpath('cache') .. '/kitty-markdown',
    max_bytes = 500 * 1024 * 1024,
  },
  helper = {
    path = nil,               -- explicit binary path; nil means auto
    download = true,          -- fetch the matching release binary
    request_timeout_ms = 30000,
  },
  mermaid = {
    theme = 'auto',           -- 'auto' | 'default' | 'dark' | 'neutral' | 'forest' | 'base'
  },
})
```

| Option                      | Meaning                                                          |
|-----------------------------|------------------------------------------------------------------|
| `placement`                 | Where the preview opens when the command gives no placement.     |
| `theme`                     | `auto` follows `&background`. Palettes: GitHub light and dark.   |
| `text_width`                | Maximum text width in cells. The window width is the floor.      |
| `scroll_sync`               | Scroll a split preview to the block under the source cursor.     |
| `remote_images.enabled`     | Fetch `http` and `https` images on first render.                 |
| `remote_images.timeout_ms`  | Give up on a remote image after this time.                       |
| `remote_images.max_bytes`   | Refuse remote images larger than this.                           |
| `cache.dir`                 | Root of the on-disk cache.                                       |
| `cache.max_bytes`           | Garbage collection starts above this size.                       |
| `helper.path`               | Use this helper binary instead of the downloaded or built one.   |
| `helper.download`           | Allow the download of a release binary.                          |
| `helper.request_timeout_ms` | Give up on a helper request after this time.                     |
| `mermaid.theme`             | `auto` picks `default` on light and `dark` on dark backgrounds.  |

All highlight groups are named `KittyMarkdown*` and are defined with
`default`, so a highlight you set before the preview opens wins.

## Features

Markdown dialect: GFM core.

- CommonMark: headings, paragraphs, emphasis, inline code, links, lists,
  blockquotes, fenced code, thematic breaks, hard line breaks.
- GFM: tables, task lists, strikethrough, autolinks, footnotes.
- GitHub alerts: `> [!NOTE]`, `> [!TIP]`, `> [!IMPORTANT]`,
  `> [!WARNING]`, `> [!CAUTION]`.
- Fenced code with tree-sitter syntax highlighting and a language label.
- Emoji shortcodes such as `:tada:`.
- Heading anchors.
- Math: a Unicode approximation in cells. Greek letters, superscripts,
  subscripts, common operators, and simple fractions become Unicode text.
  Anything else shows the TeX source in a code style.
- YAML front matter is hidden.

Typography uses text cells only: highlights, conceal, Nerd Font icons,
bold, italic, underline, colored rules, and Unicode box drawing. Headings,
tables, and code are never rasterized.

Images:

- Local paths resolve against the markdown file's directory.
- Remote images are fetched on first render, cached on disk, and shown
  from the cache when offline.
- PNG, JPEG, GIF (first frame), WebP, BMP, and SVG.
- Images scale down to the text width, keep their aspect ratio, and never
  scale up past their native size. `<img width>` and `<img height>` win
  when smaller.

Mermaid diagrams render on save. The renderer honors `%%{init: ...}%%`
directives and `---` front matter config, and ships all five built-in
themes. Diagram types arrive in five phases:

| Phase | Diagram types                                                                                   |
|-------|-------------------------------------------------------------------------------------------------|
| 1     | flowchart, sequence, class, state                                                               |
| 2     | ER, pie, gantt, xychart, quadrant                                                               |
| 3     | gitgraph, mindmap, timeline, user journey                                                       |
| 4     | sankey, block, packet, kanban, architecture, C4, requirement, zenuml                            |
| 5     | radar, treemap, venn, ishikawa, wardley, cynefin, treeview, event modeling, use case, swimlanes |

The parity bar is pixel-near: output is compared with mermaid 12.2.0 by
SSIM. The handDrawn look, ELK layout, icon packs, and image nodes are out
of scope and render as a bordered box.

Failures never pop up. An image or diagram that cannot render shows a
bordered box with the alt text or diagram type and a one-line reason.
Details go to `:messages`.

## How it works

Neovim's tree-sitter markdown grammars parse the source buffer into a block
tree. A pure Lua layout turns the tree into lines, highlight spans, and
block descriptors for images and diagrams. The preview buffer fills with
text at once. Each image or diagram block then goes to the helper as an
asynchronous request. The helper decodes images, rasterizes SVG with resvg,
and renders mermaid through its own parser, layout, and SVG scene. Results
come back as PNG files in the cache. The plugin transmits each PNG with the
kitty graphics protocol, makes a virtual placement, and fills the block rows
with Unicode placeholder cells as virtual lines, so the image scrolls with
the text.

The full architecture is in `AGENTS.md`.

## Development process

Two Claude Code sessions work on this repo, each with its own role. Both
read `AGENTS.md` at start, which holds the full rules.

- Dev agent: owns `SPEC.md`, writes the code in `lua/`, `plugin/`, `doc/`,
  and `helper/`, the unit tests in `tests/` and `helper/tests/`, and keeps
  the architecture and conventions in `AGENTS.md`. It has no access to
  `held_out/`.
- QA agent: owns `held_out/`, which holds hidden tests and evals. On each
  run it executes them, checks the code against `SPEC.md`, and writes its
  findings to `REVIEW.md` in the repo root. It has access to every folder.

The loop: the dev agent writes or updates the spec, implements it, and runs
the unit tests. The QA agent runs `held_out/` and writes `REVIEW.md`. The
dev agent reads `REVIEW.md`, fixes what it reports, and the loop repeats.

### Launch

Run both from the repo root. The project settings file
`.claude/settings.json` denies reads and edits under `held_out/`, so a
plain `claude` session is the dev session. The QA session skips the
project settings and loads `.claude/settings.qa.json`, which allows
`held_out/` and `REVIEW.md` and denies edits on every dev-owned path.

```sh
# Dev agent: no access to held_out/
claude

# QA agent: held_out/ and REVIEW.md writable, everything else read-only
claude --setting-sources user --settings .claude/settings.qa.json \
  --append-system-prompt 'Read AGENTS.md first and follow the QA agent role.'
```

### Spec constraints

Every requirement in `SPEC.md` has a stable ID, uses one EARS pattern
(ubiquitous, event-driven, state-driven, unwanted behavior, or optional
feature), and is written in ASD-STE100 Simplified Technical English.

### Commands

```sh
just lint    # stylua, selene, rustfmt, clippy
just test    # busted under nvim -l, cargo test
just build   # cargo build --release for the helper
```

### Mermaid oracle

The QA agent generates golden images with mermaid 12.2.0 through
`@mermaid-js/mermaid-cli` 12.0.0 (`npx`, node, and a Chromium download on
the QA machine only). The helper and the oracle run on the same machine
with the same fonts.

## Layout

- `SPEC.md`: the specification. Source of truth for behavior.
- `AGENTS.md`: roles, access rules, workflow, architecture, and
  conventions.
- `lua/kitty-markdown/`: the plugin modules.
- `plugin/kitty-markdown.lua`: the command and autocommands.
- `doc/kitty-markdown.txt`: vimdoc.
- `helper/`: the Rust helper crate.
- `tests/`: Lua unit tests written by the dev agent.
- `held_out/`: hidden tests and evals, QA agent only.
- `REVIEW.md`: QA findings from the latest run. Generated and gitignored.

## License

All rights reserved. Personal project by Joh Hoskin.
