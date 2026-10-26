-- Layout rules that the first QA round found broken. Each test covers
-- several inputs of one class, not one example.

local parser = require('kitty-markdown.parser')
local layout = require('kitty-markdown.layout')
local icons = require('kitty-markdown.icons')

local OPTS = { width = 44, theme = 'light', dir = '/fixtures' }
local TEXT_WIDTH = 40

local function build(lines)
  if type(lines) == 'string' then
    lines = vim.split(lines, '\n')
  end
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  local model = layout.build(parser.parse(buf), OPTS)
  vim.api.nvim_buf_delete(buf, { force = true })
  return model
end

-- Preview lines without the 2-cell margin.
local function texts(model)
  local out = {}
  for i, line in ipairs(model.lines) do
    out[i] = line:sub(3)
  end
  return out
end

local function find_line(model, needle)
  for i, line in ipairs(model.lines) do
    if line:find(needle, 1, true) then
      return i
    end
  end
  return nil
end

local function groups_at(model, line, needle)
  local s = model.lines[line]:find(needle, 1, true)
  assert.is_not_nil(s, 'no ' .. vim.inspect(needle) .. ' on line ' .. line)
  local out = {}
  for _, hl in ipairs(model.highlights) do
    if hl.line == line and hl.col_start <= s - 1 and hl.col_end >= s - 1 + #needle then
      out[#out + 1] = hl.group
    end
  end
  return out
end

local function has_group_anywhere(model, group)
  for _, hl in ipairs(model.highlights) do
    if hl.group == group then
      return true
    end
  end
  return false
end

describe('layout rules', function()
  describe('soft line breaks (R-029)', function()
    local cases = {
      { 'alpha', '*beta* gamma', 'alpha beta gamma' },
      { 'alpha', '**beta**', 'alpha beta' },
      { 'alpha', '`code` end', 'alpha code end' },
      { 'alpha', '[link](https://x.y) end', 'alpha link end' },
      { 'alpha', '~~gone~~', 'alpha gone' },
      { '*one*', '*two*', 'one two' },
      { '`one`', '`two`', 'one two' },
      { '[one](a)', '[two](b)', 'one two' },
      { 'alpha *beta*', 'gamma', 'alpha beta gamma' },
      { 'alpha `code`', 'gamma', 'alpha code gamma' },
      { 'alpha [link](u)', 'gamma', 'alpha link gamma' },
      { '**one**', '`two`', 'one two' },
      { 'alpha', '<https://x.y>', 'alpha https://x.y' },
    }
    for _, case in ipairs(cases) do
      it('joins ' .. vim.inspect(case[1] .. '\\n' .. case[2]) .. ' with one space', function()
        local model = build({ case[1], case[2] })
        assert.are.same({ case[3] }, texts(model))
      end)
    end

    it('joins a styled run after a break inside a list item and a quote', function()
      assert.are.same({ '● one two' }, texts(build({ '- one', '  *two*' })))
      assert.are.same({ icons.quote .. ' one two' }, texts(build({ '> one', '> `two`' })))
    end)
  end)

  describe('inline math delimiters (R-062, R-280)', function()
    local literal = {
      'Price $5 and $10 here.',
      'From $5 to $10 and $20.',
      'a $ x$ b',
      'a $x $ b',
      'a $ x $ b',
      '*cost $5 and $10*',
      'a$b $c',
    }
    for _, text in ipairs(literal) do
      it('keeps ' .. vim.inspect(text) .. ' as written', function()
        local model = build({ text })
        local want = text:gsub('%*', '')
        assert.are.same({ want }, texts(model))
        assert.is_false(has_group_anywhere(model, 'KittyMarkdownMath'))
      end)
    end

    it('keeps two prices across a soft break as written', function()
      local model = build({ 'costs $5 and', '$10 more' })
      assert.are.same({ 'costs $5 and $10 more' }, texts(model))
      assert.is_false(has_group_anywhere(model, 'KittyMarkdownMath'))
    end)

    it('opens math at the next valid dollar sign', function()
      local model = build({ 'a $ 5 and $x$ b' })
      assert.are.same({ 'a $ 5 and x b' }, texts(model))
      assert.is_true(vim.tbl_contains(groups_at(model, 1, 'x'), 'KittyMarkdownMath'))
    end)

    it('keeps prices as written next to a valid span', function()
      local model = build({ 'Pay $5 and $10 for $x^2$.', '', 'Not $ y$ nor $y $ but $z$.' })
      assert.are.same({ 'Pay $5 and $10 for x².', '', 'Not $ y$ nor $y $ but z.' }, texts(model))
      local math_spans = 0
      for _, hl in ipairs(model.highlights) do
        if hl.group == 'KittyMarkdownMath' then
          math_spans = math_spans + 1
        end
      end
      assert.are.equal(2, math_spans)
    end)

    it('still treats valid spans as math', function()
      local model = build({ 'with $x^2$ and $y$ end' })
      assert.are.same({ 'with x² and y end' }, texts(model))
      assert.is_true(vim.tbl_contains(groups_at(model, 1, 'x²'), 'KittyMarkdownMath'))
      assert.is_true(vim.tbl_contains(groups_at(model, 1, 'y'), 'KittyMarkdownMath'))
    end)
  end)

  describe('wikilinks (R-036)', function()
    local cases = {
      'See [[Page]] here.',
      'See [[Page|alias]] here.',
      'See ![[image.png]] here.',
      'See ![[image.png|200]] here.',
      'See [[Page#Section]] here.',
    }
    for _, text in ipairs(cases) do
      it('keeps ' .. vim.inspect(text) .. ' as written with no link', function()
        for _, defs in ipairs({
          {},
          { '', '[Page]: https://x.y', '[image.png]: https://x.y/i.png' },
        }) do
          local lines = { text }
          vim.list_extend(lines, defs)
          local model = build(lines)
          assert.are.same({ text }, texts(model))
          assert.are.same({}, model.links)
          assert.are.same({}, model.blocks)
        end
      end)
    end
  end)
end)
