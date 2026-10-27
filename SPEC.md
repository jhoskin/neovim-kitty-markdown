# Specification

## Status

Draft 2, written 2026-10-22. Draft 1 was written 2026-10-21. No code
exists yet. The requirements below cover the full product. Draft 2 raises
the Neovim minimum to 0.12, pins the oracle config, moves zenuml to the
non-goals, and adds R-165 to R-211 for the gaps that the implementation
design found. R-212 to R-248 were added, and R-042 and R-175 were
revised, on 2026-10-22 from the Wave 1 and Wave 2 package reports.
Changes to this file are Joh's; the architect agent edits it when Joh
asks, and the dev agent proposes wording. Delivery follows the mermaid
phases in the Constraints section; a requirement for a later phase is not
a defect until that phase starts. R-249 to R-345 were added, and R-017,
R-043, R-055, R-088, R-098, R-112, R-123, R-130, R-175, R-179, R-181,
R-234, and R-237 were revised, on 2026-10-25 to answer the open questions
of the QA review. R-346 to R-353 were added, and R-225, R-234, R-252,
R-273, R-279, R-297, and R-324 were revised, on 2026-10-27 from the dev
agent proposals for QA round 1.

Defaults that the plan did not fix and that this draft sets:

- Front matter is hidden, not shown raw.
- Remote image size cap: 20 MiB (20971520 bytes).
- Cache size limit: 500 MiB (524288000 bytes).
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
- Mermaid handDrawn look (rough.js), the `neo` look, ELK layout, icon
  packs, image nodes, and the mermaid 12 themes `neo`, `neo-dark`,
  `redux`, `redux-dark`, `redux-color`, and `redux-dark-color`. These
  render as a failure box.
- The zenuml, info, agentflow, and railroad diagram types. zenuml lays
  out with CSS inside a `foreignObject`, so no SVG geometry exists to
  port. These render as a failure box. mermaid-cli 12.0.0 loads
  `@mermaid-js/mermaid-zenuml` and renders zenuml; the helper differs
  from the oracle here on purpose.
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
- Measurement oracle: `helper/oracle/measure.mjs`, which runs Chrome
  through puppeteer and reports `getBBox` for an SVG label and
  `getBoundingClientRect` for an HTML label.
- Text measurer: the helper code that computes the size of a label for
  mermaid layout.
- Math token: one glyph, one number, or one identifier in a math span,
  together with its attached superscripts and subscripts.
- SSIM: the structural similarity index between two images of the same
  pixel size, in the range -1 to 1. Both images are composited over white
  and converted to BT.601 grayscale. The index uses a Gaussian window of
  11 by 11 pixels with sigma 1.5, K1 0.01, K2 0.03, and L 255. It uses
  only the window positions that fit inside the image, with no padding,
  and reports the mean over those positions. When a side of an image is
  shorter than 11 pixels, the index uses the mean, variance, and
  covariance of the whole images in place of the window.
- Natural size: the pixel size of a diagram as mermaid computes it from
  its SVG `viewBox`, before any scaling to the target width.
- Anchor line: the one empty line in the layout model that stands for a
  pixel block. Boxes and placeholder rows hang below it as virtual lines.
- Block key: the SHA-256 of the kind, the source, the width in cells, the
  theme, and the `<img>` attributes of a pixel block. For an image, the
  source is the resolved path. It identifies the block across re-renders.
  A caller reads it from the `key` field of the block descriptor.
- Fallback text: text with at least one character outside ASCII, emoji
  included. The text measurer may use a fallback font for it.
- Icons: the glyphs the preview shows. Headings 1 to 6: U+F0CA1, U+F0CA3,
  U+F0CA5, U+F0CA7, U+F0CA9, U+F0CAB. Bullets by depth, repeating: U+25CF,
  U+25CB, U+25C6, U+25C7. Task items: checked U+F0C52, unchecked U+F0131.
  Alerts: `NOTE` U+F02FD, `TIP` U+F0336, `IMPORTANT` U+F017E, `WARNING`
  U+F002A, `CAUTION` U+F0CE6. Loading box U+F051F, failure box U+F0156,
  quote bar U+258E, thematic break U+2500.
- Key string: the fields of a cache key joined by `|` with no spaces. A
  hash field is the lowercase hex SHA-256 of the image bytes or of the
  diagram source. A number is decimal. An absent limit is `0`.
