# Open questions: text rendering

- R-273 says later paragraphs and blocks of a footnote definition are "indented under its number". Should they line up with the text after the number, or sit a fixed number of cells to the right of the number? The checks require only that they start to the right of the number.
- R-055 asks for "the border color of the palette", and R-071 names the GitHub light and dark markdown palettes, but the spec gives no color values. Should the spec list the palette colors (text, muted, border, link, code background, alert colors), so that a check does not depend on which Primer release is meant?
- R-042 says the checkbox replaces "the bullet". For a task item in an ordered list, should the checkbox also replace the number, or come after it?
- R-270 does not say what happens to the spaces around a hidden inline HTML comment. Should the two spaces collapse to one?
