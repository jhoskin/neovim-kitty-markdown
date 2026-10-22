# Specification

## Status

Draft 1, written 2026-10-21. No code exists yet. The requirements below
cover the full product. Delivery follows the mermaid phases in the
Constraints section; a requirement for a later phase is not a defect until
that phase starts.

Defaults that the plan did not fix and that this draft sets:

- Front matter is hidden, not shown raw.
- Remote image size cap: 20 MB.
- Cache size limit: 500 MB.
- Helper request timeout: 30 s. Probe timeout: 500 ms.
- Scroll sync debounce: 100 ms.
- Maximum text width: 100 cells.
- An image inside a paragraph is placed on its own rows after that
  paragraph.

## Goals

- Show a markdown buffer as a read-only preview in Neovim, with typography
  at or above the quality of GitHub's markdown renderer, in text cells.
- Show local and remote images inline through the kitty graphics protocol.
- Render mermaid diagrams on save, with no browser and no node at runtime,
  at pixel-near parity with mermaid 12.2.0.
- Keep editing responsive: text first, pixels later, no blocking.

## Non-goals

- Decoration of the source buffer. The source buffer is never changed.
- Render on every keystroke.
- Rasterized headings, tables, or code.
- Front matter rendering. Front matter is hidden.
- Obsidian extras (wikilinks, callouts, embeds, dataview).
- An inline HTML subset beyond `<img>`.
- Full TeX math rendering. Math is a Unicode approximation with a source
  fallback.
- Enter-to-follow-links and jump-to-source.
- Animated GIF playback.
- Mermaid handDrawn look (rough.js), ELK layout, icon packs, and image
  nodes. These render as a failure box.
- A GitHub Actions lint or test workflow. Only a release workflow exists.
- Terminals other than kitty. tmux and zmx are best effort.
- A license file. Copyright Joh Hoskin, personal use.

## Definitions

- Plugin: `kitty-markdown.nvim`, the Lua code loaded by Neovim.
- Helper: `kitty-markdown-helper`, the Rust binary that the plugin runs.
- Source buffer: the markdown buffer that the user edits.
- Preview: the read-only buffer and window that show the rendered source
  buffer.
- Block: one top-level unit of the document: a heading, a paragraph, a
  list, a table, a code fence, a blockquote, an alert, an image, a
  mermaid fence, an HTML block, a footnote definition, a thematic break,
  or front matter.
- Pixel block: an image block or a mermaid block. A pixel block is shown
  as pixels when the terminal supports it.
- Layout model: the output of `layout(bufnr)`: lines, highlight spans,
  conceal ranges, and block descriptors.
- Block descriptor: the entry in the layout model for one pixel block. It
  carries the kind, the source, the requested width in cells, and the
  pixel size and reserved rows when known.
- Text width: the number of cells available for text on one preview line.
  It is the preview window width minus a margin of 2 cells on each side,
  capped by the configured maximum.
- Cell size: the width and height of one terminal cell in pixels, as
  reported by the terminal.
- Target width: the text width multiplied by the cell width, in pixels.
- Loading box: a bordered box, as tall as 3 rows, that holds the place of
  a pixel block while it renders. It shows the alt text or the diagram
  type and the word "loading".
- Failure box: a bordered box that replaces a pixel block that did not
  render. It shows the alt text or the diagram type and a one-line reason
  in the muted color.
- Placeholder cells: the U+10EEEE cells with diacritics that the kitty
  graphics protocol maps to a virtual image placement.
- Text-only mode: the preview state in which the terminal did not answer
  the graphics probe. Pixel blocks show as bordered boxes with alt text or
  diagram source.
- Oracle: mermaid 12.2.0 run through `@mermaid-js/mermaid-cli` 12.0.0,
  which produces the golden images for parity checks.
- SSIM: the structural similarity index between two images of the same
  pixel size, in the range 0 to 1.
- Cache: the directory tree under the configured cache root, with the
  subdirectories `images`, `remote`, and `mermaid`.
