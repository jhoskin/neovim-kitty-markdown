# Open questions: math, theme, test hooks

- R-283: The palette groups may leave the background unset so that the terminal background shows. Is "the background color of that palette" then the GitHub page background (white for light, the GitHub dark canvas for dark)? The checks use that reading when no group sets a background.
- R-282: Does R-282 apply only while a preview is open, or from `setup()` on, before any preview has opened?
- R-339: Is "no language" a missing `lang` field (nil), or may it be an empty string? The checks accept both.
- Block key: Is the resolved path normalized before hashing, so that a relative path with a leading `./` and the same path without it give the same key?
- R-277: Unicode 14 added modifier letters for some characters that had no script form before (for example a superscript small q). Which Unicode version decides whether a character "has a Unicode script form"?
