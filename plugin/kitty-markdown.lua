-- :KittyMarkdown command (R-001, R-003). Autocommands are registered by
-- the preview module when the first preview opens, so loading this file
-- does no work beyond defining the command.

if vim.g.loaded_kitty_markdown then
  return
end
vim.g.loaded_kitty_markdown = true

local subcommands = { 'open', 'close', 'toggle', 'refresh' }
local placements = { 'split', 'tab', 'float' }
local takes_placement = { open = true, toggle = true }

local function complete(arglead, line)
  local words = vim.split(line, '%s+', { trimempty = true })
  -- words[1] is the command itself; a trailing space means a new argument.
  local at = #words + (line:match('%s$') and 1 or 0)
  local candidates = {}
  if at <= 2 then
    candidates = subcommands
  elseif at == 3 and takes_placement[words[2]] then
    candidates = placements
  end
  return vim.tbl_filter(function(c)
    return c:sub(1, #arglead) == arglead
  end, candidates)
end

vim.api.nvim_create_user_command('KittyMarkdown', function(cmd)
  local sub, placement = cmd.fargs[1], cmd.fargs[2]
  if not vim.tbl_contains(subcommands, sub) then
    vim.notify(
      string.format('kitty-markdown: unknown subcommand %s', vim.inspect(sub)),
      vim.log.levels.ERROR
    )
    return
  end
  -- The first argument no subcommand accepts is the one the error names:
  -- a bad placement before a third argument (R-250).
  local unexpected = nil
  if placement and not (takes_placement[sub] and vim.tbl_contains(placements, placement)) then
    unexpected = placement
  elseif cmd.fargs[3] then
    unexpected = cmd.fargs[3]
  end
  if unexpected then
    vim.notify(
      string.format('kitty-markdown: unexpected argument %s', vim.inspect(unexpected)),
      vim.log.levels.ERROR
    )
    return
  end
  require('kitty-markdown')[sub](placement)
end, {
  nargs = '+',
  complete = complete,
  desc = 'kitty-markdown preview: open|close|toggle|refresh [split|tab|float]',
})
