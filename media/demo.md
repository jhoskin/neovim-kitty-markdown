# Release notes

The preview renders **bold**, *italic*, `code`, ~~strikethrough~~, and
[links](https://github.com/jhoskin/neovim-kitty-markdown) in terminal
cells, with GitHub's palette.

> [!NOTE]
> Images and diagrams are real pixels, drawn through the kitty graphics
> protocol.

![Banner](banner.png)

## Checklist

- [x] Tables, alerts, and footnotes[^1]
- [x] Inline math such as $x^2 + y^2 = r^2$
- [ ] World domination

| Feature  | Text | Pixels |
| -------- | :--: | -----: |
| Markdown |  yes |     no |
| Images   |   no |    yes |
| Mermaid  |   no |    yes |

## Pipeline

```mermaid
flowchart LR
  A[Buffer] --> B(Tree-sitter)
  B --> C{Block kind}
  C -->|text| D[Cells]
  C -->|image| E[Rust helper]
  E --> F[kitty graphics]
```

```lua
require('kitty-markdown').setup({ placement = 'split' })
```

[^1]: Footnotes go at the end of the document.
