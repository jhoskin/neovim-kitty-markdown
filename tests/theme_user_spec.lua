-- User highlight groups and colorschemes against the theme (R-076, R-281,
-- R-282). These run the real `:colorscheme`, `:highlight`, and
-- &background paths, and open real previews.

local km = require('kitty-markdown')
local config = require('kitty-markdown.config')
local theme = require('kitty-markdown.theme')
local transport = require('kitty-markdown.transport')
local helper = require('kitty-markdown.helper')
local preview = require('kitty-markdown.preview')

local LIGHT_FG = 0x1f2328
local DARK_FG = 0xf0f6fc

local function hl(group)
  return vim.api.nvim_get_hl(0, { name = group })
end

describe('theme and user groups', function()
  local tmp, rtp_before, notify_before

  -- A colorscheme file under a temporary runtimepath entry.
  local function colorscheme(name, lines)
    vim.fn.mkdir(tmp .. '/colors', 'p')
    vim.fn.writefile(lines, tmp .. '/colors/' .. name .. '.vim')
  end

  local function md_buf(name)
    local path = tmp .. '/' .. name .. '.md'
    vim.fn.writefile({ '# Title', '', 'Some text.' }, path)
    local buf = vim.fn.bufadd(path)
    vim.fn.bufload(buf)
    vim.bo[buf].filetype = 'markdown'
    vim.api.nvim_win_set_buf(0, buf)
    return buf
  end

  before_each(function()
    tmp = vim.fn.tempname()
    vim.fn.mkdir(tmp, 'p')
    tmp = vim.uv.fs_realpath(tmp)
    rtp_before = vim.o.runtimepath
    vim.opt.runtimepath:prepend(tmp)
    vim.o.termguicolors = true
    vim.o.background = 'light'
    vim.cmd('hi clear')
    theme.reset()
    preview.reset()
    helper.reset()
    transport.reset()
    transport.set_writer(function() end)
    transport.set_winsize_reader(function()
      return 24, 80, 1600, 480
    end)
    helper.set_locator(function(cb)
      cb(nil)
    end)
    notify_before = vim.notify
    vim.notify = function() end
    config.setup({ placement = 'split' })
  end)

  after_each(function()
    preview.reset()
    helper.reset()
    transport.reset()
    vim.notify = notify_before
    vim.cmd('silent! tabonly')
    vim.cmd('silent! only')
    for _, buf in ipairs(vim.api.nvim_list_bufs()) do
      if vim.api.nvim_buf_get_name(buf):find(tmp, 1, true) then
        pcall(vim.api.nvim_buf_delete, buf, { force = true })
      end
    end
    vim.o.runtimepath = rtp_before
    vim.g.colors_name = nil
    vim.cmd('hi clear')
    theme.reset()
    vim.o.background = 'light'
    vim.fn.delete(tmp, 'rf')
    config.setup({})
  end)

  describe('a user group set after setup (R-076, R-281)', function()
    it('is kept when the preview opens', function()
      km.setup({ placement = 'split' })
      vim.api.nvim_set_hl(0, 'KittyMarkdownH1', { fg = '#123456' })
      vim.cmd('highlight KittyMarkdownText guifg=#654321')
      assert.is_not_nil(preview.open(md_buf('a'), 'split'))
      assert.are.equal(0x123456, hl('KittyMarkdownH1').fg)
      assert.are.equal(0x654321, hl('KittyMarkdownText').fg)
      assert.are.equal(LIGHT_FG, hl('KittyMarkdownH2').fg)
    end)

    it('is kept across a second setup call', function()
      km.setup({})
      vim.api.nvim_set_hl(0, 'KittyMarkdownLink', { fg = '#abcdef', bold = true })
      km.setup({ theme = 'dark' })
      assert.are.same(
        { fg = 0xabcdef, bold = true, cterm = { bold = true } },
        hl('KittyMarkdownLink')
      )
      assert.are.equal(DARK_FG, hl('KittyMarkdownH1').fg)
    end)

    it('is kept when the user group is a link', function()
      km.setup({})
      vim.api.nvim_set_hl(0, 'KittyMarkdownQuote', { link = 'Comment' })
      vim.cmd('highlight! link KittyMarkdownRule NonText')
      vim.o.background = 'dark'
      theme.apply()
      km.setup({})
      assert.are.same({ link = 'Comment' }, hl('KittyMarkdownQuote'))
      -- `:highlight! link` keeps the old attributes next to the link.
      assert.are.equal('NonText', hl('KittyMarkdownRule').link)
    end)

    it('is kept when the user changes only one attribute', function()
      km.setup({})
      vim.cmd('highlight KittyMarkdownH3 gui=italic')
      km.setup({})
      assert.is_true(hl('KittyMarkdownH3').italic)
    end)
  end)

  describe('a user group set while a preview is open (R-281)', function()
    it('survives a background change', function()
      km.setup({ placement = 'split' })
      local buf = md_buf('a')
      assert.is_not_nil(preview.open(buf, 'split'))
      vim.api.nvim_set_hl(0, 'KittyMarkdownH1', { fg = '#123456' })
      vim.o.background = 'dark'
      assert.are.equal(0x123456, hl('KittyMarkdownH1').fg)
      assert.are.equal(DARK_FG, hl('KittyMarkdownH2').fg)
      vim.o.background = 'light'
      assert.are.equal(0x123456, hl('KittyMarkdownH1').fg)
      assert.are.equal(LIGHT_FG, hl('KittyMarkdownH2').fg)
    end)

    it('survives opening another preview', function()
      km.setup({ placement = 'split' })
      assert.is_not_nil(preview.open(md_buf('a'), 'split'))
      vim.api.nvim_set_hl(0, 'KittyMarkdownCode', { fg = '#010203', bg = '#040506' })
      vim.cmd('wincmd p')
      assert.is_not_nil(preview.open(md_buf('b'), 'split'))
      assert.are.equal(0x010203, hl('KittyMarkdownCode').fg)
      assert.are.equal(0x040506, hl('KittyMarkdownCode').bg)
    end)

    it('survives a refresh and a second setup call', function()
      km.setup({ placement = 'split' })
      local buf = md_buf('a')
      assert.is_not_nil(preview.open(buf, 'split'))
      vim.api.nvim_set_hl(0, 'KittyMarkdownBullet', { fg = '#0a0b0c' })
      preview.retheme()
      km.setup({ placement = 'split' })
      assert.are.equal(0x0a0b0c, hl('KittyMarkdownBullet').fg)
    end)
  end)

  it('defines a group again that the user cleared', function()
    km.setup({})
    vim.cmd('highlight clear KittyMarkdownH1')
    km.setup({})
    assert.are.equal(LIGHT_FG, hl('KittyMarkdownH1').fg)
  end)

  describe('a colorscheme load (R-282)', function()
    it('defines the groups again with no preview open', function()
      colorscheme('kmplain', { 'hi clear', "let g:colors_name = 'kmplain'" })
      km.setup({})
      vim.cmd('colorscheme kmplain')
      for _, group in ipairs(theme.groups) do
        assert.is_not_nil(next(hl(group)), group)
      end
      assert.are.equal(LIGHT_FG, hl('KittyMarkdownH1').fg)
    end)

    it('defines the groups again while a preview is open', function()
      colorscheme('kmplain', { 'hi clear', "let g:colors_name = 'kmplain'" })
      km.setup({ placement = 'split' })
      assert.is_not_nil(preview.open(md_buf('a'), 'split'))
      vim.cmd('colorscheme kmplain')
      vim.cmd('colorscheme kmplain')
      for _, group in ipairs(theme.groups) do
        assert.is_not_nil(next(hl(group)), group)
      end
    end)

    it('keeps a group that the colorscheme defines', function()
      colorscheme('kmown', {
        'hi clear',
        "let g:colors_name = 'kmown'",
        'hi KittyMarkdownH2 guifg=#a1b2c3',
        'hi link KittyMarkdownQuote Comment',
      })
      km.setup({})
      vim.cmd('colorscheme kmown')
      assert.are.equal(0xa1b2c3, hl('KittyMarkdownH2').fg)
      assert.are.same({ link = 'Comment' }, hl('KittyMarkdownQuote'))
      assert.are.equal(LIGHT_FG, hl('KittyMarkdownH1').fg)
      -- The colorscheme group then counts as a user group.
      vim.o.background = 'dark'
      theme.apply()
      assert.are.equal(0xa1b2c3, hl('KittyMarkdownH2').fg)
      assert.are.equal(DARK_FG, hl('KittyMarkdownH1').fg)
    end)

    it('uses the palette of the background the colorscheme sets', function()
      colorscheme('kmdark', {
        'hi clear',
        'set background=dark',
        "let g:colors_name = 'kmdark'",
      })
      km.setup({})
      vim.cmd('colorscheme kmdark')
      assert.are.equal(DARK_FG, hl('KittyMarkdownH1').fg)
    end)

    it('drops a user group that the colorscheme clears', function()
      colorscheme('kmplain', { 'hi clear', "let g:colors_name = 'kmplain'" })
      km.setup({})
      vim.api.nvim_set_hl(0, 'KittyMarkdownH1', { fg = '#123456' })
      vim.cmd('colorscheme kmplain')
      assert.are.equal(LIGHT_FG, hl('KittyMarkdownH1').fg)
    end)

    it('works when setup runs after a colorscheme already loaded', function()
      colorscheme('kmown', {
        'hi clear',
        "let g:colors_name = 'kmown'",
        'hi KittyMarkdownH2 guifg=#a1b2c3',
      })
      vim.cmd('colorscheme kmown')
      km.setup({})
      assert.are.equal(0xa1b2c3, hl('KittyMarkdownH2').fg)
      vim.cmd('colorscheme kmown')
      assert.are.equal(0xa1b2c3, hl('KittyMarkdownH2').fg)
      assert.are.equal(LIGHT_FG, hl('KittyMarkdownH1').fg)
    end)
  end)
end)
