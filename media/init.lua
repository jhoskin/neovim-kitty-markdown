-- Neovim config for the README media. media/record.sh runs it with
-- `nvim --clean -u`. With KMD_DEMO_SCRIPT=1 it types a short edit session.
local root = vim.fn.fnamemodify(debug.getinfo(1, 'S').source:sub(2), ':p:h:h')
vim.opt.rtp:prepend(root)
vim.o.laststatus = 0
vim.o.ruler = false
vim.o.showmode = false
vim.o.number = true
vim.o.hlsearch = false

require('kitty-markdown').setup({
  helper = {
    path = root .. '/helper/target/release/kitty-markdown-helper',
    download = false,
  },
})

-- Each step is a delay in ms before it, the keys, and the delay per key.
local steps = {
  { 3000, ':16\r', 60 },
  { 300, 'o- [x] Re-renders on every save', 70 },
  { 300, '\27:w\r', 120 },
  { 2500, '/kitty graphics]\r', 60 },
  { 300, 'o  F --> G[Preview window]', 70 },
  { 300, '\27:w\r', 120 },
  { 3000, 'G', 0 },
}

local function run(i)
  local step = steps[i]
  if not step then
    return
  end
  vim.defer_fn(function()
    local keys = vim.split(step[2], '')
    local function feed(k)
      if k > #keys then
        return run(i + 1)
      end
      vim.api.nvim_feedkeys(keys[k], 'n', false)
      vim.defer_fn(function()
        feed(k + 1)
      end, step[3])
    end
    feed(1)
  end, step[1])
end

vim.api.nvim_create_autocmd('VimEnter', {
  callback = function()
    vim.defer_fn(function()
      vim.cmd('KittyMarkdown open split')
      if vim.env.KMD_DEMO_SCRIPT == '1' then
        run(1)
      end
    end, 300)
  end,
})
