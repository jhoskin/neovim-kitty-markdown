# Every Feature

## Inline styles

Plain text with *emphasis*, _also emphasis_, **strong**, __also strong__,
***both at once***, ~~struck out~~, and `inline code`. A line can end with
two spaces for a hard break  
like this one, or with a backslash\
like this one.

Escapes: \*not emphasis\*, \_not either\_, \# no heading,
and a literal backslash \\ here.

Emoji: :tada: :+1: :sparkles: and an unknown :not_an_emoji_code: stays.

## Links

An [inline link](https://example.org "Title"), a [reference link][ref],
a collapsed [ref][], a shortcut [ref], an autolink <https://auto.example>,
and a bare URL https://bare.example/path. Also www.example.net.

[ref]: https://ref.example/target

### Heading level three

#### Heading level four

##### Heading level five

###### Heading level six

Setext level one
================

Setext level two
----------------

## Lists

- First bullet
- Second bullet with a longer text that has to wrap when the window is
  narrow enough to force it
  - Nested bullet
    - Deeper bullet
- Third bullet

* Star bullet
+ Plus bullet

3. Third
4. Fourth
5. Fifth

1) Paren one
2) Paren two

- [x] Checked task
- [ ] Open task
  - [ ] Nested open task

## Table

| Left | Center | Right |
|:-----|:------:|------:|
| a    | b      | c     |
| 漢字 | 😀     | `x`   |
| **s** | [l](http://t.example) | 3 |

## Code

```lua
local function add(a, b)
  return a + b
end
```

~~~
no language
~~~

```nosuchlanguage
literal **text** stays
```

## Quotes and alerts

> A quote with *style*
> over two lines.
>
> > A nested quote.

> [!TIP]
> A tip.

> [!IMPORTANT]
> Important text.

> [!WARNING]
> A warning.

> [!CAUTION]
> Be careful.

## Footnotes

A claim[^one] and another[^two] and again[^one]. An undefined[^ghost] one.

[^two]: The second note.
[^one]: The first note.
[^unused]: Nobody points here.

## HTML

<details>
<summary>More</summary>
Hidden text.
</details>

Inline <kbd>Ctrl</kbd>+<kbd>C</kbd> keys.

***

The end.
