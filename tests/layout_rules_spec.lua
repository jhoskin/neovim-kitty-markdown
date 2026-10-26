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

  describe('task items (R-042, R-272, R-274)', function()
    it('shows the checkbox in place of the bullet and the marker', function()
      local model = build({ '- [ ] open', '- [x] done', '- [X] upper' })
      assert.are.same(
        { icons.unchecked .. ' open', icons.checked .. ' done', icons.checked .. ' upper' },
        texts(model)
      )
      for _, line in ipairs(model.lines) do
        for _, bullet in ipairs(icons.bullet) do
          assert.is_nil(line:find(bullet, 1, true))
        end
      end
      assert.is_true(
        vim.tbl_contains(groups_at(model, 1, icons.unchecked), 'KittyMarkdownUnchecked')
      )
      assert.is_true(vim.tbl_contains(groups_at(model, 2, icons.checked), 'KittyMarkdownChecked'))
    end)

    it('uses the glyphs of the Icons definition', function()
      local model = build({ '- [ ] a', '- [x] b' })
      assert.are.equal(0xF0131, vim.fn.char2nr(model.lines[1]:sub(3)))
      assert.are.equal(0xF0C52, vim.fn.char2nr(model.lines[2]:sub(3)))
    end)

    it('shows a nested task item with the checkbox and no depth bullet', function()
      local model = build({ '- parent', '  - [ ] child', '    - [x] grandchild', '  - plain' })
      assert.are.same({
        icons.bullet[1] .. ' parent',
        '  ' .. icons.unchecked .. ' child',
        '    ' .. icons.checked .. ' grandchild',
        '  ' .. icons.bullet[2] .. ' plain',
      }, texts(model))
    end)

    it('aligns wrapped task text under the text after the checkbox', function()
      local words = string.rep('word ', 12)
      local model = build({ '- [ ] ' .. words })
      assert.is_true(#model.lines >= 2)
      local cont = model.lines[2]:sub(3)
      assert.are.equal(
        string.rep(' ', vim.api.nvim_strwidth(icons.unchecked) + 1),
        cont:match('^ *')
      )
    end)

    it('shows the checkbox in place of the number in an ordered list', function()
      local model = build({ '1. [ ] first', '2. second', '3. [x] third' })
      assert.are.same(
        { icons.unchecked .. ' first', '2. second', icons.checked .. ' third' },
        texts(model)
      )
    end)
  end)

  describe('footnotes (R-054, R-219, R-273)', function()
    local doc = {
      'Body ref[^a] and[^b].',
      '',
      '[^a]: First paragraph.',
      '',
      '    Second paragraph.',
      '',
      '    ```lua',
      '    print(1)',
      '    ```',
      '',
      '    - one',
      '    - two',
      '',
      '[^b]: Other note.',
      '',
      'Tail paragraph.',
    }

    it('shows every paragraph and block of a definition at the end', function()
      local t = texts(build(doc))
      assert.are.same({
        'Body ref¹ and².',
        '',
        'Tail paragraph.',
        '',
        '¹ First paragraph.',
        '',
        '  Second paragraph.',
        '',
        '  ── lua ' .. string.rep('─', TEXT_WIDTH - 2 - 2 - 5),
        '  print(1)' .. string.rep(' ', TEXT_WIDTH - 2 - 8),
        '  ' .. string.rep('─', TEXT_WIDTH - 2),
        '',
        '  ● one',
        '  ● two',
        '',
        '² Other note.',
      }, t)
    end)

    it('keeps a later paragraph of the last definition at the end', function()
      local t = texts(build({ 'See[^n].', '', '[^n]: One.', '', '    Two.', '', '    Three.' }))
      assert.are.same({ 'See¹.', '', '¹ One.', '', '  Two.', '', '  Three.' }, t)
    end)

    it('maps the lines of a later paragraph to its source line', function()
      -- the definition is last, so the mapping can stay non-decreasing
      local model = build({ 'See[^n].', '', '[^n]: One.', '', '    Two.', '', '    Three.' })
      assert.are.equal(5, model.source_lines[find_line(model, 'Two.')])
      assert.are.equal(7, model.source_lines[find_line(model, 'Three.')])
    end)

    it('shows a reference with no definition as written', function()
      local model = build({ 'a[^x] b[^nope] c[^y] d[^Nope2]', '', '[^x]: X.', '[^y]: Y.' })
      local t = texts(model)
      assert.are.equal('a¹ b[^nope] c² d[^Nope2]', t[1])
      assert.are.same({ { label = 'x', number = 1 }, { label = 'y', number = 2 } }, model.footnotes)
      assert.are.same({ '¹ X.', '', '² Y.' }, { t[#t - 2], t[#t - 1], t[#t] })
    end)

    it('numbers nothing when no reference has a definition', function()
      local model = build({ 'Only[^missing] here.' })
      assert.are.same({ 'Only[^missing] here.' }, texts(model))
      assert.are.same({}, model.footnotes)
    end)
  end)

  describe('alerts in any letter case (R-267)', function()
    local kinds = {
      { 'note', 'NOTE', 'Note' },
      { 'Warning', 'WARNING', 'Warning' },
      { 'tIp', 'TIP', 'Tip' },
      { 'important', 'IMPORTANT', 'Important' },
      { 'Caution', 'CAUTION', 'Caution' },
    }
    for _, k in ipairs(kinds) do
      it('recognizes [!' .. k[1] .. ']', function()
        local model = build({ '> [!' .. k[1] .. ']', '> body text' })
        local upper = build({ '> [!' .. k[2] .. ']', '> body text' })
        assert.are.same(upper.lines, model.lines)
        assert.are.same(upper.highlights, model.highlights)
        local title = icons.alert[k[2]] .. ' ' .. k[3]
        assert.are.equal(icons.quote .. ' ' .. title, texts(model)[1])
        local group = 'KittyMarkdownAlert' .. k[3]
        assert.is_true(vim.tbl_contains(groups_at(model, 1, k[3]), group))
      end)
    end
  end)

  describe('HTML comments (R-270)', function()
    it('hides a comment block and keeps one blank line between its neighbors', function()
      assert.are.same({ 'a', '', 'b' }, texts(build({ 'a', '', '<!-- hidden -->', '', 'b' })))
      assert.are.same(
        { 'a', '', 'b' },
        texts(build({ 'a', '', '<!-- one -->', '', '<!-- two -->', '', 'b' }))
      )
      assert.are.same({ 'b' }, texts(build({ '<!-- start -->', '', 'b' })))
      assert.are.same({ 'a' }, texts(build({ 'a', '', '<!-- end -->' })))
    end)

    it('hides a multi-line comment block entirely', function()
      local model = build({ 'a', '', '<!--', 'line one', '', 'line two', '-->', '', 'b' })
      assert.are.same({ 'a', '', 'b' }, texts(model))
      model = build({ 'a', '', '<!-- start', '  more', 'end -->', '', 'b' })
      assert.are.same({ 'a', '', 'b' }, texts(model))
    end)

    it('hides a comment block inside a list item and a quote', function()
      local t = texts(build({ '- item', '', '  <!-- hidden -->', '', '  more' }))
      assert.is_nil(table.concat(t, '\n'):find('hidden', 1, true))
      t = texts(build({ '> quoted', '>', '> <!-- hidden -->' }))
      assert.is_nil(table.concat(t, '\n'):find('hidden', 1, true))
    end)

    it('keeps the HTML around a comment in the same block', function()
      local t = texts(build({ '<div>', '<!-- note -->', 'kept', '</div>' }))
      assert.are.same({ '<div>', 'kept', '</div>' }, t)
    end)

    it('hides an inline comment and keeps the text around it', function()
      assert.are.same({ 'two words' }, texts(build({ 'two <!-- x --> words' })))
      assert.are.same({ 'two words' }, texts(build({ 'two<!-- x --> words' })))
      assert.are.same({ 'end' }, texts(build({ 'end <!-- x -->' })))
      assert.are.same({ 'a b' }, texts(build({ 'a <!-- x', 'y --> b' })))
      assert.are.same({ 'a b c' }, texts(build({ 'a <!--x--> *b* c' })))
    end)
  end)

  describe('duplicate slugs (R-206, R-275)', function()
    local function slugs(headings)
      local lines = {}
      for _, h in ipairs(headings) do
        lines[#lines + 1] = '# ' .. h
        lines[#lines + 1] = ''
      end
      local model = build(lines)
      local by_line = {}
      for slug, line in pairs(model.anchors) do
        by_line[#by_line + 1] = { line, slug }
      end
      table.sort(by_line, function(a, b)
        return a[1] < b[1]
      end)
      local out = {}
      for i, entry in ipairs(by_line) do
        out[i] = entry[2]
      end
      return out
    end

    it('follows github-slugger for suffixed duplicates', function()
      assert.are.same(
        { 'dup', 'dup-1', 'dup-1-1', 'dup-2' },
        slugs({ 'Dup', 'Dup', 'Dup-1', 'Dup' })
      )
      assert.are.same({ 'a-1', 'a', 'a-2' }, slugs({ 'a-1', 'a', 'a' }))
      assert.are.same({ 'x', 'x-1', 'x-2', 'x-3' }, slugs({ 'x', 'x-1', 'x', 'x' }))
      assert.are.same({ 'b', 'b-1', 'b-2', 'b-1-1' }, slugs({ 'b', 'b', 'b', 'b-1' }))
    end)
  end)

  describe('display math source fallback (R-070, R-279)', function()
    it('shows each source line on its own centered preview line', function()
      local model = build({ '$$', '\\unknowncmd{a}', 'x   +   y', '$$' })
      local t = texts(model)
      assert.are.equal(2, #t)
      local width = math.max(#'\\unknowncmd{a}', #'x + y')
      local pad = math.floor((TEXT_WIDTH - width) / 2)
      assert.are.equal(string.rep(' ', pad) .. '\\unknowncmd{a}', t[1])
      assert.are.equal(string.rep(' ', pad) .. 'x + y', t[2])
      for line = 1, 2 do
        assert.is_true(
          vim.tbl_contains(groups_at(model, line, t[line]:match('%S.*')), 'KittyMarkdownMath')
        )
      end
    end)

    it('keeps one-line display math on one centered line', function()
      local t = texts(build({ '$$\\unknowncmd{a}$$' }))
      local pad = math.floor((TEXT_WIDTH - #'\\unknowncmd{a}') / 2)
      assert.are.same({ string.rep(' ', pad) .. '\\unknowncmd{a}' }, t)
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
