local config = require('kitty-markdown.config')
local theme = require('kitty-markdown.theme')

describe('theme', function()
  before_each(function()
    config.setup({})
    theme.reset()
    vim.o.background = 'light'
  end)

  after_each(function()
    theme.reset()
    vim.o.background = 'light'
  end)

  it('defines every group for the light palette', function()
    theme.apply('light')
    for _, group in ipairs(theme.groups) do
      local hl = vim.api.nvim_get_hl(0, { name = group })
      assert.is_not_nil(next(hl), group)
    end
    assert.are.equal(0x1f2328, vim.api.nvim_get_hl(0, { name = 'KittyMarkdownH1' }).fg)
    assert.are.equal(0x0969da, vim.api.nvim_get_hl(0, { name = 'KittyMarkdownLink' }).fg)
  end)

  it('uses the dark palette for the dark theme', function()
    theme.apply('dark')
    assert.are.equal(0xf0f6fc, vim.api.nvim_get_hl(0, { name = 'KittyMarkdownH1' }).fg)
    assert.are.equal(0x151b23, vim.api.nvim_get_hl(0, { name = 'KittyMarkdownCode' }).bg)
  end)

  it('follows &background when the theme option is auto', function()
    assert.are.equal('light', theme.current())
    vim.o.background = 'dark'
    assert.are.equal('dark', theme.current())
  end)

  it('lets the theme option override &background', function()
    vim.o.background = 'dark'
    config.setup({ theme = 'light' })
    assert.are.equal('light', theme.current())
    assert.are.equal('dark', theme.resolve('dark'))
  end)

  it('keeps a group the user defined before the first apply', function()
    vim.api.nvim_set_hl(0, 'KittyMarkdownH1', { fg = '#123456' })
    theme.apply('light')
    assert.are.equal(0x123456, vim.api.nvim_get_hl(0, { name = 'KittyMarkdownH1' }).fg)
    theme.apply('dark')
    assert.are.equal(0x123456, vim.api.nvim_get_hl(0, { name = 'KittyMarkdownH1' }).fg)
    assert.are.equal(0xf0f6fc, vim.api.nvim_get_hl(0, { name = 'KittyMarkdownH2' }).fg)
  end)

  it('re-applies over its own groups on a palette change', function()
    theme.apply('light')
    theme.apply('dark')
    assert.are.equal(0xf0f6fc, vim.api.nvim_get_hl(0, { name = 'KittyMarkdownText' }).fg)
  end)

  it('picks the mermaid theme from the palette unless configured', function()
    assert.are.equal('default', theme.mermaid_theme())
    vim.o.background = 'dark'
    assert.are.equal('dark', theme.mermaid_theme())
    config.setup({ mermaid = { theme = 'forest' } })
    assert.are.equal('forest', theme.mermaid_theme())
  end)
end)
