# Open questions: images and cache

- R-088 says an SVG is rasterized "at the target width", and R-091 says an image is never scaled up past its native pixel size. For an SVG whose declared size is narrower than the target width, which rule wins: the target width or the declared size?
- R-092: when an `<img>` tag gives only a width or only a height, does the other dimension follow the aspect ratio, or does it stay at the computed size?
- R-152: for a remote image, which paths count as "the paths that open previews use"? The prepared PNG, the fetched file in `remote`, or both?
- R-225: is a `file://` URL a scheme other than `http` or `https` (so "fetch failed"), or a local path?
- R-096: what must the "full error" in `:messages` hold? For example, the file path or URL, the HTTP status, the decoder detail.
- R-082 and R-083: the spec names a "configured timeout" and a "configured size cap" but gives no option names and no default for the timeout. Does "20 MB" mean 20 * 1024 * 1024 bytes or 20,000,000 bytes?
- R-084 and R-173: when a fetch fails, must a later render try the fetch again, or may the failure stay until a refresh?
- R-083: may the helper keep a partial or oversize remote body in the `remote` cache, or must it store nothing?
- R-202: does "request garbage collection once" mean once per render batch, or once until the estimate drops under the limit again?
- R-213: is the diagram source hash taken over the fence body with or without its final newline?
