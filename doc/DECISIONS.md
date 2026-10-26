# Decisions

Internal decisions that `SPEC.md` does not make and the user cannot see.
Each entry says what the code does and why. Newest entries go at the end
of their section. A decision the user can see goes to the architect agent
as a spec proposal instead.

## Oracle and parity

- The parity harness runs the oracle with a `--size` that makes it draw
  the diagram at the helper output width, then crops the oracle PNG to the
  helper size at the top-left corner. mermaid-cli 12.0.0 measures its clip
  in a viewport 16 px wider and then shrinks the SVG by 8 px, so a plain
  `--size` gives a drawing about 1.6% smaller than the helper's.
- Gantt lays out its time scale over the available width, so gantt runs
  the oracle with `--size` equal to the helper width. The parity test then
  scales its own render to `width - 8`, as the oracle screenshot does, and
  crops the golden. The gantt cases set `todayMarker off`, because the
  oracle draws the today line at the real date.
- The pinned base config sets `layout: dagre`. mermaid 12.2.0 then lays
  out mindmap and swimlane diagrams with dagre, so the helper has no
  cose-bilkent or swimlane layout port. Swimlane diagrams go through the
  flowchart pipeline.
- Cynefin seeds its boundary waves from the SVG id `my-svg`, so its oracle
  output is deterministic and the helper uses the same seed.
  `goldens.mjs` still renders mindmap and cynefin five times per case.
- Architecture always lays out with fcose: `layoutArchitecture` never
  reads `config.layout`. It runs fcose twice under `withSeededRandom(1)`,
  so its oracle output is deterministic and needs one golden per case.
  `helper/tests/fcose.rs` replays 14 graphs dumped from cytoscape 3.34.4
  and fcose 2.2.0 and matches every position to 1e-6.
- Venn diagrams with four or more sets use V8's `Math.random` generator
  (xorshift128+) with a fixed seed. The oracle seeds it per page, so no
  golden case has four sets.

## Fonts and text

- resvg matches family names with case. The shared font database adds a
  lower-case alias face for every family (`text::fontdb_arc`), so
  mermaid's chain `"trebuchet ms", verdana, arial, sans-serif` finds
  Trebuchet MS.
- mermaid's default `fontFamily` ends in `;`, which is not valid CSS.
  Chrome then measures and draws that text in the page default font,
  Times New Roman. `css_family_or_page_default` reproduces this.
- Generic families resolve to Chrome's macOS defaults: `sans-serif` to
  Arial, `serif` to Times New Roman, `monospace` to Menlo.
- HTML labels use Chrome's first baseline,
  `floor((line_height - ascent - descent) / 2) + ascent`, with rounded
  `hhea` metrics (`LabelBox.baseline`). resvg places `dominant-baseline:
  central` 0.5 to 1 px lower than Chrome.
- The SVG serializer writes sibling `<tspan>` elements with no whitespace
  between them. Under the default `xml:space`, that whitespace becomes a
  trailing space and moves centered lines by half a space.

- Clip paths and `xml:space` are fields on the scene `Style`, next to
  `mix_blend_mode`, because many renderers build `Common`, `Text`, and
  `Scene` as full struct literals. A clip reference carries its own
  `<clipPath>` definition, and the serializer writes each id once, before
  its first user.
- Chrome draws bold text in a font that has no bold face (Open Sans here)
  by thickening the outline. The raster step adds a stroke of the fill,
  2 x 0.02334 x font size wide, to that text only. `Scene::to_svg` does
  not look up fonts, so the SVG snapshots do not depend on the installed
  fonts.
- The raster font resolver asks `FontSystem` for each family, so the drawn
  face is the measured face (`monospace` draws in Menlo on macOS and in
  DejaVu Sans Mono on Linux). Glyph fallback stays with resvg: routing it
  through `FontSystem::fallback_for` drew arrows in Hiragino Sans and
  lowered cynefin parity by up to 0.006.

- Stylesheet label fonts travel as `Node::label_font` and the `ShapeCtx`
  fields `edge_label_font`, `edge_terminal_font`, and
  `edge_label_padding_x`. Inline label styles merge over them, as the
  browser cascade does. `LabelBox::font` keeps the font a label was
  measured in, so the label is drawn in the same font.
- `calculateTextWidth` measures in the root `config.fontFamily` and
  `config.fontSize`, not in the label font, as mermaid's `utils.ts` does.
- HTML label widths and line widths round up to 1/64 px, because Blink
  lays out the label in LayoutUnits and dagre's alignment can depend on
  the last fraction.
- Edge terminal labels draw no background. Each carries an unpainted rect
  as its `getBBox` box (the `setTerminalWidth` box in HTML mode, the text
  box in SVG mode), so the scene bounds include it.

- The measurer tries the rest of the CSS `font-family` list before the
  platform fallback list, as Chrome does: under mermaid's chain, `→` comes
  from Arial, because Trebuchet MS and Verdana lack it.
- Only a grapheme with U+FE0F, or an Emoji_Presentation=Yes character with
  no U+FE0E, skips the CSS list and tries Apple Color Emoji first, as
  Blink's emoji fallback does. The table is generated from Unicode 16.0
  `emoji-data.txt` into `text/wrap/emoji_presentation.rs`.
- On macOS the platform fallback list starts with Lucida Grande and puts
  Zapf Dingbats before Arial Unicode MS. With that order, `✓`, `★`, and a
  lone `❤` resolve to Lucida Grande, Hiragino Sans, and Zapf Dingbats, the
  faces Chrome 156 draws them in.
- Raster glyph fallback stays with resvg. usvg 0.48 redraws a whole text
  run in a fallback face when that face covers every character of the run,
  so a fallback by CSS list drew the cynefin subtitles in Arial Italic and
  lowered cynefin by 0.0013. A fix needs one `<text>` per font run or a
  patched usvg.