- Base config: the mermaid config the helper applies before the diagram
  directives: `layout` `dagre`, `look` `classic`, and the selected theme.
- Cache: the directory tree under the configured cache root, with the
  subdirectories `images`, `remote`, and `mermaid`.
- System opener: `vim.ui.open`, which hands a path or URL to the operating
  system.

## Constraints

- Each requirement has a stable ID `R-NNN`, one testable statement, one
  EARS pattern, and ASD-STE100 prose. IDs are never reused or renumbered.
- Neovim 0.12 or later. Platforms: macOS arm64, Linux x86_64, Linux arm64.
  Neovim 0.12 is the first release whose `TermResponse` event delivers the
  kitty graphics (APC) reply and the Primary Device Attributes reply.
  The plugin uses the `markdown` and `markdown_inline` parsers that Neovim
  bundles. A Neovim build without them is not supported.
- kitty graphics protocol as documented by kitty, with Unicode
  placeholders (kitty 0.28 or later).
- Mermaid pin: mermaid 12.2.0 through `@mermaid-js/mermaid-cli` 12.0.0.
  The dev agent bumps the pin on purpose and records the new version here.
- The oracle and the helper run on the same machine with the same font
  set. The helper bundles Open Sans Regular and Italic, the fonts that
  mermaid-cli loads into its page, so both sides see them.
- Mermaid 12.0 changed its defaults to ELK layout, the `neo` look, and
  the `redux-color` theme. The oracle runs with the config file
  `helper/oracle/mermaid-config.json`, which holds
  `{"layout":"dagre","look":"classic","theme":"default","handDrawnSeed":1}`,
  and the helper applies the same base config. The oracle package pins
  `@mermaid-js/mermaid-cli` 12.0.0 with an npm override of `mermaid` to
  12.2.0 and a committed lockfile.
- The parity check and the text metrics calibration run on macOS. The
  generic families `sans-serif`, `serif`, and `monospace` resolve to
  Chrome's macOS defaults. Linux parity is out of scope for this release.
- Mermaid diagram types arrive in these phases:

| Phase | Diagram types                                                                                   |
|-------|-------------------------------------------------------------------------------------------------|
| 1     | flowchart, sequence, class, state                                                               |
| 2     | ER, pie, gantt, xychart, quadrant                                                               |
| 3     | gitgraph, mindmap, timeline, user journey                                                       |
| 4     | sankey, block, packet, kanban, architecture, C4, requirement                                    |
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
- R-165: If the user runs `:KittyMarkdown open` in a buffer whose filetype
  is not `markdown`, then the plugin shall show the message
  `kitty-markdown: not a markdown buffer (filetype is "<filetype>")`.
- R-166: If Neovim is older than 0.12, then the plugin shall show the
  message `kitty-markdown: Neovim 0.12 or later is required` and open no
  preview.
- R-167: The preview buffer shall have the name
  `kitty-markdown://<source buffer name>`.
- R-215: When the user runs `:KittyMarkdown close`, `toggle`, or `refresh`
  in a preview window, the plugin shall act on the source buffer of that
  preview.
- R-249: If the user runs `:KittyMarkdown` with a subcommand other than
  `open`, `close`, `toggle`, or `refresh`, then the plugin shall show the
  error `kitty-markdown: unknown subcommand "<subcommand>"` and do
  nothing.
- R-250: If the user gives `:KittyMarkdown` an argument that its
  subcommand does not accept, then the plugin shall show the error
  `kitty-markdown: unexpected argument "<argument>"` and do nothing. This
  includes a placement other than `split`, `tab`, or `float`, a placement
  after `close` or `refresh`, and a third argument.
- R-251: The plugin shall define `:KittyMarkdown` so that Neovim rejects
  the command without a subcommand with its error E471.
- R-252: If the preview window is the last window of its tab page and
  another tab page exists when the plugin closes the preview, then the
  plugin shall delete the preview buffer and close that tab page.
- R-346: If the preview window is the only window of the only tab page
  when the plugin closes the preview, then the plugin shall delete the
  preview buffer and show the source buffer in that window.
- R-347: If the source buffer is not loaded when R-346 applies, then the
  plugin shall show an empty buffer in that window.
- R-253: The preview window shall hide the absolute line numbers and the
  relative line numbers.
- R-254: The `<source buffer name>` of R-167 shall be the full path of the
  source buffer.
- R-255: If the source buffer has no name, then the preview buffer name
  shall be `kitty-markdown://`.

