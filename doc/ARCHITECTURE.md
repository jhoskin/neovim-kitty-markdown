# Architecture

The product is `kitty-markdown.nvim`, a Neovim plugin that renders a
markdown buffer into a read-only preview. Text is rendered in terminal
cells. Images and mermaid diagrams are rendered to pixels by a Rust helper
and shown through the kitty graphics protocol.

## Components

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

## Data flow

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

## Terminal transport details

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

## Helper protocol

One helper process per Neovim instance, started on first need and stopped
on `VimLeavePre`. The transport is JSON lines over stdin and stdout.

- Request: `{"id": <int>, "method": "<name>", "params": {...}}`.
- Response: `{"id": <int>, "result": {...}}` or
  `{"id": <int>, "error": {"code": "<string>", "message": "<string>"}}`.
- Methods: `probe`, `image.prepare`, `svg.rasterize`, `mermaid.render`,
  `fetch`, `cache.gc`, `shutdown`. Params, results, and error codes are
  in `doc/CONVENTIONS.md` under Protocol methods and errors.
- Results that carry pixels return a path to a PNG file in the cache, not
  inline bytes.
- CLI mode for direct evaluation:
  `kitty-markdown-helper render --mermaid in.mmd --width <px> --theme
  <name> -o out.png`, with `--image` and `--svg` equivalents.

## Mermaid pipeline

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

## Parity oracle

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
