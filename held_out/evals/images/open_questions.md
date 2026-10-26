# Open questions: images and cache

- R-088: what is the "declared width" of an SVG that has a `viewBox` but no `width` attribute, or a `width` in a unit other than pixels (for example `em` or `%`)?
- R-329: is a render that sends no request to the helper (every pixel block is already placed) a "render batch"? If it is, must the plugin still request garbage collection while the estimate exceeds the limit?
- R-288: the spec does not list the values of `<code>`. Are the codes part of the contract (so a test may expect a given code for a given failure), or only a single word without spaces or colons?
- R-291: which events count as "a render" for the retry of a failed fetch: only `:KittyMarkdown refresh` and a save of the source buffer, or also a theme change or a window resize?
