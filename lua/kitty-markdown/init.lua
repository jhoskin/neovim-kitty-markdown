-- Public API: setup, open, close, toggle, refresh, layout (R-013).

local M = {}

-- TermResponse delivers the kitty graphics and DA1 replies only from 0.12,
-- so older Neovim cannot run the probe at all (R-166).
local function supported()
  if vim.fn.has('nvim-0.12') == 1 then
    return true
  end
  vim.notify('kitty-markdown: Neovim 0.12 or later is required', vim.log.levels.ERROR)
  return false
end

function M.setup(opts)
  require('kitty-markdown.config').setup(opts)
  require('kitty-markdown.theme').apply()
end

function M.open(placement)
  if not supported() then
    return
  end
  return require('kitty-markdown.preview').open(vim.api.nvim_get_current_buf(), placement)
end

function M.close()
  if not supported() then
    return
  end
  return require('kitty-markdown.preview').close(vim.api.nvim_get_current_buf())
end

function M.toggle(placement)
  if not supported() then
    return
  end
  return require('kitty-markdown.preview').toggle(vim.api.nvim_get_current_buf(), placement)
end

function M.refresh()
  if not supported() then
    return
  end
  return require('kitty-markdown.preview').render(
    vim.api.nvim_get_current_buf(),
    { reprobe = true }
  )
end

--- The layout model for a buffer, without a terminal, a preview, or a
--- helper (R-153). `opts` keys: width, cell, sizes, theme, images, dir.
function M.layout(bufnr, opts)
  local parser = require('kitty-markdown.parser')
  local layout = require('kitty-markdown.layout')
  return layout.build(parser.parse(bufnr or vim.api.nvim_get_current_buf()), opts or {})
end

return M