### Refresh and performance

- R-016: When the source buffer is written to disk, the plugin shall
  re-render its preview.
- R-017: When the user runs `:KittyMarkdown refresh`, the plugin shall
  re-render the preview of the source buffer, re-probe the terminal, and
  re-transmit the images of every open preview.
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
- R-168: The layout model shall hold one anchor line for each pixel
  block.
- R-169: The preview shall show loading boxes, failure boxes, text-only
  boxes, and placeholder rows as virtual lines below the anchor line.
- R-170: The plugin shall treat a pixel block as unchanged when its block
  key is unchanged.
- R-216: When a re-render keeps a pixel block whose last render failed,
  the plugin shall request that block again.
- R-217: The anchor line of a pixel block shall stand in for the blank
  line before that block.
- R-256: The 1000-line document of R-023 shall be a repeated mix of
  headings, wrapped paragraphs with inline markup and links, nested
  lists, tables, code fences, blockquotes, and images. The time runs from
  the command to the first text in the preview.
- R-257: The render of R-024 shall be a flowchart with at most 20 nodes
  that is not in the cache. The time runs from the request to the
  result. R-023 and R-024 apply on the machine that runs the parity
  check.
- R-258: When a pixel block is the first block of the document, the
  anchor line of that block shall be line 1 of the preview.
- R-259: The preview shall keep one blank line after a pixel block, also
  when the pixel block is the last block of the document.

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
  unchecked checkbox icon in place of the bullet and the `[x]` or `[ ]`
  marker.
- R-349: When a task list item is in an ordered list, the preview shall
  show the checkbox icon in place of the number.
- R-350: The preview shall count a task list item in the numbering of its
  ordered list.
- R-043: The preview shall show a table with Unicode box drawing lines
  around the table, below the header row, and between the columns. The
  preview shall not draw a line between two body rows.
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
  across the text width in the border color of the palette.
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
- R-171: The preview text shall hold the rendered text without the
  markdown markers.
- R-218: If a footnote definition has no reference, then the preview shall
  not show it.
- R-219: If a footnote reference has no definition, then the preview shall
  show the reference as written.
- R-260: If a table is wider than the text width, then the preview shall
  show the table at its full width and shall not wrap or truncate a cell.
- R-261: If a code line is wider than the text width, then the preview
  shall show the full line and shall not wrap or truncate it.
- R-262: If a word is wider than the text width, then the preview shall
  show the word on its own line and shall not split it.
- R-263: The preview shall hide the angle brackets of an autolink.
- R-264: The preview shall start the marker of a nested list item 2 cells
  to the right of the start of the marker of its parent item.
- R-265: The preview shall show one vertical bar for each blockquote
  nesting level.
- R-266: The preview shall show the alert title as `Note`, `Tip`,
  `Important`, `Warning`, or `Caution`.
- R-267: The preview shall recognize the alert kind in any letter case,
  as GitHub does.
- R-268: The preview shall hide front matter that opens and closes with
  `---` or with `+++`.
- R-269: The preview shall not treat a block that closes with `...` as
  front matter.
- R-270: The preview shall hide HTML comments, in HTML blocks and inline.
- R-351: If a space is on each side of a hidden inline HTML comment, then
  the preview shall show one space in place of the two.
- R-271: The preview shall show the text between inline HTML tags in the
  text color and only the tags in the muted color.
- R-272: The preview shall treat `[X]` as a checked task marker, the same
  as `[x]`.
- R-273: The preview shall show every paragraph and block of a footnote
  definition at the end of the document, aligned with the text after its
  number.
- R-274: The preview shall use the glyphs that the Icons definition
  lists.
- R-275: When a heading slug is already in use, the layout shall append
  `-1`, `-2`, and so on, and take the first result that no earlier
  heading uses, as `github-slugger` does.

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
- R-220: When the preview shows math source, the preview shall show the
  TeX between the delimiters without the `$` markers.
- R-221: The preview shall show the whitespace between two math tokens as
  one space and shall omit whitespace at the ends of a math span.
- R-222: The preview shall show `\sqrt{x}` as `√x`, with parentheses
  around a radicand that has more than one math token.
- R-223: The preview shall map `\epsilon` and `\phi` to U+03B5 and
  U+03C6, and `\varepsilon` and `\varphi` to U+03F5 and U+03D5.
