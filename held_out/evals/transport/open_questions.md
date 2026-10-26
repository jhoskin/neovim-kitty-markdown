# Transport open questions

- R-123: the quiet flag `q=2` is required, but the text does not say if every chunk of a chunked transmission must carry it or only the first chunk. kitty sends its reply after the last chunk, so the flag on the last chunk may be the one that counts. Should every chunk carry `q=2`?
- R-130 and R-017: R-130 says the refresh command re-transmits "every placed image", and R-017 says "the preview" (singular) and "all images". When more than one preview is open, does a refresh in one preview re-transmit the images of the other previews too? The checks read R-130 as all open previews.
- R-186: the start id is "picked per Neovim instance". Must two instances pick different start values (for example a random start), or is any fixed value between 1 and 2^23 compliant?
- R-239: the message must give "the new mode and its reason". Text-only mode has the four R-240 reasons, but image mode has no listed reason. What must the image mode message say, and is a fixed wording expected for either mode?
- R-118 and R-131: if the graphics reply arrives after the 500 ms timeout has already put the preview in text-only mode, must the plugin switch to image mode, or does the late reply count for nothing until the next refresh?
- R-122: the DA1 request is not a graphics sequence. Should it go through the tmux passthrough as well, or must it be sent bare so that tmux answers it?
- R-128: "delete its image by id" does not say whether the delete also frees the image data in the terminal (kitty `d=I`) or only removes the placements (`d=i`). Which one is required?
- R-240: a graphics reply that carries an error message instead of `OK` shows that the terminal speaks the protocol. Does that reply count as an answer that turns images on, or as `no graphics reply`?
