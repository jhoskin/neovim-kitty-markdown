---
title: Widget Toolkit
tags: [cli, tools]
---

# Widget Toolkit :rocket:

Widget Toolkit is a small command line tool that turns plain text
widget specs into rendered widgets. It runs on macOS and Linux, and it
has **no runtime dependencies** beyond a POSIX shell.

> [!NOTE]
> The toolkit is in early development. Expect breaking changes before
> the first stable release.

## Install

Download a release from <https://example.com/releases> or build it
from source with `make install`. See the [build guide](docs/BUILD.md)
for details.

```sh
git clone https://example.com/widget.git
cd widget
make install
```

## Usage

1. Write a spec file.
2. Run `widget render spec.txt`.
3. Open the output in your browser.

Options:

- `--width N` sets the output width.
- `--theme NAME` picks a theme:
  - `light` (default)
  - `dark`
- `--quiet` hides progress output.

## Status

| Feature   | State | Since |
|:----------|:-----:|------:|
| Render    | done  | 0.1   |
| Themes    | beta  | 0.3   |
| Plugins   | todo  | -     |

## Roadmap

- [x] Basic rendering
- [x] Theme support
- [ ] Plugin system
- [ ] Windows support

---

Made with care by the widget team[^team]. Questions go to
someone@example.com or the issue tracker at https://example.com/issues.

[^team]: A group of three part-time maintainers.
