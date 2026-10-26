# Open questions: mermaid diagrams

- R-098 says the helper renders at the target width, and R-114 and R-180 say a narrower diagram renders at its own or its natural width. The checks read the narrower width as the winner. Please state that order in R-098.
- A gantt chart takes its width from its container, so it has no natural size apart from the target width. Does R-180 apply to gantt at all, and if it does, what is the natural size of a gantt chart?
- The Non-goals say the `neo`, `neo-dark`, and `redux*` themes render as a failure box, but R-234 says a theme outside the five built-in themes is ignored. Which one applies to those six names? The checks test R-234 only with a name that is in neither list.
- R-174 puts the base config before the directives and the front matter, but no requirement gives the order between an init directive and front matter when both set the same key. Mermaid lets the directive win. Should the spec say so?
- R-112 and R-175 give the reason as "unsupported: `<feature>`" but do not list the feature strings, and do not say if the backticks are part of the text. The checks only require the prefix "unsupported: " and a reason that names the feature.
- R-231 does not say if `<text>` is the whole diagram text (with line breaks) or only its first line, or how a long text is shortened in `:messages`. The checks look only for the message with the start of the text.
- R-176 names zenuml, but the pinned oracle does not register zenuml, so the oracle reports "No diagram type detected" for it. The checks follow R-176 ("unsupported diagram type"). Please confirm that the spec means to differ from the oracle here.
- Mermaid has several railroad variants (`railroad-beta` and the EBNF, ABNF, and PEG forms). Does R-176 cover all of them?
- R-179 asks for the bundled Open Sans before the system fonts. On a machine without a system Open Sans, a black-box check can only show that the bundled font is used, not the load order. Is a static or unit check of the font database order the intended test?
- R-116 puts the mermaid version in the cache key, but a black-box check cannot change the pinned version. Is the key string of R-213 the intended test for that part?
- R-235 says the helper "shall not derive a new theme". The oracle merges a theme variable and keeps the colors that the theme derived from the old values. Please confirm that this is what "not derive" means, and not "do not switch to the base theme".
- R-111 asks for "the diagram type" in the failure box. The spec does not say if that is the keyword the diagram starts with (for example `sequenceDiagram`) or the type name (`sequence`). The checks accept either.