- An inherited `text-decoration: underline` on an HTML label is a rect per
  line, drawn before the text, in the page default black. Its thickness is
  the face's `post` underline thickness rounded (at least 1 px), and it
  sits `max(1, ceil(t / 2))` px under the baseline. Skip-ink gaps are not
  drawn.
- HTML labels honour the label style `text-align` and `white-space:
  nowrap`, as `addHtmlSpan` lays them out.

## Diagram ports

- Langium grammars are hand-written parsers. Wardley emulates
  Chevrotain's error recovery, so its multi-error `Parsing failed:` text
  matches the oracle. Pie and radar report only the first parser error.
- mermaid validation errors that carry no position (wardley ranges,
  packet gaps, radar axes, event modeling frame IDs) become `parse_error`
  with the line of the statement and column 0, and mermaid's message.
- An error that mermaid raises while it renders (for example xychart
  `No Plot to render`, sankey `circular link`, state `No such shape`)
  becomes `internal` with mermaid's message.
- Gantt dates use the system time zone, as the oracle does.
- C4 lays out its outermost level with a width limit of 800 px, which is
  `screen.availWidth` in the oracle's headless Chrome.
- Wardley applies no stylesheet: its `draw` removes the `<style>` element
  first, so the renderer ports `getTheme` as presentation attributes.
- Sankey rounds the viewBox to single precision, as `getBBox` does, so a
  node bottom at 400.00000000000006 gives 400 rows, not 401.
- Architecture measures cytoscape label widths in Helvetica Neue, as the
  oracle's cytoscape stylesheet does, and adds the 1 px group border that
  the default stylesheet gives every group to the bounds.
- Architecture icon text (`service x "text"`) is drawn as SVG text, not as
  the oracle's HTML `foreignObject`.
- At the top level of a graph, edges clip against the nodes named by
  `edge.start` and `edge.end`, as mermaid's common painter does with
  `getRenderedNode`. Every self-loop segment then clips against the real
  node, not the dummy nodes. Edges inside a recursive cluster keep the
  dagre end points.
- Diagram stylesheet marker rules (class, usecase) live in
  `markers::stylesheet_rule`, keyed by diagram type. The unified renderer
  applies them before the shared `.marker` paint.
- The dagre dummy node id counter resets once per diagram render, so two
  renders of one source give the same SVG.
- The theme tells its own `KittyMarkdown*` groups from a user or
  colorscheme group by the `nvim_get_hl` read-back of what it last wrote.
  A group that reads back the same is the plugin's and gets the current
  palette; an empty group is defined again; any other group is kept and
  stops being the plugin's (R-076, R-281, R-282). A user definition equal
  to the plugin's own is not told apart, which the user cannot see.
- The theme registers its `ColorScheme` autocmd (augroup
  `kitty-markdown-theme`) on the first `apply`, which `setup` runs, so a
  colorscheme load defines the groups again before any preview opens.

## Preview and helper client

- A URL is a source that starts with an RFC 3986 scheme
  (`^%a[%w+.-]*:`), the same pattern the layout uses to leave a source
  unresolved. The scheme compares in lower case, so `HTTP://` is fetched.
  Any other scheme fails in the plugin with `fetch_failed` and `status`
  0, before a helper request, so a `file://` URL never reaches the
  remote cache (R-225, R-287).
- When the preview window is the last non-float window of its tab page,
  closing the preview shows the source buffer in it, or a new empty
  buffer when the source is unloaded, and resets the preview window
  options to their global values (R-252).
- `:KittyMarkdown refresh` re-renders only the preview of the current
  source; the other previews get their placed images transmitted and
  placed again without a re-render, so a refresh does not re-request
  their failed blocks.
- A helper exit no longer writes the stderr tail to `:messages`. Every
  `vim.notify` level reaches `:messages`, and R-313 allows one message
  per exit; `helper.stderr_tail()` keeps the lines.
- Scroll sync finds the top-level parser block whose source range holds
  the cursor line (the first one, so a paragraph wins over the image
  lifted out of it), or the nearest block above, and maps the first line
  of that block to the preview.
- An image failure message that does not contain the image source gets
  the source in front: `<path or URL>: <helper message>`.

## Markdown layout

- The parser loads the `markdown` and `markdown_inline` grammars that ship
  with Neovim by file path, under the names `kitty_markdown` and
  `kitty_markdown_inline`, and parses a string copy of the buffer with its
  own injection query (inline content only). It looks in
  `$VIMRUNTIME/parser`, then `lib/nvim/parser` of the install prefix
  (found from `$VIMRUNTIME` and from `v:progpath`), then the parser
  directories on 'runtimepath', bundled ones first. A user parser of
  another version, a runtimepath with no parser directory, or a missing
  `markdown_inline` then give the same model (R-209), and no code fence
  language parser is loaded. With no bundled grammar anywhere, `parse`
  raises an error.
- Single-dollar math nodes from the inline grammar are not used. The
  parser scans the text between the other inline nodes for `$` pairs
  itself, with GitHub's rules (R-280), so the pairing does not depend on
  how the grammar paired them. The scan runs on the text joined over
  soft breaks, and a `$$` node from the grammar stays display math.
- A link or image node that overlaps `[[...]]` or `![[...]]` on its
  source line stays text (R-036). A code span that holds such text stays
  a code span.
- The later paragraphs and blocks of a footnote definition parse as an
  indented code block after the definition, because the grammar has no
  footnote node. The parser parses the lines of that block again as
  markdown, with the link definitions and slug counts of the document,
  and adds the result to the definition.
- A hidden inline HTML comment leaves the spaces on both sides of it;
  the layout shows one space between the words there.
