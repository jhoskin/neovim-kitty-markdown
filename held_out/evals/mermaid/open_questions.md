# Open questions: mermaid diagrams

- R-296 lists `randomize`, `participant type <kind>`, and `timeline direction TD` as features, but no requirement says which diagrams are refused for them. The checks test only the form of the reason when the helper refuses such a diagram. Should a requirement name those cases?
- R-112 lists ELK layout, and mermaid also selects ELK through the `flowchart-elk` diagram keyword. The checks expect the feature `layout: elk` for that keyword. Please confirm that the keyword counts as ELK layout and that `layout: elk` is its feature.
- R-330 joins the lines of the fence body without a final line feed. It does not say if empty lines at the end of the body, just before the closing fence, are part of the body. Should those lines count?
- R-297 now keeps one line break for each removed front matter line, but the pinned mermaid 12.2.0 drops the front matter lines with no line break, and R-231 otherwise follows the oracle text. The checks follow R-297 as written. Please confirm that this difference from the oracle is intended.