- R-276: The preview shall approximate a nested fraction with the rule of
  R-068 at each level.
- R-277: If one character of a superscript or a subscript has no Unicode
  script form, then the preview shall show the whole math span as source
  (R-069).
- R-278: When an inline math span falls back to the source, the preview
  shall apply the whitespace rules of R-221 to the source.
- R-279: When a display math span falls back to the source, the preview
  shall show each source line on its own preview line, centered as one
  block with a common left edge.
- R-352: When a display math span falls back to the source, the preview
  shall apply the whitespace rules of R-221 to each source line.
- R-280: The preview shall open inline math only at a `$` that a
  character other than a space follows, and close it only at a `$` that
  a character other than a space comes before, as GitHub does.
- R-353: The preview shall not open or close inline math at a `$` that a
  backslash comes before.

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
- R-281: The plugin shall not overwrite a `KittyMarkdown*` group that the
  user defined, whenever the user defined it.
- R-282: When a colorscheme loads, the plugin shall define again each
  `KittyMarkdown*` group that has no definition.
- R-283: The text color and the link color of each palette shall have a
  contrast ratio of at least 4.5 to 1 against the background color of
  that palette.

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
  width or at its declared width, whichever is smaller.
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
- R-172: If a local image file cannot be read, then the preview shall show
  a failure box with the reason "cannot read file".
- R-173: If a remote fetch fails for a reason other than timeout or size,
  then the preview shall show a failure box with the reason "fetch
  failed".
- R-224: If an image file in a supported format is corrupt, then the
  preview shall show a failure box with the reason "unsupported format".
- R-225: If an image URL has a scheme other than `http` or `https`, then
  the preview shall show a failure box with the reason "fetch failed".
  This also applies where remote images are disabled, because R-085
  covers only `http` and `https` images.
- R-348: The plugin shall compare the scheme of an image URL without
  regard to letter case.
- R-226: When an image is SVG, the helper shall rasterize it onto a
  transparent background.
- R-227: The plugin shall place an image over the number of columns equal
  to its width in pixels divided by the cell width, rounded up, capped at
  the text width.
- R-228: Where remote images are disabled or no helper is available, the
  plugin shall not write a message per pixel block to `:messages`.
- R-284: The plugin shall resolve a relative image path or a relative file
  link against the directory of the source file.
- R-285: If the source buffer has no file name, then the plugin shall
  resolve a relative path against the current directory of Neovim.
- R-286: When an `<img>` tag gives only a width or only a height, the
  plugin shall compute the other dimension from the aspect ratio.
- R-287: The plugin shall treat a `file://` URL as a URL with a scheme
  other than `http` or `https` (R-225).
- R-288: When a pixel block fails, the plugin shall write
  `kitty-markdown: <kind> <title>: <code>: <message>` to `:messages`.
  `<title>` is the alt text or the kind. `<message>` names the path or
  URL and the cause, such as the HTTP status.
- R-289: The timeout of R-082 shall be the option
  `remote_images.timeout_ms`, with the default 10000.
- R-290: The size cap of R-083 shall be the option
  `remote_images.max_bytes`, with the default 20971520.
- R-291: When a render meets a pixel block whose fetch failed, the
  plugin shall request the fetch again.
- R-292: If a fetch fails, then the helper shall store nothing in the
  `remote` cache for that URL.

### Mermaid diagrams

- R-097: The plugin shall treat a fenced code block with the info string
  `mermaid` as a mermaid block.
- R-098: When the plugin renders a mermaid block, the helper shall render
  the diagram to PNG at its natural width when that width is at most the
  target width, and at the target width otherwise.
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
  architecture, C4, and requirement diagrams (phase 4).
- R-109: The helper shall render radar, treemap, venn, ishikawa, wardley,
  cynefin, treeview, event modeling, use case, and swimlane diagrams
  (phase 5).
- R-110: The helper output for each diagram type in a delivered phase
  shall reach an SSIM of 0.95 or more against the oracle at the same pixel
  size and fonts.
- R-111: If a mermaid diagram does not parse, then the preview shall show
  a failure box with the diagram type and the parse error line, in the
  form the oracle uses.
- R-112: If a diagram uses the handDrawn look, ELK layout, an icon pack,
  or an image node, then the preview shall show a failure box with the
  reason `unsupported: <feature>`.
- R-113: If a diagram type is not in a delivered phase, then the preview
  shall show a failure box with the reason "unsupported diagram type".
