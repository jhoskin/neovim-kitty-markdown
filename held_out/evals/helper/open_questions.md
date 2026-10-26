# Open questions: helper lifecycle

- R-318 and R-247: A request has a string `method` but no `id` field at all. Should the helper answer it with `invalid_params` and `id` 0, or run it and answer with `id` 0?
- R-320: What does `bytes` count: the size of the PNG file at `path`, or something else? Is `cached` true exactly when the helper served the result from the cache without writing a new file?
- R-322: When two images differ by 1 pixel on an axis, the center crop must drop one row or column from one side. Which side drops it?
- R-325: What should the helper write to stderr for invalid CLI arguments? The R-324 one-line form, a usage text, or nothing?
- R-326: What should happen when `helper.request_timeout_ms` is not a positive integer: an error from `setup`, or a `WARN` and the default?
- R-324: A mermaid parse error has a caret line and an expected-token line in the oracle's form. Must the CLI join that error into the one `kitty-markdown-helper: <reason>` line, and if so, how should it join the lines?
