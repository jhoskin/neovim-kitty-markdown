# Open questions: text rendering

- R-043 says "borders around every cell". Does that require a horizontal border between consecutive body rows, or only above the header, below the header, and below the last row? The checks read it literally (a border between every row).
- R-043 to R-045 do not say what happens when a table is wider than the text width. Should the widest column wrap inside its cells, should the table scroll or overflow, or should the cells be truncated?
- R-047 to R-050 do not say what happens to a code line longer than the text width: wrap, truncate, or overflow the text width?
- R-030 covers only words shorter than the text width. Should a longer word (for example a long URL) be split at the text width, or overflow it?
- R-037 does not say whether the angle brackets of an autolink are shown. The checks treat them as markdown markers under R-171 and expect them hidden.
- R-041 does not say whether the 2-cell indent is measured from the parent's marker or from the parent's text. The checks accept both.
- R-051 does not say how a nested blockquote looks (one bar per level, or one bar with a deeper indent).
- R-052 does not give the alert title text or casing (for example "Note" or "NOTE"), and does not say whether the kind word is case-insensitive in the source, as GitHub treats it.
- R-055 says the thematic break uses "the muted color". Is that the same color as muted HTML text (R-060) and failure reasons, or may a separate border color be used?
- R-059 does not say whether front matter closed with `...` (the YAML document end marker) is hidden, or whether TOML (`+++`) front matter is hidden.
- R-060 does not say whether an HTML comment is shown as muted text (as the requirement reads) or hidden (as GitHub does). It also does not say whether the text between inline tags is muted or only the tags.
- R-042 does not say whether an uppercase `[X]` marks an item checked (GFM accepts both cases).
- R-054 does not say how a footnote definition with several paragraphs or a code block is laid out at the end of the document.
- R-027 names no icons, and R-039 and R-042 name no glyphs. Should these be listed, so a user with a different font can predict them?
