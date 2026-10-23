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
- The dagre dummy node id counter resets once per diagram render, so two
  renders of one source give the same SVG.
