# Open questions: math, theme, test hooks

- R-203: Is `opts.width` the preview window width (so the text width is `width - 4`, capped at 100) or the text width itself? The checks accept both readings.
- R-156, R-203: The spec does not name the fields of `opts.cell` and of each `opts.sizes` entry (for example `width_px` and `height_px`, as in the block descriptor). Name them in the spec so a caller can rely on them.
- R-204, R-155: `opts.sizes` is keyed by the block key, but R-155 does not list a descriptor field that carries the block key, and the Definitions do not give the field order or separator of the hashed string. Should R-155 add a `key` field to the descriptor?
- R-156: When a block is wider than the target width, are the reserved rows computed from the scaled height or the natural height?
- R-154: Are highlight and link columns byte offsets or display cells, and are lines and columns 0-based or 1-based? The checks accept both bases.
- R-069, R-063: Is a nested fraction (or another structure beyond a "simple fraction") something the plugin may approximate, or must it fall back to the source?
- R-065, R-066: When a superscript or subscript holds a letter that has no Unicode script form, must the whole span fall back to the source (R-069), or may the plugin keep that one script in TeX form?
- R-220, R-221: When a span falls back to the source, do the whitespace rules of R-221 (collapse runs, trim the ends) apply to the source, or is the source shown exactly as written between the delimiters?
- R-062: Does `$` followed by a space, or two prices on one line, open inline math? GitHub requires a non-space character after the opening `$` and before the closing `$`.
- R-075, R-076: Is a user-defined `KittyMarkdown*` group kept when `&background` changes and the plugin re-themes, and when the user runs `:colorscheme` (which clears highlights)?
- R-071: Which revision of the GitHub Primer palettes is the reference? GitHub has changed the dark palette text and link colors over time, so the checks test only palette properties (contrast, blue links, distinct muted color).
- R-208: Does a mermaid fence, an empty fence, or an indented code block get a code region?
- R-206: GitHub resolves a slug collision between a duplicate suffix and a real heading (a heading whose own slug equals the suffixed slug of an earlier duplicate heading) by adding another suffix. Confirm that the layout must follow the same rule.
