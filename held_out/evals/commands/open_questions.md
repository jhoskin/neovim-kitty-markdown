# Open questions: commands, refresh, interaction

- R-001, R-003: the spec does not say what the plugin does for `:KittyMarkdown` with no subcommand, for an unknown subcommand, for a placement argument that is not `split`, `tab`, or `float`, or for a placement argument after `close` or `refresh`. Should each of these show an error message and do nothing? If so, what is the message text?
- R-011: when the preview is the last window in its tabpage and the source buffer is deleted, the window cannot close. Is it enough to remove the preview buffer (the window then shows another buffer), or must the plugin do something else?
- R-015: "hide line numbers" does not name `relativenumber`. The checks read it as both absolute and relative numbers. Please confirm.
- R-160: the spec does not say where a relative file link or image path is resolved from. The checks read it as the directory of the source file, not the current working directory. Please confirm.
- R-162, R-248: which block is "under the source cursor" when the cursor is on a blank line between two blocks, or inside front matter that the preview hides?
- R-163: does the debounce allow a scroll at the start of a burst of cursor movement (leading edge), or only after the movement pauses (trailing edge)? Both meet "at most one scroll per 100 ms".
- R-167: the spec does not say whether `<source buffer name>` is the full path or the name as `:ls` shows it, or what the name is for a source buffer that has no name.
- R-211: "the top third of the window" does not say how to round when the window height is not a multiple of three. The checks accept a target line whose row starts inside the top third.
- R-217: the spec does not say what happens for a pixel block at the very top of the document, where there is no blank line before it, or whether a blank line follows a pixel block that ends the document.
- R-023, R-024: the content mix of the 1000-line document, the mermaid diagram size, the machine, and whether a cache hit counts are not given. The checks use a mixed document and small uncached diagrams on the QA machine.
