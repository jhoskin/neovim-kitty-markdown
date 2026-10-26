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
  DejaVu Sans Mono on Linux). Glyph fallback follows the measurer too; see
  the raster fallback entries under Fallback text measurement.

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
- Kanban parses `encodeEntities` of the text, as flowchart and state do,
  so `#quot;` and `#35;` in a card, ticket, or assignee show as the
  character. A section title is measured decoded; the oracle measures the
  encoded markers and decodes them only in the finished SVG, so a section
  title with an entity code can be centred a few pixels apart.
- Kanban sections that share an id repeat their cards as the oracle does:
  `getData` lists every node of the id after each such section, and the
  renderer filters that list by id again, so `k` sections of one id draw
  `k^3` cards in all.
- Kanban SVG text labels (`htmlLabels: false`) start at `x = 0`, because
  the kanban sheet has no `text-anchor` rule. The shared label builder
  centres SVG text, as the flowchart sheet does, so the kanban renderer
  sets the anchor of its label texts back to `start` after building them.

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

## Fallback text measurement

These entries replace the macOS platform fallback list in Fonts and text.

- On macOS the measurer asks CoreText for the platform fallback, through
  `CTFontCreateForString` with the primary face as the base, as Blink's
  `PlatformFallbackFontForCharacter` does. It keeps the family name and
  matches the requested weight and style in it. A fixed list gave Han to
  Hiragino Sans (Chrome: PingFang SC), Arabic italic, Devanagari, and Thai
  to Arial Unicode MS (Chrome: Geeza Pro, Kohinoor Devanagari, Thonburi),
  and `₹` to Geneva (Chrome: Helvetica), up to 8 px wide. CoreText's
  cascade list is compiled into the framework, so FFI is the only source.
  Linux keeps the fontconfig-order list.
- The font database also loads
  `/System/Library/PrivateFrameworks/FontServices.framework/Resources/Reserved`,
  where macOS keeps PingFang. It loads after the system fonts, so resvg's
  own glyph fallback still finds the earlier faces first.
- PingFang keeps its outlines in a table that ttf-parser does not read.
  Its ink boxes come from `CTFontCreatePathForGlyph` and
  `CGPathGetPathBoundingBox`.
- A grapheme with emoji presentation goes to the color emoji face even when
  the primary face has a text glyph (`☺️` under Arial).
- A synthetic oblique skews the glyph outline and takes the tight box of
  the skewed curves. Skewing the corners of the upright box was up to
  1.4 px too wide.
- The horizontal `trak` table adds to each cluster's advance at the font
  size in CSS px: the track of value 0, or the only track. This applies
  only to PingFang among the stock faces (track 1, 20 units at 16 px).
  Chrome does not shift the glyph by half the tracking, as HarfBuzz would.
- A color emoji box reaches one pixel above the em box, whatever the
  primary face. Under a synthetic oblique its right edge moves by the skew
  of that top, rounded up to a whole pixel.
- `measure.mjs` replaces `<br>` with a space in SVG mode, as
  `drawSimpleText` does, and records the face of each fallback grapheme
  with `CSS.getPlatformFontsForNode`.
- The raster step draws each fallback grapheme in the face the measurer
  chose (`TextMetrics::grapheme_faces`), which reverses the earlier choice
  to keep resvg's own fallback. The raster SVG gives each fallback stretch
  a `tspan` whose `font-family` is a token that names one face, so usvg
  never falls back and never redraws a whole run in a face that covers it
  (that redraw is what cost cynefin 0.0013 under a CSS-list fallback).
  `Scene::to_svg` asks for no pieces, so the snapshots do not change.
- A piece's `letter-spacing` makes usvg's advance equal the measured one:
  usvg shapes with HarfBuzz rules only, while Chrome adds PingFang's
  `trak` and draws Apple Color Emoji 1.25 em wide. usvg adds no spacing
  after the last cluster of a chunk, so a centered or end-anchored chunk
  that ends in a spaced piece gets a `dx` of minus half or all of that
  spacing on its first character.