- R-114: When a diagram sets its own width smaller than the target width,
  the helper shall render at the diagram width.
- R-115: The helper shall resolve the mermaid font family chain
  `"trebuchet ms", verdana, arial, sans-serif` against the system fonts, in
  that order.
- R-116: The plugin shall cache each mermaid render under a hash of the
  diagram source, the width, the theme, and the mermaid version.
- R-174: The helper shall apply the base config before the directives and
  the front matter of the diagram.
- R-175: If a diagram sets `look` to `neo` or `handDrawn` or `layout` to
  `elk`, then the preview shall show a failure box with the reason
  `unsupported: <feature>`.
- R-176: The helper shall treat the zenuml, info, agentflow, and railroad
  diagram types as types outside the delivered phases.
- R-177: If a mermaid diagram does not parse, then the failure box reason
  shall be `Parse error on line <n>`, where `<n>` is the line the oracle
  reports.
- R-178: When a mermaid diagram does not parse, the plugin shall write the
  full error to `:messages` in the form the oracle uses, with the caret
  line and the expected tokens.
- R-179: The helper shall use the bundled Open Sans faces for every
  request of the Open Sans family, also when the system has an Open Sans
  font.
- R-180: The helper shall render a diagram at its natural size when that
  size is narrower than the target width.
- R-181: The parity check shall run the oracle with the `--size` of
  R-237, crop the golden to the helper output size from the top left
  corner, and compare at equal pixel size, with a tolerance of 1 pixel.
- R-182: The parity check for mindmap and cynefin diagrams shall render
  each case five times with the oracle and pass when the SSIM against any
  one golden reaches 0.95.
- R-214: The text measurer shall match the measurement oracle within
  0.5 px for Latin text and within 3 px for text that needs a fallback
  font.
- R-229: When the plugin renders a mermaid block, the helper shall render
  the diagram onto a transparent background.
- R-230: If no diagram type matches the diagram text, then the preview
  shall show a failure box with the reason "Parse error on line 1".
- R-231: When no diagram type matches the diagram text, the plugin shall
  write `No diagram type detected matching given configuration for text:
  <text>` to `:messages`.
- R-232: If a diagram starts with `---` and the front matter does not
  close, then the preview shall show a failure box with the reason "Parse
  error on line 1".
- R-233: If the front matter of a diagram is not valid YAML, then the
  preview shall show a failure box with the reason "Parse error on line
  1".
- R-234: If the theme name of a diagram, after R-295, is a name that
  mermaid 12.2.0 does not register, then the helper shall apply the theme
  of the base config.
- R-235: When a diagram sets `themeVariables` without `theme`, the helper
  shall merge the variables over the current theme variables and shall
  not derive a new theme.
- R-236: The parity check shall render each case at a target width of 960
  pixels.
- R-237: The parity check shall choose the oracle `--size` so that
  mermaid-cli 12.0.0 draws the diagram at the helper output width. The
  check shall not resample either image. mermaid-cli reflows the SVG
  8 pixels narrower than `--size` before the screenshot, so the sizes are:
  - a gantt diagram: the helper output width.
  - a diagram with a fixed SVG size: the longer side of the helper output.
  - a diagram at least as wide as it is tall: the helper output width
    plus 8.
  - a taller diagram: the smallest `n` for which `round(n * aspect)`
    equals the helper output width plus 8, where `aspect` is the width
    divided by the height.
- R-293: The natural width of a gantt diagram shall be the target width,
  or `gantt.useWidth` when the diagram sets it.
- R-294: If a diagram sets the theme `neo`, `neo-dark`, `redux`,
  `redux-dark`, `redux-color`, or `redux-dark-color`, then the preview
  shall show a failure box with the reason `unsupported: theme: <name>`.
- R-295: When an init directive and the front matter set the same key,
  the helper shall apply the value of the init directive.
- R-296: The `<feature>` of an unsupported reason shall be one of
  `look: <name>`, `layout: <name>`, `theme: <name>`, `icon pack`,
  `image node`, `randomize`, `participant type <kind>`, or
  `timeline direction TD`.
- R-297: The `<text>` of R-231 shall be the whole diagram text without
  front matter, directives, and `%%` comments, without shortening. The
  text shall keep its line breaks and one line break for each removed
  directive line. The removed front matter shall leave no line break.
- R-298: The helper shall treat the railroad, railroad EBNF, railroad
  ABNF, and railroad PEG diagram types as types outside the delivered
  phases.