- System opener: `vim.ui.open`, which hands a path or URL to the operating
  system.

## Constraints

- Each requirement has a stable ID `R-NNN`, one testable statement, one
  EARS pattern, and ASD-STE100 prose. IDs are never reused or renumbered.
- Neovim 0.11 or later. Platforms: macOS arm64, Linux x86_64, Linux arm64.
- kitty graphics protocol as documented by kitty, with Unicode
  placeholders (kitty 0.28 or later).
- Mermaid pin: mermaid 12.2.0 through `@mermaid-js/mermaid-cli` 12.0.0.
  The dev agent bumps the pin on purpose and records the new version here.
- The oracle and the helper run on the same machine with the same font
  set.
- Mermaid diagram types arrive in these phases:

| Phase | Diagram types                                                                                   |
|-------|-------------------------------------------------------------------------------------------------|
| 1     | flowchart, sequence, class, state                                                               |
| 2     | ER, pie, gantt, xychart, quadrant                                                               |
| 3     | gitgraph, mindmap, timeline, user journey                                                       |
| 4     | sankey, block, packet, kanban, architecture, C4, requirement, zenuml                            |
| 5     | radar, treemap, venn, ishikawa, wardley, cynefin, treeview, event modeling, use case, swimlanes |

## Requirements

### Commands and placement

- R-001: The plugin shall provide the user command `:KittyMarkdown` with
  the subcommands `open`, `close`, `toggle`, and `refresh`.
- R-002: When the user runs `:KittyMarkdown open` in a markdown buffer, the
  plugin shall show a preview of that buffer.
- R-003: The plugin shall accept one optional placement argument `split`,
  `tab`, or `float` after `open` and after `toggle`.
- R-004: When the command gives no placement argument, the plugin shall
  use the configured placement, whose default is `tab`.
- R-005: The plugin shall not change the text, the highlights, or the
  extmarks of the source buffer.
- R-006: The plugin shall keep at most one preview per source buffer.
- R-007: When the user runs `:KittyMarkdown open` and a preview of the
  source buffer exists, the plugin shall focus that preview.
- R-008: When the user runs `:KittyMarkdown close`, the plugin shall close
  the preview of the current source buffer and delete its images.
- R-009: When the user runs `:KittyMarkdown toggle` and a preview exists,
  the plugin shall close it.
- R-010: When the user runs `:KittyMarkdown toggle` and no preview exists,
  the plugin shall open one.
- R-011: When the source buffer is unloaded or deleted, the plugin shall
  close its preview.
- R-012: If the user runs `:KittyMarkdown open` in a buffer whose filetype
  is not `markdown`, then the plugin shall show an error message and open
  no preview.
- R-013: The plugin shall expose the Lua module `kitty-markdown` with the
  functions `setup`, `open`, `close`, `toggle`, `refresh`, and `layout`.
- R-014: The preview buffer shall have the filetype `kitty-markdown` and
  shall not be modifiable.
- R-015: The preview window shall hide line numbers, the sign column, and
  the fold column, and shall not wrap lines.

### Refresh and performance

- R-016: When the source buffer is written to disk, the plugin shall
  re-render its preview.
- R-017: When the user runs `:KittyMarkdown refresh`, the plugin shall
  re-render the preview, re-probe the terminal, and re-transmit all
  images.
- R-018: The plugin shall not re-render the preview on a text change that
  is not a write.
- R-019: When the plugin renders a preview, the preview shall show the
  document text before any pixel block is ready.
- R-020: While a pixel block renders, the preview shall show a loading
  box in its place.
- R-021: When a pixel block result arrives, the preview shall replace the
  loading box with the image and keep the rest of the text unchanged.
- R-022: The plugin shall not block input in the source buffer while a
  render runs.
- R-023: When the user opens a preview of a document with 1000 lines, the
  preview shall show the text within 200 ms.
- R-024: When the helper process is already running, the helper shall
  return a mermaid render result within 100 ms.