- On macOS the raster step draws two kinds of face itself, through
  CoreText as Skia does: faces whose outlines usvg cannot read (PingFang)
  as `CTFontCreatePathForGlyph` paths in the span's paint, and `sbix`
  color glyphs (Apple Color Emoji) as `CTFontDrawGlyphs` bitmaps at the
  device size, on whole device pixels with the origin rounded to a quarter
  pixel across and a whole pixel down. Each such grapheme is a no-break
  space in the primary face in the SVG, spaced to the measured advance:
  usvg's span merging panics when two faces shape a ZWJ sequence or a
  flag into clusters of different lengths, and resvg drew these faces in
  another face, smaller, or not at all. A first parse lays the text out;
  a second hides the placeholders and draws the glyphs after their text.
  Linux keeps resvg's drawing of every face, through the same tokens.
- Not ported to the drawn glyphs: Chrome's synthetic oblique of a
  fallback face under italic text (the glyphs stay upright), and
  `text-spacing-trim` (`halt`), which the measurer applies to pairs such
  as `」、`; a line with such a pair is drawn up to half an em per pair
  wider than it was measured.

- Ishikawa labels follow DOMPurify's fast path: a label with no `<` is
  kept as written, so `R&D` shows as `R&D`. A label with a tag keeps the
  tags and escapes the text between them (`&` shows as `&amp;`), as the
  HTML parse and serialize in the oracle does.
- A kanban (`mindmap.useMaxWidth: false`) or ishikawa
  (`ishikawa.useMaxWidth: false`) diagram with a fixed size grows its
  frame to whole pixels from the same origin. The browser draws a fixed
  SVG at scale 1 into a clip rounded up to whole pixels, so the helper
  output keeps scale 1 and the oracle height, not the height that
  scaling the width up to whole pixels gives.

- Every diagram parser reads `Resolved::parse_text`: the cleaned code plus
  one newline, as `Diagram.fromText` builds it. Event modeling gets the
  code with comments instead (`preserveCommentsWhenParsing`), unless that
  text is longer than `maxTextSize`. No diagram module appends its own
  newline. The newline sets the line, the excerpt, and the caret of an
  error at the end of the text.
- `encodeEntities` runs only when a parse fails. The renderers parse the
  text without it, because the oracle decodes the entities again in the
  finished SVG. When the first parse fails and encoding changes the text,
  the helper parses the encoded text once more and reports that error, so
  the excerpt shows `ﬂ°°35¶ß` for `#35;` and a `style` line loses its last
  `;`, as in the oracle. If the encoded text parses, the first error
  stays.
- The zenuml detector (`/^\s*zenuml/`, case-sensitive) is the last one,
  because mermaid-cli registers `@mermaid-js/mermaid-zenuml` after the
  built-in diagrams.
- A theme name that mermaid does not register is passed over in
  `resolveAppearance`, so the theme of the next layer applies. An init
  directive with an unknown name replaces the front matter `theme` before
  that lookup, so a front matter `forest` is lost too; the oracle does the
  same. The unknown name still counts as a theme change, so
  `themeVariables` next to it derive a new theme from the base config
  theme.
- A failed CLI command writes its reason on one line: each line break
  becomes the two characters `\n` and each backslash becomes `\\`.
  `printf '%b'` gives the multi-line oracle text back. Every failure exits
  with status 1.

- The shapes that rough.js draws in the classic look draw the same two
  paths: a fill path and a stroke path with every segment twice, each pass
  its own subpath. Chrome and tiny-skia stroke 1 px lines as hairlines
  one segment at a time, so the second pass makes the outline as dark as
  the oracle's. The flowchart stylesheet's `.node path` rule
  (`NodeStyleVars::node_rule`) paints both paths and also gives the fork
  bar and the start and end dots the node colors. Class, ER, requirement,
  note, and choice shapes keep one path for now.
- Flowchart and state diagrams encode entity codes (`#quot;`, `#9829;`)
  before they parse, as `Diagram.fromText` does, and the label measurer
  decodes them. HTML labels then decode character references with a table
  of common names; a name missing from the table shows as typed.
- A node label is an HTML label unless the root `htmlLabels` is false
  (`labelHelper`); `flowchart.htmlLabels: false` turns only edge and
  cluster labels to SVG text. A label is drawn in the mode it was
  measured in.

