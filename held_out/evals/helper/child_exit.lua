--# selene: allow(global_usage, incorrect_standard_library_use)
-- Child editor for the exit checks: starts the plugin's helper through the
-- helper path option, prints READY to stderr, and then ends the script, so
-- Neovim exits the normal way (VimLeavePre runs).
--
-- Usage: nvim --clean -l child_exit.lua <plugin root> <helper path>

local root, helper_path = _G.arg[1], _G.arg[2]
vim.opt.runtimepath:prepend(root)
vim.cmd('runtime plugin/kitty-markdown.lua')

require('kitty-markdown').setup({ helper = { path = helper_path, download = false } })
local helper = require('kitty-markdown.helper')
if type(helper.ensure) ~= 'function' then
  io.stderr:write('NO_ENSURE\n')
  return
end

local done, err = false, nil
helper.ensure(function(e)
  done, err = true, e
end)
vim.wait(8000, function()
  return done
end, 10)
if not done or err then
  io.stderr:write('NOT_READY ' .. vim.inspect(err) .. '\n')
  return
end
io.stderr:write('READY\n')