- R-025: When the plugin starts the helper, the helper shall accept its
  first request within 500 ms.
- R-026: When a re-render changes no pixel block, the plugin shall keep
  the existing images and shall not re-transmit them.

### Text rendering

- R-027: The preview shall show each heading in the highlight group of its
  level, with a Nerd Font icon before the text.
- R-028: The preview shall conceal the `#` markers of ATX headings and the
  underline of setext headings.
- R-029: The preview shall wrap paragraph text at the text width, at word
  boundaries.
- R-030: The preview shall not split a word that is shorter than the text
  width.
- R-031: The preview shall show a hard line break (two trailing spaces or
  a backslash) as a line break.
- R-032: The preview shall show emphasis text in italic and conceal the
  markers.
- R-033: The preview shall show strong text in bold and conceal the
  markers.
- R-034: The preview shall show strikethrough text with strikethrough and
  conceal the markers.
- R-035: The preview shall show inline code in the code highlight group
  and conceal the backticks.
- R-036: The preview shall show link text underlined in the link color and
  conceal the brackets and the URL.
- R-037: The preview shall show a bare URL and an autolink in angle
  brackets underlined in the link color.
- R-038: The preview shall show a reference-style link the same way as an
  inline link and hide its definition line.
- R-039: The preview shall show each unordered list item with a bullet
  icon in place of the marker.
- R-040: The preview shall show each ordered list item with its number
  followed by a period.
- R-041: The preview shall indent each nested list level by 2 cells.
- R-042: The preview shall show a task list item with a checked or an
  unchecked checkbox icon in place of `[x]` or `[ ]`.
- R-043: The preview shall show a table with Unicode box drawing borders
  around every cell.
- R-044: The preview shall align each table column as its delimiter row
  states: left, center, or right.
- R-045: The preview shall size each table column by the display width of
  its widest cell, including wide characters.
- R-046: The preview shall show the header row of a table in bold.
- R-047: The preview shall show a fenced code block with tree-sitter
  highlighting for the language in its info string.
- R-048: If no tree-sitter parser is available for the language, then the
  preview shall show the code block in the plain code highlight group.
- R-049: The preview shall show the language label of a fenced code block
  on its top border.
- R-050: The preview shall conceal the fence markers of a code block and
  fill its rows with the code background color.
- R-051: The preview shall show a blockquote with a vertical bar in the
  left margin and the quote highlight group.
- R-052: The preview shall show a GitHub alert (`NOTE`, `TIP`, `IMPORTANT`,
  `WARNING`, `CAUTION`) with the icon, title, and color of its kind.
- R-053: The preview shall show a footnote reference as a superscript
  number.
- R-054: The preview shall show footnote definitions at the end of the
  document, numbered in order of first reference.
- R-055: The preview shall show a thematic break as a horizontal rule
  across the text width in the muted color.
- R-056: The preview shall replace an emoji shortcode such as `:tada:`
  with its emoji.
- R-057: If an emoji shortcode is unknown, then the preview shall show the
  shortcode as written.
- R-058: The preview shall show a backslash escape as the escaped
  character.
- R-059: When the document starts with YAML front matter, the preview
  shall hide it.
- R-060: The preview shall show HTML blocks and inline HTML, other than
  `<img>` tags, as plain text in the muted color.
- R-061: The preview shall keep one blank line between blocks and no blank
  line at the top of the buffer.

### Math

- R-062: The preview shall treat `$...$` as inline math and `$$...$$` as
  display math.
- R-063: When a math span contains only Greek letters, superscripts,
  subscripts, common operators, and simple fractions, the preview shall
  show it as Unicode text.
- R-064: The preview shall map each Greek letter command, such as `\alpha`
  and `\Omega`, to its Unicode letter.
- R-065: The preview shall map a superscript of digits, signs, or Latin
  letters with Unicode superscript forms to superscript characters.
- R-066: The preview shall map a subscript of digits, signs, or Latin
  letters with Unicode subscript forms to subscript characters.