- On macOS the text measurer picks a face inside a family on the CoreText
  weight trait, bucketed as Blink's `ToCssFontWeight` buckets it, and
  compares faces as Blink's `BetterChoiceCT` does. The OS/2 weight class
  gives Hiragino Sans W3 the weight 300, so the OS/2 rule drew 400 kana in
  W4. W4 has smaller kerning values, and a 175 px kanban card then wrapped
  one character early. Other platforms keep the fontdb CSS match.
- A platform fallback face is the face CoreText gives for the character
  plus the variation of that family that has the requested traits and
  weight. The variation is used only when it has the character
  (`PlatformFallbackFontForCharacter`). A face that fontdb does not list
  (the named instances of the variable PingFang SC) falls back to the
  family match, so PingFang SC text is measured with the Medium instance.
  Its advances are the same as the advances of the Regular instance.
- Fallback faces are picked per script segment. Common characters join the
  segment before them. In each segment the fallback is asked once for one
  hint character, chosen as `ChooseHintIndex` chooses it, and the answer
  draws every leftover character it covers. Shaping runs break at segment
  changes. Script extensions and paired brackets of ICU's script runs are
  not ported.
- `words_width` shapes each run of words of one style as one string.
  Shaping each word alone lost kerning across spaces and between CJK
  characters, which are a word each in a wrapped line.
- `text-spacing-trim: normal` is ported for adjacent pairs only: `halt`
  on a closing mark before a closing or middle mark, and on an opening
  mark after an opening, middle, or closing mark. The trim at the start
  of a wrapped line after the first and the trim at the end of a line
  that overflows are not ported.
- CJK line breaks: a break is allowed between Hangul syllables. A break is
  not allowed before closing marks, the ideographic comma and full stop,
  or `・`, and not after opening marks. A break is allowed before small
  kana and `ー`.

- The text measurer follows Blink's font fallback order on macOS. A
  grapheme is covered by a face when HarfBuzz shapes it there without a
  `.notdef` glyph, so marks, decompositions, and default ignorables count.
  The faces after the `font-family` list are the standard font (Times New
  Roman), then one CoreText answer per hint character, then Times, then
  the `.notdef` of the first face tried. CoreText's LastResort face is not
  used. A face already tried, or a hint asked before, stops the search.
- Emoji-presentation graphemes (also an emoji modifier base alone, and a
  grapheme with a skin tone modifier) are segments of their own. Katakana
  and hiragana are one script. Private-use characters and noncharacters
  are a script of their own. Unassigned code points are not detected.
- The page's Open Sans is a segmented web font. `helper/src/text/open_sans.rs`
  holds the declared `unicode-range` and the drawn characters of each
  `@fontsource/open-sans` 5.3.0 subset. To regenerate it, run
  `gen_subsets.py` from the QA round 2 scratch folder with fontTools on
  the `.woff` files of that package. Mark stacks compose as HarfBuzz
  composes them, with a small canonical combining class table for Latin.
- CoreText maps some characters that a `cmap` lacks to another glyph
  (U+2010 and U+2011 to the hyphen, U+202F to the thin space). The
  measurer copies that list. It sets the `opsz` axis of a variable face to
  the font size, as CoreText does.
- The advance and the ink box of an Apple Color Emoji glyph come from a
  table of the oracle's values for 6 to 64 px. Chrome's values agree with
  neither the font tables nor CoreText.
- An SVG `<text>` collapses spaces, tabs, and line breaks like CSS
  `white-space: normal`. Other Unicode spaces and NBSP are text.
- HTML labels break between a closing and an opening bracket, before and
  after an em dash, after an ellipsis, U+2012, and a zero-width space,
  and after a hyphen at the start of a word unless a digit follows.
- Not ported: CoreText's synthetic bold of a fallback face, which widens
  each glyph by 0.0576 em (bold Syriac); the named weight instances of
  variable fallback faces (Kefa Bold); fonts that CoreText loads on
  demand and fontdb does not list (Songti SC for C1 controls); leading
  combining marks in Menlo.
- To rerun the wide sweep: `node helper/oracle/measure.mjs --sweep
  <out.json>`, then `KITTY_MARKDOWN_SWEEP=<out.json> cargo test --test
  text_metrics sweep -- --ignored --nocapture` in `helper/`. Set
  `KITTY_MARKDOWN_SWEEP_CLI=1` to include the `measure` CLI route. The
  test writes `<out.json>.tsv` and `<out.json>.faces.txt`.
