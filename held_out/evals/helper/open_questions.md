# Open questions: helper lifecycle

- R-196: Does `:KittyMarkdown refresh` clear the restart stop when no preview is open in the current buffer, or only when it refreshes a preview?
- R-243: Does a helper path option that names a file that does not exist count as "not executable" (one `WARN`, no download)? After that `WARN`, should the plugin try the local build, or go straight to text-only mode?
- R-195: A helper exit is reported with one `WARN`. May the preview also report each pixel block that failed with "helper exited" at `ERROR` level, or should the exit produce only the one `WARN`?
- R-247: Is a line that is valid JSON but has a `method` that is not a string a line with "no `method`"? Should the helper answer an empty line? When a line is not valid JSON but an id can be read from its start, should the answer use that id or `id` 0?
- R-139: The spec does not give the type of `id`. Must the helper accept string ids, and must it echo an id of any JSON type unchanged?
- R-142 and R-147: The spec does not name the probe result fields (helper version) or the result fields for the cache path and the pixel size. Name them, so that a second helper or client can follow the protocol from the spec alone.
- R-199: The spec does not give the output format of `compare` (decimal places, extra text) or what happens for two images of different pixel sizes (error text and exit status).
- R-146: Does the stderr and non-zero exit rule apply to `compare` failures too (for example a missing or unreadable file), or only to `render`?
- R-141: The request timeout is 30 s in the status table. Is it a user option, and if so, what is its name and range?
- R-192: On a platform outside the three targets, the spec implies R-133 (use the local build). Should the plugin skip the download without a message, or show a `WARN`?