- R-067: The preview shall map the operators `\times`, `\cdot`, `\pm`,
  `\leq`, `\geq`, `\neq`, `\approx`, `\infty`, `\sum`, `\int`, `\sqrt`,
  and `\to` to their Unicode symbols.
- R-068: The preview shall show `\frac{a}{b}` as `a/b`, with parentheses
  around a numerator or denominator that has more than one token.
- R-069: If a math span contains TeX that the plugin cannot approximate,
  then the preview shall show the source in the math highlight group.
- R-070: The preview shall show display math on its own line, centered in
  the text width.

### Theme

- R-071: The plugin shall define highlight groups named `KittyMarkdown*`
  with the GitHub light and GitHub dark markdown palettes.
- R-072: While `&background` is `dark`, the preview shall use the dark
  palette.
- R-073: While `&background` is `light`, the preview shall use the light
  palette.
- R-074: Where the theme option is `light` or `dark`, the preview shall
  use that palette regardless of `&background`.
- R-075: When `&background` changes, the plugin shall re-theme every open
  preview.
- R-076: Where the user defines a `KittyMarkdown*` highlight group before
  the preview opens, the plugin shall keep the user definition.

### Images

- R-077: When an image path is relative, the plugin shall resolve it
  against the directory of the markdown file.
- R-078: When an image path is absolute, the plugin shall use it as
  written.
- R-079: Where remote images are enabled, the plugin shall fetch each
  `http` or `https` image on first render.
- R-080: The plugin shall store each fetched image in the cache under a
  hash of its URL.
- R-081: When a remote image is in the cache, the plugin shall use the
  cached file and shall not fetch it again.
- R-082: If a remote fetch exceeds the configured timeout, then the
  preview shall show a failure box with the reason "timeout".
- R-083: If a remote image exceeds the configured size cap, then the
  preview shall show a failure box with the reason "too large".
- R-084: If a remote fetch fails and the cache has no copy, then the
  preview shall show a failure box with the alt text and the reason.
- R-085: Where remote images are disabled, the preview shall show each
  remote image as a failure box with the reason "remote images off".
- R-086: The helper shall decode PNG, JPEG, GIF, WebP, and BMP images and
  shall encode the result as PNG.
- R-087: When an image is a GIF with more than one frame, the helper shall
  use the first frame.
- R-088: When an image is SVG, the helper shall rasterize it at the target
  width.
- R-089: If an image has a format the helper does not decode, then the
  preview shall show a failure box with the reason "unsupported format".
- R-090: The plugin shall scale an image wider than the target width down
  to the target width and keep its aspect ratio.
- R-091: The plugin shall not scale an image up past its native pixel
  size.
- R-092: When an `<img>` tag gives a width or a height smaller than the
  computed size, the plugin shall use the smaller value.
- R-093: The plugin shall reserve for an image the number of rows equal to
  its height in pixels divided by the cell height, rounded up.
- R-094: The plugin shall obtain the cell size from the terminal and shall
  not assume a fixed cell size.
- R-095: The preview shall place an image that is inside a paragraph on
  its own rows after that paragraph.
- R-096: When a pixel block fails, the plugin shall write the full error to
  `:messages` and shall not open a popup.

### Mermaid diagrams

- R-097: The plugin shall treat a fenced code block with the info string
  `mermaid` as a mermaid block.
- R-098: When the plugin renders a mermaid block, the helper shall render
  the diagram to PNG at the target width.
- R-099: While the preview uses the dark palette, the helper shall render
  mermaid with the `dark` theme unless the diagram sets a theme.
- R-100: While the preview uses the light palette, the helper shall render
  mermaid with the `default` theme unless the diagram sets a theme.
- R-101: Where the mermaid theme option names a theme, the helper shall
  use that theme unless the diagram sets a theme.
- R-102: The helper shall apply `%%{init: ...}%%` directives in the
  diagram, including `theme`, `themeVariables`, `fontFamily`, `look`,
  `layout`, and diagram-specific settings.
