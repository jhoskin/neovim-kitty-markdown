# Open questions: commands, refresh, interaction

- R-217, R-259: for two pixel blocks in a row, does the anchor line of the second block take the place of the one blank line that R-259 keeps after the first block, or does that blank line come before the second anchor line? The checks only require that no more than one blank line separates the two anchor lines.
- R-252: a tab preview is always the last window of its tab page, so closing it keeps an empty tab page open on another buffer. Is that intended, or should the rule apply only when the preview window is the last window of the editor?
- R-255: Neovim does not let two buffers share a name, so two source buffers without names cannot both have the preview name `kitty-markdown://`. What name does the second preview get?
- R-023, R-256: should the 200 ms include the first load of the plugin modules after Neovim starts, or only an open with the plugin already loaded? The checks warm the plugin up once before they time the open.
