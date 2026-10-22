-- Nerd Font glyphs used by the layout. One place, so a user with another
-- font can override the whole set through this table.

local M = {
  -- nf-md-numeric_<n>_box_multiple_outline, H1 to H6
  h = { '󰲡', '󰲣', '󰲥', '󰲧', '󰲩', '󰲫' },
  -- bullets by nesting depth; the list repeats past the last entry
  bullet = { '●', '○', '◆', '◇' },
  checked = '󰱒',
  unchecked = '󰄱',
  alert = {
    NOTE = '󰋽',
    TIP = '󰌶',
    IMPORTANT = '󰅾',
    WARNING = '󰀪',
    CAUTION = '󰳦',
  },
  loading = '󰔟',
  failure = '󰅖',
  quote = '▎',
  rule = '─',
}

return M