- R-299: When R-235 merges a theme variable, the colors that the theme
  derived from other variables shall keep their values.
- R-300: The diagram type in the failure box of R-111 shall be the first
  keyword of the diagram, such as `sequenceDiagram`.
- R-301: The tolerance of R-214 shall apply to the width and to the
  height of each whole label.
- R-302: The 3 px tolerance of R-214 shall apply to fallback text.

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
  at most 4096 bytes, with `m=1` on every chunk but the last and `q=2` on
  every chunk.
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
  re-probe the terminal and re-transmit every placed image of every open
  preview.
- R-131: When the probe result changes between text-only mode and image
  mode, the plugin shall re-render every open preview.
- R-183: The plugin shall read the cell size from the pixel size of its
  terminal window (`TIOCGWINSZ`).
- R-184: If the terminal reports a pixel size of zero, then the preview
  shall run in text-only mode and write the reason to `:messages`.
- R-185: If `termguicolors` is off, then the preview shall run in
  text-only mode and write the reason to `:messages`.
- R-186: The plugin shall allocate image ids in increasing order from a
  start value between 1 and 2^23 that it picks per Neovim instance.
- R-187: The plugin shall not reuse an image id during the life of the
  Neovim instance.
- R-188: The plugin shall write the row diacritic and the column diacritic
  on every placeholder cell.
- R-189: The plugin shall transmit image data with `t=d` and `f=100` and
  shall not use a file path transmission.
- R-190: The plugin shall receive the probe answers through the
  `TermResponse` event and shall not block Neovim while it waits.
- R-238: If a pixel block needs more than 297 rows, then the preview shall
  show a failure box with the reason "too tall".
- R-239: When the probe result changes, the plugin shall write the new
  mode and its reason to `:messages` once.
- R-240: When the probe turns images off, the plugin shall give one of
  the reasons `no graphics reply`, `probe timeout`, `termguicolors off`,
  or `cell size unknown`.
- R-241: When the colorscheme changes, the plugin shall keep the
  foreground color that carries the image id on every placeholder cell.
- R-303: The plugin shall send the Primary Device Attributes request
  without the tmux passthrough.
- R-304: The plugin shall pick the start id of R-186 at random.
- R-305: When the probe turns images on, the plugin shall write
  `kitty-markdown: kitty graphics on, images render inline` to
  `:messages` at the `INFO` level.
- R-306: When the probe turns images off, the plugin shall write
  `kitty-markdown: text-only mode: <reason>` to `:messages` at the `INFO`
  level.
- R-307: If the graphics reply arrives after the probe timeout, then the
  plugin shall ignore it until the next probe.
- R-308: When the plugin deletes an image, it shall use `d=I`, which also
  frees the image data in the terminal.
- R-309: The plugin shall treat any graphics reply to the probe, an error
  reply included, as an answer that turns images on.

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
- R-191: The plugin shall download the helper from
  `https://github.com/jhoskin/neovim-kitty-markdown/releases/download/v<version>/kitty-markdown-helper-<target>.tar.gz`.
- R-192: The plugin shall use the target names `aarch64-apple-darwin`,
  `x86_64-unknown-linux-gnu`, and `aarch64-unknown-linux-gnu`.
- R-193: The plugin shall install the downloaded binary at
  `stdpath('data')/kitty-markdown/kitty-markdown-helper`.
- R-194: If no helper binary is available by any route, then the plugin
  shall show the message `kitty-markdown: no helper binary found; run:
  cargo build --release --manifest-path helper/Cargo.toml`.
- R-195: The plugin shall report a helper exit with `vim.notify` at the
  `WARN` level and shall not open a popup.
- R-196: If the helper exits three times within 60 seconds, then the
  plugin shall stop restarting it until the next `:KittyMarkdown refresh`.
- R-197: When Neovim exits, the plugin shall wait at most 2 seconds for
  the helper to exit and then kill it.
- R-198: If the helper exits while a pixel block waits, then the preview
  shall show a failure box with the reason "helper exited".
- R-199: The helper shall provide the CLI
  `kitty-markdown-helper compare <a.png> <b.png>`, which prints the SSIM
  of the two images.
- R-200: When the CLI runs `render`, the helper shall write the result to
  the `-o` path and shall not require a cache directory.
- R-242: If the helper version differs from the plugin version and the
  helper is a user path or a local build, then the plugin shall show one
  `WARN` message and continue.
