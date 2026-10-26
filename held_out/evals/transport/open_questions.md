# Transport open questions

- R-184 and R-306: the zero pixel size reason `cell size unknown` is given when the probe answers. If the terminal later stops reporting a pixel size (for example after a resize or a reattach) without a new probe, must the preview switch to text-only mode and write the R-306 message then, or only at the next probe?