- R-103: The helper shall apply `---` front matter config in the diagram
  with the same keys as the init directive.
- R-104: The helper shall render all five built-in themes: `default`,
  `dark`, `neutral`, `forest`, and `base`.
- R-105: The helper shall render flowchart, sequence, class, and state
  diagrams (phase 1).
- R-106: The helper shall render ER, pie, gantt, xychart, and quadrant
  diagrams (phase 2).
- R-107: The helper shall render gitgraph, mindmap, timeline, and user
  journey diagrams (phase 3).
- R-108: The helper shall render sankey, block, packet, kanban,
  architecture, C4, requirement, and zenuml diagrams (phase 4).
- R-109: The helper shall render radar, treemap, venn, ishikawa, wardley,
  cynefin, treeview, event modeling, use case, and swimlane diagrams
  (phase 5).
- R-110: The helper output for each diagram type in a delivered phase
  shall reach an SSIM of 0.95 or more against the oracle at the same pixel
  size and fonts.
- R-111: If a mermaid diagram does not parse, then the preview shall show
  a failure box with the diagram type and the line and column of the
  error, in the form the oracle uses.
- R-112: If a diagram uses the handDrawn look, ELK layout, an icon pack,
  or an image node, then the preview shall show a failure box with the
  reason "unsupported: `<feature>`".
- R-113: If a diagram type is not in a delivered phase, then the preview
  shall show a failure box with the reason "unsupported diagram type".
- R-114: When a diagram sets its own width smaller than the target width,
  the helper shall render at the diagram width.
- R-115: The helper shall resolve the mermaid font family chain
  `"trebuchet ms", verdana, arial, sans-serif` against the system fonts, in
  that order.
- R-116: The plugin shall cache each mermaid render under a hash of the
  diagram source, the width, the theme, and the mermaid version.

### Terminal transport

- R-117: When the first preview opens, the plugin shall send a kitty
  graphics query followed by Primary Device Attributes and wait for the
  answers.
- R-118: When the graphics query answers before the probe timeout, the
  plugin shall show pixel blocks as images.
- R-119: If the graphics query does not answer before the probe timeout,
  then the preview shall run in text-only mode.
- R-120: While in text-only mode, the preview shall show each image as a
  bordered box with its alt text.
- R-121: While in text-only mode, the preview shall show each mermaid
  block as a bordered box with the diagram source.
- R-122: While `$TMUX` is set, the plugin shall wrap each graphics
  sequence in a tmux DCS passthrough with the inner escape bytes doubled.
- R-123: The plugin shall transmit each image once, as base64 in chunks of
  at most 4096 bytes, with `m=1` on every chunk but the last and `q=2`.
- R-124: The plugin shall place each image as a virtual placement with
  `U=1` and the block's column and row counts.
- R-125: The plugin shall fill the block rows with placeholder cells that
  carry the row and column diacritics and the image id in the foreground
  color.
- R-126: When an image id exceeds 24 bits of foreground color, the
  placeholder cells shall carry the high byte in a third diacritic.
- R-127: The plugin shall insert placeholder rows as extmark virtual lines,
  so that the image scrolls with the text.
- R-128: When a pixel block changes or its preview closes, the plugin
  shall delete its image by id.
- R-129: The plugin shall allocate image ids from a range it owns and shall
  not reuse an id while its image is placed.
- R-130: When the user runs `:KittyMarkdown refresh`, the plugin shall
  re-probe the terminal and re-transmit every placed image.
- R-131: When the probe result changes between text-only mode and image
  mode, the plugin shall re-render every open preview.

### Helper lifecycle

- R-132: When the plugin needs the helper for the first time and no helper
  is installed, the plugin shall download the release binary for the
  platform and plugin version into `stdpath('data')/kitty-markdown/`.
- R-133: If no release binary exists for the platform or the download
  fails, then the plugin shall use `helper/target/release/kitty-markdown-helper`
  when it exists.
