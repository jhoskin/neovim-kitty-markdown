local km = require('kitty-markdown')
local config = require('kitty-markdown.config')
local preview = require('kitty-markdown.preview')

local here = vim.fn.fnamemodify(debug.getinfo(1, 'S').source:sub(2), ':p')
local root = vim.fs.dirname(vim.fs.dirname(here))

local function markdown_buf(lines)
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].filetype = 'markdown'
  return buf
end

describe('init', function()
  local notices, real_notify

  before_each(function()
    config.setup({})
    preview.reset()
    notices = {}
    real_notify = vim.notify
    vim.notify = function(msg, level)
      notices[#notices + 1] = { msg = msg, level = level }
    end
  end)

  after_each(function()
    preview.reset()
    vim.notify = real_notify
    vim.fn.has = nil
    config.setup({})
  end)

  it('exposes the documented API (R-013)', function()
    for _, name in ipairs({ 'setup', 'open', 'close', 'toggle', 'refresh', 'layout' }) do
      assert.are.equal('function', type(km[name]), name)
    end
  end)

  it('setup merges options and applies the theme', function()
    km.setup({ text_width = 60 })
    assert.are.equal(60, config.get().text_width)
    assert.is_not_nil(next(vim.api.nvim_get_hl(0, { name = 'KittyMarkdownH1' })))
  end)

  it('layout is pure: the same buffer gives the same model twice (R-153, R-157)', function()
    local buf = markdown_buf({ '# Title', '', 'Some *text* here.', '', '![Alt](pic.png)' })
    local opts = { width = 50, theme = 'light', dir = '/d' }
    local a = km.layout(buf, opts)
    local b = km.layout(buf, opts)
    assert.are.same(a, b)
    assert.are.equal(50, a.width)
    assert.are.equal(1, #a.blocks)
    assert.are.equal('/d/pic.png', a.blocks[1].source)
    assert.is_nil(preview.get(buf))
  end)

  it('refuses to open below Neovim 0.12 with the exact message (R-166)', function()
    vim.fn.has = function(feature)
      if feature == 'nvim-0.12' then
        return 0
      end
      return vim.call('has', feature)
    end
    local buf = markdown_buf({ '# Title' })
    vim.api.nvim_set_current_buf(buf)
    assert.is_nil(km.open('split'))
    assert.is_nil(preview.get(buf))
    assert.are.same({
      { msg = 'kitty-markdown: Neovim 0.12 or later is required', level = vim.log.levels.ERROR },
    }, notices)
    assert.is_nil(km.refresh())
    assert.is_nil(km.close())
  end)

  it('defines the :KittyMarkdown command with its subcommands (R-001, R-003)', function()
    vim.g.loaded_kitty_markdown = nil
    dofile(root .. '/plugin/kitty-markdown.lua')
    local cmd = vim.api.nvim_get_commands({})['KittyMarkdown']
    assert.is_not_nil(cmd)
    assert.are.equal('+', cmd.nargs)
    assert.are.same(
      { 'open', 'close', 'toggle', 'refresh' },
      vim.fn.getcompletion('KittyMarkdown ', 'cmdline')
    )
    assert.are.same(
      { 'split', 'tab', 'float' },
      vim.fn.getcompletion('KittyMarkdown open ', 'cmdline')
    )
    vim.cmd('KittyMarkdown nope')
    assert.matches('unknown subcommand', notices[#notices].msg)
    vim.cmd('KittyMarkdown close float')
    assert.matches('unexpected argument', notices[#notices].msg)
  end)

  it('rejects a third argument by name and does nothing (R-250)', function()
    vim.g.loaded_kitty_markdown = nil
    dofile(root .. '/plugin/kitty-markdown.lua')
    local buf = markdown_buf({ '# Title' })
    vim.api.nvim_set_current_buf(buf)
    local function rejects(line, arg)
      local before = #notices
      vim.cmd(line)
      assert.are.equal(before + 1, #notices, line)
      assert.are.equal(
        string.format('kitty-markdown: unexpected argument "%s"', arg),
        notices[#notices].msg
      )
      assert.are.equal(vim.log.levels.ERROR, notices[#notices].level)
    end
    rejects('KittyMarkdown open split extra', 'extra')
    rejects('KittyMarkdown open tab tab', 'tab')
    rejects('KittyMarkdown toggle float 3', '3')
    assert.is_nil(preview.get(buf))
    vim.cmd('KittyMarkdown open split')
    local p = preview.get(buf)
    assert.is_not_nil(p)
    vim.api.nvim_set_current_win(vim.fn.bufwinid(buf))
    rejects('KittyMarkdown toggle split x', 'x')
    rejects('KittyMarkdown close tab more', 'tab')
    rejects('KittyMarkdown open float more', 'more')
    assert.are.equal(p, preview.get(buf))
    assert.is_true(vim.api.nvim_win_is_valid(p.win))
    rejects('KittyMarkdown refresh split now', 'split')
  end)

  it('routes the command to the preview of the current buffer (R-002, R-008)', function()
    vim.g.loaded_kitty_markdown = nil
    dofile(root .. '/plugin/kitty-markdown.lua')
    local buf = markdown_buf({ '# Title', '', 'Body.' })
    vim.api.nvim_set_current_buf(buf)
    vim.cmd('KittyMarkdown open split')
    local p = preview.get(buf)
    assert.is_not_nil(p)
    assert.are.equal('split', p.placement)
    assert.matches('Title', vim.api.nvim_buf_get_lines(p.buf, 0, 1, false)[1])
    vim.cmd('KittyMarkdown close')
    assert.is_nil(preview.get(buf))
    vim.cmd('KittyMarkdown toggle')
    assert.are.equal('tab', preview.get(buf).placement)
    vim.cmd('KittyMarkdown toggle')
    assert.is_nil(preview.get(buf))
  end)
end)
