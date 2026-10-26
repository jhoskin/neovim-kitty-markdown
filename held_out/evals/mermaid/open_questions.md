# Open questions: mermaid diagrams

- R-297 leaves out directives and `%%` comments from the `<text>` of R-231, but it does not say what happens to the line break of a removed line. The oracle keeps one line break where a directive at the start was. The checks accept that line break at the start of the text. Should R-297 say whether the line breaks of removed lines stay?
- R-296 lists `randomize`, `participant type <kind>`, and `timeline direction TD` as features, but no requirement says which diagrams are refused for them. The checks test only the form of the reason when the helper refuses such a diagram. Should a requirement name those cases?
- R-112 lists ELK layout, and mermaid also selects ELK through the `flowchart-elk` diagram keyword. The checks expect the feature `layout: elk` for that keyword. Please confirm that the keyword counts as ELK layout and that `layout: elk` is its feature.
- R-330 joins the lines of the fence body without a final line feed. It does not say if empty lines at the end of the body, just before the closing fence, are part of the body. Should those lines count?