- R-134: If no helper binary is available by any route, then the preview
  shall run in text-only mode and show one error message with the build
  command.
- R-135: Where the helper path option is set, the plugin shall use that
  binary and shall not download one.
- R-136: The plugin shall run at most one helper process per Neovim
  instance and shall start it on first need.
- R-137: When Neovim exits, the plugin shall send `shutdown` to the helper
  and wait for it to exit.
- R-138: The plugin and the helper shall exchange JSON lines with a
  request `id`, `method`, and `params`, and a response `id` with `result`
  or `error`.
- R-139: The helper shall answer every request, in any order, with the
  same `id` as the request.
- R-140: When the helper exits without a `shutdown` request, the plugin
  shall report the exit in `:messages` and start a new helper on the next
  request.
- R-141: If a helper request has no answer within the request timeout,
  then the preview shall show a failure box with the reason "timeout".
- R-142: When the plugin starts the helper, it shall send `probe` and
  compare the helper version with the plugin version.
- R-143: If the helper version differs from the plugin version and the
  helper was downloaded, then the plugin shall download the matching
  version.
- R-144: The helper shall provide the CLI
  `kitty-markdown-helper render --mermaid <file> --width <px> --theme <name> -o <out.png>`.
- R-145: The helper shall provide the CLI
  `kitty-markdown-helper render --image <file> --width <px> -o <out.png>`
  and `--svg` with the same arguments.
- R-146: When a CLI render fails, the helper shall write the reason to
  stderr and exit with a non-zero status.
- R-147: The helper shall write pixel results to the cache and return the
  file path and pixel size, not inline bytes.

### Cache

- R-148: The plugin shall keep the cache under `stdpath('cache')/kitty-markdown/`
  unless the cache directory option names another root.
- R-149: The plugin shall keep images, remote fetches, and mermaid renders
  in the subdirectories `images`, `remote`, and `mermaid`.
- R-150: The plugin shall key each cache entry by the SHA-256 of its
  content plus its render parameters.
- R-151: When the cache exceeds the configured size limit, the helper
  shall delete the least recently used entries until the cache is under
  the limit.
- R-152: The plugin shall not delete a cache entry that an open preview
  uses.

### Test hooks

- R-153: The function `layout(bufnr)` shall return the layout model as
  plain Lua tables without a terminal, a preview, or a helper.
- R-154: The layout model shall hold `lines` (strings), `highlights`
  (line, start column, end column, group), `conceals`, and `blocks`.
- R-155: The layout model shall hold, in each block descriptor, `kind`,
  `line`, `source`, `width_cells`, and, when known, `width_px`,
  `height_px`, `rows`, and `image_id`.
- R-156: The function `layout(bufnr, opts)` shall accept block pixel sizes
  in `opts` and reserve rows for the blocks that have one.
- R-157: The function `layout` shall return the same model for the same
  buffer content, options, and window width.

### Interaction

- R-158: When the user presses `q` in the preview, the plugin shall close
  the preview.
- R-159: When the user presses `gx` on a URL in the preview, the plugin
  shall open the URL with the system opener.
- R-160: When the user presses `gx` on a local file link or an image in
  the preview, the plugin shall open the file with the system opener.
- R-161: When the user presses `gx` on a link to a heading anchor, the
  preview shall scroll to that heading.
- R-162: While the preview is a split and scroll sync is on, the preview
  shall scroll so that the block under the source cursor is visible.
- R-163: The plugin shall debounce scroll sync so that at most one scroll
  happens per 100 ms of cursor movement.
- R-164: While the preview is a tab or a float, the plugin shall not sync
  scrolling.

## Open questions

- SSIM threshold: 0.95 is a proposal. Confirm or adjust after the first
  measurements on phase 1 diagrams.
- zmx 0.6.0 does not pass the graphics protocol through. If a later
  release does, test the probe and passthrough path there.
- Font availability on Linux: the mermaid family chain resolves to
  different fonts than on macOS. Decide whether parity goldens are per
  platform or whether the helper bundles a fallback font.
