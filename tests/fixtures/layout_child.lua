-- Child process for the R-209 tests: `nvim --clean -l layout_child.lua
-- <plugin root> <runtimepath> <fixture>...` sets 'runtimepath' to exactly
-- the given value, builds the layout model of each fixture with fixed
-- options, and prints JSON: `models` holds one `vim.inspect` dump per
-- fixture, `parsers` the parser libraries left on the runtimepath, so the
-- test can check that the setup removed what it meant to remove.

local root, rtp = arg[1], arg[2]
vim.o.runtimepath = rtp
package.path = root .. '/lua/?.lua;' .. root .. '/lua/?/init.lua;' .. package.path

local parser = require('kitty-markdown.parser')
local layout = require('kitty-markdown.layout')

local out = {}
for i = 3, #arg do
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, vim.fn.readfile(arg[i]))
  local model = layout.build(parser.parse(buf), { width = 44, theme = 'light', dir = '/fixtures' })
  out[#out + 1] = vim.inspect(model)
end
local libs = vim.api.nvim_get_runtime_file('parser/*', true)
io.write(vim.json.encode({ models = out, parsers = libs }))