- R-243: If the helper path option names a file that is not executable,
  then the plugin shall show one `WARN` message and shall not download a
  binary.
- R-244: If the helper exits three times within 60 seconds, then the
  plugin shall show one `ERROR` message that names `:KittyMarkdown
  refresh`.
- R-245: While the plugin has stopped restarting the helper, the preview
  shall show each pixel block as a text-only box.
- R-246: If the helper reaches the end of its input without a `shutdown`
  request, then the helper shall answer every accepted request and exit
  with status 0.
- R-247: If a request line is not valid JSON or has no `method`, then the
  helper shall answer with the error code `invalid_params` and the
  request `id`, or `id` 0 when the line has no readable id.
- R-310: When the user runs `:KittyMarkdown refresh`, the plugin shall
  clear the restart stop of R-196, also when no preview is open.
- R-311: If the helper path option names a file that does not exist,
  then the plugin shall treat the file as not executable (R-243).
- R-312: After the `WARN` of R-243, the plugin shall try the installed
  binary and then the local build.
- R-313: When the helper exits, the plugin shall write only the one
  `WARN` of R-195 and shall not write a message for each failed pixel
  block.
- R-314: The helper shall treat a request whose `method` is not a string
  as a request without a `method` (R-247).
- R-315: The helper shall not answer an empty line or a line of only
  whitespace.
- R-316: If a request line is not valid JSON, then the helper shall
  answer with `id` 0.
- R-317: The helper shall accept a request `id` that is a non-negative
  integer.
- R-318: If a request `id` is not a non-negative integer, then the
  helper shall answer with the error code `invalid_params` and `id` 0.
- R-319: The `probe` result shall hold `helper_version`,
  `mermaid_version`, `cache_bytes`, `fonts`, and `diagram_types`.
- R-320: A result that carries pixels shall hold `path`, `width_px`,
  `height_px`, `bytes`, and `cached`.
- R-321: The command `compare` shall print the SSIM with four decimals
  and a line break, print nothing else, and exit with status 0.
- R-322: If the two images given to `compare` differ by at most 1 pixel
  on each axis, then the helper shall crop both images to the common
  size around their centers before it computes the SSIM.
- R-323: If the two images given to `compare` differ by more than 1 pixel
  on an axis, then the helper shall write
  `kitty-markdown-helper: image sizes differ: <aw>x<ah> vs <bw>x<bh>` to
  stderr and exit with status 1.
- R-324: When a CLI command fails, the helper shall write one line
  `kitty-markdown-helper: <reason>` to stderr and exit with status 1.
  In `<reason>`, the helper shall write each line break as `\n` and each
  backslash as `\\`.
- R-325: If the CLI arguments are not valid, then the helper shall exit
  with status 2.
- R-326: The request timeout of R-141 shall be the option
  `helper.request_timeout_ms`, a positive integer with the default 30000.
- R-327: If no release binary exists for the platform, then the plugin
  shall show the `WARN`
  `kitty-markdown: helper download failed: no release binary for this
  platform` once and use the local build.

### Cache

- R-148: The plugin shall keep the cache under `stdpath('cache')/kitty-markdown/`
  unless the cache directory option names another root.
- R-149: The plugin shall keep images, remote fetches, and mermaid renders
  in the subdirectories `images`, `remote`, and `mermaid`.
- R-150: The plugin shall key each image and mermaid cache entry by the
  SHA-256 of its content plus its render parameters, and each remote entry
  by the SHA-256 of its URL.
- R-151: When the cache exceeds the configured size limit, the plugin
  shall request garbage collection, and the helper shall delete the least
  recently used entries until the cache is under the limit.
- R-152: When the plugin requests garbage collection, it shall pass the
  paths that open previews use, and the helper shall not delete them.
- R-201: The helper shall set the modification time of a cache entry to
  the current time on each use.
- R-202: When the cache size estimate of the plugin exceeds the limit
  after a render, the plugin shall request garbage collection once.
- R-212: The plugin and the helper shall compute the key of an image entry
  as the SHA-256 of the key string
  `<content hash>|<width in pixels>|<height limit>|<width limit>`.
- R-213: The plugin and the helper shall compute the key of a mermaid
  entry as the SHA-256 of the key string
  `<source hash>|<width in pixels>|<theme>|<mermaid version>`.
- R-328: The paths of R-152 shall be the prepared PNG of each block and,
  for a remote image, also the fetched file in `remote`.
- R-329: While the cache size estimate exceeds the limit, the plugin
  shall request garbage collection once after each render batch.
- R-330: The source hash of R-213 shall cover the lines of the fence body
  joined by line feeds, without a final line feed.
- R-331: The test of the mermaid version in the key of R-116 shall be
  the key string of R-213.

### Test hooks

- R-153: The function `layout(bufnr)` shall return the layout model as
  plain Lua tables without a terminal, a preview, or a helper.
- R-154: The layout model shall hold `lines` (strings), `highlights`
  (line, start column, end column, group), `conceals`, and `blocks`.
  `conceals` may be empty.
- R-155: The layout model shall hold, in each block descriptor, `kind`,
  `line`, `source`, `width_cells`, and, when known, `width_px`,
  `height_px`, `rows`, and `image_id`. The layout sets `image_id` only
  from `opts.sizes`.
- R-156: The function `layout(bufnr, opts)` shall accept block pixel sizes
  in `opts` and reserve rows for the blocks that have one.
- R-157: The function `layout` shall return the same model for the same
  buffer content, options, and window width.
- R-203: The function `layout(bufnr, opts)` shall accept the keys `width`,
  `cell`, `sizes`, `theme`, `images`, and `dir` in `opts`.
- R-204: The function `layout` shall key `opts.sizes` by the block key.
- R-205: The layout model shall hold `source_lines`, which maps each
  preview line to the source line that produced it.
- R-206: The layout model shall hold `anchors`, which maps each heading
  slug in GitHub form to its preview line.
- R-207: The layout model shall hold `links` with the line, start column,
  end column, kind, and target of each link and image.
- R-208: The layout model shall hold `code_regions` with the first line,
  last line, and language of each fenced code block.
- R-209: The function `layout` shall return the same model regardless of
  which tree-sitter parsers are installed.
- R-210: The command `nvim -l tests/run.lua [filter]` shall run the unit
  tests and exit with a non-zero status when a test fails.
- R-332: The `width` in `opts` shall be the preview window width.
- R-333: The `cell` in `opts` shall hold `width_px` and `height_px`.
- R-334: Each entry of `sizes` in `opts` shall hold `width_px`,
  `height_px`, and `image_id`.
- R-335: Each block descriptor shall hold `key`, the block key.
- R-336: The layout shall compute the reserved rows from the `height_px`
  of `opts.sizes`, which is the height after scaling.
- R-337: Lines in the layout model shall be 1-based. Columns shall be
  0-based byte offsets with an exclusive end.
- R-338: The layout shall give a code region to each fenced code block
  with at least one line and to each indented code block. A mermaid
  fence shall get no code region.
- R-339: An indented code block shall have no language in its code
  region.

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
- R-211: When scroll sync moves the preview, the preview shall place the
  target line in the top third of the window.
- R-248: When scroll sync moves the preview, the target line shall be the
  first line of the block under the source cursor.
- R-340: If the source cursor is on a blank line between two blocks, then
  scroll sync shall target the block above that line.
- R-341: If the source cursor is in front matter that the preview hides,
  then scroll sync shall target preview line 1.
- R-342: The plugin shall scroll after the cursor movement pauses for
  100 ms, and not at the start of the movement.
- R-343: When scroll sync moves the preview, the number of window lines
  above the target line shall be the window height divided by 3, rounded
  down, or fewer at the top of the buffer.
- R-344: The plugin shall resolve a file link that `gx` opens with the
  rules of R-284 and R-285.
- R-345: The test of R-179 shall run on a machine with a system Open Sans
  font, so that the bundled faces and the system faces differ.

## Open questions

- SSIM threshold: 0.95 is a proposal. Confirm or adjust after the first
  measurements on phase 1 diagrams.
- zmx 0.6.0 does not pass the graphics protocol through. If a later
  release does, test the probe and passthrough path there.
- Font availability on Linux: the mermaid family chain and the generic
  families resolve to different fonts than on macOS. Decided 2026-10-22:
  the parity check and the calibration run on macOS for this release
  (Constraints). Open: a Linux font set, and whether to bundle a
  Trebuchet MS substitute.
- Mindmap layout in mermaid 12.2.0 is unseeded (cytoscape cose-bilkent),
  so R-182 uses the best of five goldens. Revisit if a later mermaid
  release seeds it.
