local parser = require('kitty-markdown.parser')
local layout = require('kitty-markdown.layout')
local icons = require('kitty-markdown.icons')
local config = require('kitty-markdown.config')

local here = debug.getinfo(1, 'S').source:sub(2)
local fixtures = vim.fs.joinpath(vim.fs.dirname(vim.fn.fnamemodify(here, ':p')), 'fixtures')

-- Every model in these tests is built with fixed options, so the result
-- does not depend on the machine, the terminal, or &background.
local OPTS = { width = 44, theme = 'light', dir = '/fixtures' }
local TEXT_WIDTH = 40

local function load_fixture(name)
  local buf = vim.fn.bufadd(vim.fs.joinpath(fixtures, name))
  vim.fn.bufload(buf)
  vim.bo[buf].filetype = 'markdown'
  return buf
end

local function build_fixture(name, opts)
  return layout.build(parser.parse(load_fixture(name)), opts or OPTS)
end

local function build_lines(lines, opts)
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].filetype = 'markdown'
  return layout.build(parser.parse(buf), opts or OPTS)
end

-- The first preview line whose text contains `needle`.
local function find_line(model, needle)
  for i, line in ipairs(model.lines) do
    if line:find(needle, 1, true) then
      return i
    end
  end
  return nil
end

-- Highlight groups that cover `needle` on `line`, in emission order.
local function groups_over(model, line, needle)
  local text = model.lines[line]
  local s = text:find(needle, 1, true)
  assert.is_not_nil(s, 'no ' .. vim.inspect(needle) .. ' on line ' .. line)
  local col_start, col_end = s - 1, s - 1 + #needle
  local out = {}
  for _, hl in ipairs(model.highlights) do
    if hl.line == line and hl.col_start <= col_start and hl.col_end >= col_end then
      out[#out + 1] = hl.group
    end
  end
  return out
end

local function has_group(model, line, needle, group)
  return vim.tbl_contains(groups_over(model, line, needle), group)
end

local function span_of(model, line, group)
  for _, hl in ipairs(model.highlights) do
    if hl.line == line and hl.group == group then
      return model.lines[line]:sub(hl.col_start + 1, hl.col_end)
    end
  end
  return nil
end

local function strwidth(s)
  return vim.api.nvim_strwidth(s)
end

local function stub_math(fn)
  local saved = package.loaded['kitty-markdown.math']
  package.loaded['kitty-markdown.math'] = { render = fn }
  return function()
    package.loaded['kitty-markdown.math'] = saved
  end
end

describe('layout', function()
  after_each(function()
    config.setup({})
  end)

  describe('slug', function()
    it('makes GitHub slugs', function()
      assert.are.equal('title-one', layout.slug('Title One'))
      assert.are.equal('section-two', layout.slug('Section *two*'))
      assert.are.equal('whats-new-v20', layout.slug("What's New: v2.0"))
      assert.are.equal('keep_under-score', layout.slug('keep_under-score'))
      assert.are.equal('日本語', layout.slug('日本語'))
    end)
  end)

  describe('model shape (R-153, R-154, R-203)', function()
    it('returns plain tables only', function()
      local model = build_fixture('headings.md')
      local function check(value, path)
        local t = type(value)
        if t == 'table' then
          assert.is_nil(getmetatable(value), path .. ' has a metatable')
          for k, v in pairs(value) do
            check(v, path .. '.' .. tostring(k))
          end
        else
          assert.is_true(t == 'string' or t == 'number' or t == 'boolean', path .. ' is a ' .. t)
        end
      end
      check(model, 'model')
    end)

    it('holds every documented key', function()
      local model = build_fixture('images.md')
      for _, key in ipairs({
        'width',
        'text_width',
        'theme',
        'lines',
        'highlights',
        'conceals',
        'blocks',
        'code_regions',
        'links',
        'anchors',
        'source_lines',
        'footnotes',
      }) do
        assert.is_not_nil(model[key], 'missing ' .. key)
      end
      assert.are.same({}, model.conceals)
      assert.are.equal(44, model.width)
      assert.are.equal(TEXT_WIDTH, model.text_width)
      assert.are.equal('light', model.theme)
    end)

    it('caps the text width by the configured maximum', function()
      config.setup({ text_width = 30 })
      local model = build_lines({ 'x' }, { width = 100, theme = 'light' })
      assert.are.equal(30, model.text_width)
      local wide = build_lines({ 'x' }, { width = 20, theme = 'light' })
      assert.are.equal(16, wide.text_width)
    end)

    it('defaults width to the configured text width plus the margins', function()
      local model = build_lines({ 'x' }, { theme = 'light' })
      assert.are.equal(config.get().text_width + 4, model.width)
      assert.are.equal(config.get().text_width, model.text_width)
    end)

    it('runs through the public module without a terminal', function()
      local buf = load_fixture('headings.md')
      local model = require('kitty-markdown').layout(buf, OPTS)
      assert.are.equal(TEXT_WIDTH, model.text_width)
      assert.is_true(#model.lines > 0)
    end)

    it('gives every line a 2-cell left margin', function()
      local model = build_fixture('inline.md')
      for i, line in ipairs(model.lines) do
        if line ~= '' then
          assert.are.equal('  ', line:sub(1, 2), 'line ' .. i)
        end
      end
    end)
  end)

  describe('headings (R-027, R-028, R-171, R-206)', function()
    local model
    before_each(function()
      model = build_fixture('headings.md')
    end)

    it('shows an icon and the text in the level group', function()
      assert.are.equal('  ' .. icons.h[1] .. ' Title One', model.lines[1])
      assert.is_true(has_group(model, 1, 'Title One', 'KittyMarkdownH1'))
      local two = find_line(model, 'Section two')
      assert.are.equal('  ' .. icons.h[2] .. ' Section two', model.lines[two])
      assert.is_true(has_group(model, two, 'Section two', 'KittyMarkdownH2'))
      assert.is_true(has_group(model, two, 'two', 'KittyMarkdownEmph'))
      local six = find_line(model, 'Six')
      assert.is_true(has_group(model, six, 'Six', 'KittyMarkdownH6'))
    end)

    it('shows no markers', function()
      for _, line in ipairs(model.lines) do
        assert.is_nil(line:match('^%s*#'), line)
        assert.is_nil(line:match('^%s*[=-]+$'), line)
        assert.is_nil(line:find('*', 1, true), line)
      end
    end)

    it('maps slugs to preview lines', function()
      assert.are.equal(1, model.anchors['title-one'])
      assert.are.equal(find_line(model, 'Section two'), model.anchors['section-two'])
      assert.are.equal(7, model.anchors['dup'])
      assert.are.equal(9, model.anchors['dup-1'])
      assert.is_not_nil(model.anchors['setext-two'])
    end)
  end)

  describe('paragraphs (R-029, R-030, R-031)', function()
    it('wraps at word boundaries within the text width', function()
      local model = build_fixture('long_lines.md')
      local start = find_line(model, 'Short words')
      assert.are.equal('  Short words wrap normally across the', model.lines[start])
      assert.are.equal('  configured width when the paragraph is', model.lines[start + 1])
      assert.are.equal('  long enough to need more than one line.', model.lines[start + 2])
      for i = start, start + 2 do
        assert.is_true(strwidth(model.lines[i]) <= TEXT_WIDTH + 2)
      end
    end)

    it('never splits a word; a long word stands alone', function()
      local model = build_fixture('long_lines.md')
      assert.matches('^  Avery.*xxxx$', model.lines[1])
      assert.are.equal('  stands alone.', model.lines[2])
    end)

    it('breaks the line on a hard line break', function()
      local model = build_fixture('inline.md')
      local line = find_line(model, 'Hard break')
      assert.are.equal('  Hard break with spaces', model.lines[line])
      assert.are.equal('  next line and backslash', model.lines[line + 1])
      assert.are.equal('  third line.', model.lines[line + 2])
    end)

    it('measures wide characters by display width (R-045)', function()
      local model = build_fixture('wide_chars.md')
      local line = find_line(model, '東京')
      assert.are.equal('  │ 東京 │ 1  │', model.lines[line])
      assert.are.equal(strwidth(model.lines[line - 2]), strwidth(model.lines[line]))
    end)
  end)

  describe('inline styles (R-032 to R-035, R-056 to R-058, R-060)', function()
    local model
    before_each(function()
      model = build_fixture('inline.md')
    end)

    it('styles emphasis, strong, strike, and code without markers', function()
      assert.are.equal('  Plain emph and strong and strike and', model.lines[1])
      assert.are.equal('emph', span_of(model, 1, 'KittyMarkdownEmph'))
      assert.are.equal('strong', span_of(model, 1, 'KittyMarkdownStrong'))
      assert.are.equal('strike', span_of(model, 1, 'KittyMarkdownStrike'))
      assert.are.equal('code span', span_of(model, 2, 'KittyMarkdownCode'))
    end)

    it('covers a nested run with both groups', function()
      local line = find_line(model, 'both')
      assert.is_true(has_group(model, line, 'both', 'KittyMarkdownEmph'))
      assert.is_true(has_group(model, line, 'both', 'KittyMarkdownStrong'))
    end)

    it('replaces known shortcodes and keeps unknown ones', function()
      local line = find_line(model, 'Shortcodes')
      assert.are.equal('  Shortcodes 🎉 and :unknown_thing: stay.', model.lines[line])
    end)

    it('shows escapes and entities decoded', function()
      local line = find_line(model, 'Escapes')
      assert.are.equal('  Escapes * _ # and entities & < © A B', model.lines[line])
      assert.are.equal('  &bogus;.', model.lines[line + 1])
    end)

    it('shows inline HTML as muted text', function()
      local line = find_line(model, 'bold html')
      assert.are.equal('  Inline <b>bold html</b> and <br> tags.', model.lines[line])
      assert.are.equal('<b>', span_of(model, line, 'KittyMarkdownHtml'))
      assert.is_true(has_group(model, line, '<br>', 'KittyMarkdownHtml'))
    end)
  end)

  describe('links (R-036 to R-038, R-207)', function()
    local model
    before_each(function()
      model = build_fixture('links.md')
    end)

    it('shows link text underlined and hides the URL', function()
      assert.are.equal('  Inline link text here.', model.lines[1])
      assert.are.equal('link text', span_of(model, 1, 'KittyMarkdownLink'))
      assert.are.same({
        line = 1,
        col_start = 9,
        col_end = 18,
        kind = 'url',
        target = 'https://example.com/path',
      }, model.links[1])
    end)

    it('shows bare URLs and autolinks in the URL group', function()
      local line = find_line(model, 'Autolink')
      assert.are.equal('https://auto.example', span_of(model, line, 'KittyMarkdownUrl'))
      assert.are.equal('https://bare.example/x', span_of(model, line + 1, 'KittyMarkdownUrl'))
      local mail = find_line(model, 'me@example.com')
      local found
      for _, link in ipairs(model.links) do
        if link.line == mail and link.target == 'mailto:me@example.com' then
          found = link
        end
      end
      assert.is_not_nil(found)
    end)

    it('renders reference links like inline links and hides definitions', function()
      local line = find_line(model, 'Reference')
      assert.are.equal('  Reference ref link and collapsed and', model.lines[line])
      assert.is_true(has_group(model, line, 'ref link', 'KittyMarkdownLink'))
      assert.is_true(has_group(model, line, 'collapsed', 'KittyMarkdownLink'))
      assert.is_true(has_group(model, line + 1, 'shortcut', 'KittyMarkdownLink'))
      assert.is_nil(find_line(model, 'ref.example'))
    end)

    it('classifies anchor and file targets', function()
      local line = find_line(model, 'Anchor')
      local kinds = {}
      for _, link in ipairs(model.links) do
        if link.line == line then
          kinds[#kinds + 1] = { link.kind, link.target }
        end
      end
      assert.are.same({
        { 'anchor', 'target-heading' },
        { 'file', '/fixtures/docs/README.md' },
      }, kinds)
      assert.are.equal(find_line(model, 'Target Heading'), model.anchors['target-heading'])
    end)
  end)

  describe('lists (R-039 to R-042)', function()
    local model
    before_each(function()
      model = build_fixture('lists.md')
    end)

    it('shows bullets by depth with 2-cell indents', function()
      assert.are.equal('  ' .. icons.bullet[1] .. ' one', model.lines[1])
      assert.are.equal('    ' .. icons.bullet[2] .. ' two.a', model.lines[3])
      assert.are.equal('      ' .. icons.bullet[3] .. ' two.a.i', model.lines[4])
      assert.are.equal('        ' .. icons.bullet[4] .. ' two.a.i.x', model.lines[5])
      assert.is_true(has_group(model, 1, icons.bullet[1], 'KittyMarkdownBullet'))
    end)

    it('numbers ordered items from the start number', function()
      local first = find_line(model, 'first')
      assert.are.equal('  1. first', model.lines[first])
      assert.are.equal('  3. third', model.lines[first + 2])
      assert.is_true(has_group(model, first, '1.', 'KittyMarkdownNumber'))
      local seven = find_line(model, 'seven')
      assert.are.equal('  7. seven', model.lines[seven])
      assert.are.equal('  8. eight', model.lines[seven + 1])
    end)

    it('shows checkbox icons for task items', function()
      local done = find_line(model, 'done task')
      assert.are.equal(
        '  ' .. icons.bullet[1] .. ' ' .. icons.checked .. ' done task',
        model.lines[done]
      )
      assert.is_true(has_group(model, done, icons.checked, 'KittyMarkdownChecked'))
      assert.is_true(has_group(model, done + 1, icons.unchecked, 'KittyMarkdownUnchecked'))
    end)

    it('aligns continuation lines under the item text', function()
      local line = find_line(model, 'item with a long')
      assert.are.equal(
        '  ' .. icons.bullet[1] .. ' item with a long text that must wrap',
        model.lines[line]
      )
      assert.are.equal('    onto a continuation line aligned under', model.lines[line + 1])
    end)

    it('indents a second paragraph under the item text', function()
      local line = find_line(model, 'loose item')
      assert.are.equal('', model.lines[line + 1])
      assert.are.equal('     second paragraph', model.lines[line + 2])
    end)
  end)

  describe('tables (R-043 to R-046)', function()
    local model
    before_each(function()
      model = build_fixture('tables.md')
    end)

    it('draws box borders around every cell', function()
      assert.are.equal(
        '  ┌─────────────┬────────┬───────┐',
        model.lines[1]
      )
      assert.are.equal('  │ Left        │ Center │ Right │', model.lines[2])
      assert.are.equal(
        '  ├─────────────┼────────┼───────┤',
        model.lines[3]
      )
      assert.are.equal(
        '  └─────────────┴────────┴───────┘',
        model.lines[7]
      )
      assert.is_true(has_group(model, 1, '┌', 'KittyMarkdownTableBorder'))
      assert.is_true(has_group(model, 2, '│', 'KittyMarkdownTableBorder'))
    end)

    it('aligns columns per the delimiter row', function()
      assert.are.equal('  │ a           │   b    │     c │', model.lines[4])
      assert.are.equal('  │ longer cell │  bold  │     1 │', model.lines[5])
    end)

    it('sizes columns by display width including wide characters', function()
      assert.are.equal('  │ 日本語      │   x    │    🎉 │', model.lines[6])
      assert.are.equal(strwidth(model.lines[1]), strwidth(model.lines[6]))
    end)

    it('shows the header row bold and keeps inline styles in cells', function()
      assert.is_true(has_group(model, 2, 'Left', 'KittyMarkdownTableHeader'))
      assert.is_true(has_group(model, 2, 'Center', 'KittyMarkdownTableHeader'))
      assert.is_true(has_group(model, 5, 'bold', 'KittyMarkdownStrong'))
      assert.is_false(has_group(model, 4, 'a', 'KittyMarkdownTableHeader'))
    end)

    it('pads short rows and drops extra cells', function()
      local line = find_line(model, 'only one')
      assert.are.equal('  │ only one │     │', model.lines[line])
      assert.are.equal('  │ one      │ two │', model.lines[line + 1])
    end)

    it('lets a wide table overflow without truncation', function()
      local wide = build_fixture('long_lines.md')
      local line = find_line(wide, 'pushes the table')
      assert.is_true(strwidth(wide.lines[line]) > TEXT_WIDTH + 2)
      assert.is_not_nil(wide.lines[line]:find('on and on and on │', 1, true))
    end)
  end)

  describe('code (R-047 to R-050, R-208, R-209)', function()
    local model
    before_each(function()
      model = build_fixture('code.md')
    end)

    it('labels the top border with the language', function()
      assert.are.equal('── lua ', model.lines[1]:sub(3, 3 + #'── lua ' - 1))
      assert.are.equal(' lua ', span_of(model, 1, 'KittyMarkdownCodeLang'))
      assert.is_true(has_group(model, 1, '──', 'KittyMarkdownCodeBorder'))
      assert.are.equal(TEXT_WIDTH, strwidth(model.lines[1]) - 2)
    end)

    it('pads code lines to the text width under the code background', function()
      assert.are.equal(
        '  local x = 1' .. string.rep(' ', TEXT_WIDTH - #'local x = 1'),
        model.lines[2]
      )
      assert.are.equal(model.lines[2]:sub(3), span_of(model, 2, 'KittyMarkdownCodeBg'))
      assert.are.equal('  ' .. string.rep('─', TEXT_WIDTH), model.lines[4])
    end)

    it('lists code regions with preview lines and language', function()
      assert.are.same({ line_start = 2, line_end = 3, lang = 'lua' }, model.code_regions[1])
      assert.is_nil(model.code_regions[2].lang)
      assert.are.equal('python', model.code_regions[4].lang)
      assert.are.equal('sh', model.code_regions[5].lang)
    end)

    it('adds no token highlights of its own', function()
      local allowed = {
        KittyMarkdownCodeBg = true,
        KittyMarkdownCodeLang = true,
        KittyMarkdownCodeBorder = true,
      }
      for _, hl in ipairs(model.highlights) do
        if hl.line >= 1 and hl.line <= 4 then
          assert.is_true(allowed[hl.group] == true, hl.group .. ' on line ' .. hl.line)
        end
      end
    end)

    it('narrows a nested code box to the item width', function()
      local line = find_line(model, 'echo nested')
      assert.are.equal(
        '    echo nested' .. string.rep(' ', TEXT_WIDTH - 2 - #'echo nested'),
        model.lines[line]
      )
    end)
  end)

  describe('quotes and alerts (R-051, R-052)', function()
    local model
    before_each(function()
      model = build_fixture('quotes_alerts.md')
    end)

    it('shows a bar and the quote group', function()
      assert.are.equal('  ' .. icons.quote .. ' A quote line continues here.', model.lines[1])
      assert.is_true(has_group(model, 1, icons.quote, 'KittyMarkdownQuoteBar'))
      assert.is_true(has_group(model, 1, 'A quote line', 'KittyMarkdownQuote'))
      assert.are.equal('  ' .. icons.quote, model.lines[2])
      assert.are.equal('  ' .. icons.quote .. ' Second paragraph.', model.lines[3])
    end)

    it('adds a bar per nesting level', function()
      local line = find_line(model, 'Inner quote')
      assert.are.equal(
        '  ' .. icons.quote .. ' ' .. icons.quote .. ' Inner quote',
        model.lines[line]
      )
    end)

    it('shows alert title lines with icon, title case, and color', function()
      local kinds = {
        { 'NOTE', 'Note', 'KittyMarkdownAlertNote' },
        { 'TIP', 'Tip', 'KittyMarkdownAlertTip' },
        { 'IMPORTANT', 'Important', 'KittyMarkdownAlertImportant' },
        { 'WARNING', 'Warning', 'KittyMarkdownAlertWarning' },
        { 'CAUTION', 'Caution', 'KittyMarkdownAlertCaution' },
      }
      for _, kind in ipairs(kinds) do
        local title = icons.alert[kind[1]] .. ' ' .. kind[2]
        local line = find_line(model, title)
        assert.is_not_nil(line, kind[2])
        assert.are.equal('  ' .. icons.quote .. ' ' .. title, model.lines[line])
        assert.is_true(has_group(model, line, title, kind[3]))
        assert.is_true(has_group(model, line, icons.quote, kind[3]))
        assert.is_true(has_group(model, line + 1, 'body', 'KittyMarkdownQuote'))
      end
    end)
  end)

  describe('footnotes (R-053, R-054)', function()
    it('numbers references in order of first use and lists definitions last', function()
      local model = build_fixture('footnotes.md')
      assert.are.equal('  First ref¹ then second² and again¹.', model.lines[1])
      assert.is_true(has_group(model, 1, '¹', 'KittyMarkdownFootnote'))
      assert.are.equal('  ¹ Beta note with emph text.', model.lines[3])
      assert.are.equal('  ² Alpha note.', model.lines[5])
      assert.is_true(has_group(model, 3, 'emph', 'KittyMarkdownEmph'))
      assert.are.same({ { label = 'b', number = 1 }, { label = 'a', number = 2 } }, model.footnotes)
      assert.is_nil(find_line(model, 'Never referenced'))
    end)
  end)

  describe('rules, html, front matter (R-055, R-059, R-060)', function()
    it('draws a rule across the text width', function()
      local model = build_lines({ 'a', '', '---', '', 'b' })
      assert.are.equal('  ' .. string.rep(icons.rule, TEXT_WIDTH), model.lines[3])
      assert.is_true(has_group(model, 3, icons.rule, 'KittyMarkdownRule'))
    end)

    it('shows HTML blocks as muted plain text', function()
      local model = build_fixture('html.md')
      assert.are.equal('  <div class="x">', model.lines[1])
      assert.are.equal('    <p>block html</p>', model.lines[2])
      assert.is_true(has_group(model, 1, '<div', 'KittyMarkdownHtml'))
    end)

    it('hides front matter', function()
      local model = build_fixture('front_matter.md')
      assert.are.equal('  ' .. icons.h[1] .. ' Visible', model.lines[1])
      assert.is_nil(find_line(model, 'Hidden'))
      assert.are.equal(6, model.source_lines[1])
    end)
  end)

  describe('math (R-062, R-069, R-070)', function()
    it('shows the TeX source in the math group when render gives nil', function()
      local restore = stub_math(function()
        return nil
      end)
      local model = build_fixture('math.md')
      restore()
      assert.are.equal('  Inline E = mc^2 math.', model.lines[1])
      assert.are.equal('E = mc^2', span_of(model, 1, 'KittyMarkdownMath'))
    end)

    it('uses the rendered text when render gives one', function()
      local restore = stub_math(function(tex)
        return tex == '\\alpha' and 'α' or nil
      end)
      local model = build_fixture('math.md')
      restore()
      local line = find_line(model, 'Text')
      assert.are.equal('  Text α more.', model.lines[line])
    end)

    it('centers display math on its own line', function()
      local restore = stub_math(function(tex)
        return tex:match('^\\sum') and 'Σ i' or nil
      end)
      local model = build_fixture('math.md')
      restore()
      local line = find_line(model, 'Σ i')
      local pad = math.floor((TEXT_WIDTH - strwidth('Σ i')) / 2)
      assert.are.equal('  ' .. string.rep(' ', pad) .. 'Σ i', model.lines[line])
      assert.is_true(has_group(model, line, 'Σ i', 'KittyMarkdownMath'))
    end)
  end)

  describe('blank lines (R-061)', function()
    it('keeps one blank between blocks and none at the top', function()
      local model = build_fixture('blank_lines.md')
      assert.are.same({
        '  ' .. icons.h[1] .. ' After blanks',
        '',
        '  Paragraph one.',
        '',
        '  Paragraph two.',
        '',
        '  ' .. icons.bullet[1] .. ' list',
        '',
        '  Last.',
      }, model.lines)
    end)
  end)

  describe('pixel blocks (R-093, R-155, R-156, R-168, R-204)', function()
    local model
    before_each(function()
      model = build_fixture('images.md')
    end)

    it('uses one empty anchor line per block with a blank after it', function()
      assert.are.same({
        '  Before image.',
        '',
        '',
        '',
        '',
        '',
        '',
        '',
        '',
        '  Inline inside a paragraph.',
        '',
        '',
        '  After image.',
      }, model.lines)
      local anchors = {}
      for _, block in ipairs(model.blocks) do
        anchors[#anchors + 1] = block.line
      end
      assert.are.same({ 2, 4, 6, 8, 11 }, anchors)
    end)

    it('fills the descriptor and resolves paths against dir', function()
      local b = model.blocks[1]
      assert.are.equal('image', b.kind)
      assert.are.equal(1, b.index)
      assert.are.equal(3, b.src_line)
      assert.are.equal('/fixtures/img/local.png', b.source)
      assert.are.equal('Local alt', b.alt)
      assert.are.equal(TEXT_WIDTH, b.width_cells)
      assert.are.same({}, b.attrs)
      assert.is_nil(b.width_px)
      assert.is_nil(b.rows)
      assert.is_nil(b.image_id)
      assert.is_nil(b.text_only)
      assert.are.equal('/abs/pic.jpg', model.blocks[2].source)
      assert.are.equal('https://example.com/r.png', model.blocks[3].source)
      assert.are.same({ width = 120, height = 40 }, model.blocks[4].attrs)
    end)

    it('computes the block key from kind, source, width, theme, and attrs', function()
      local b = model.blocks[4]
      local expected = vim.fn.sha256(
        table.concat(
          { 'image', '/fixtures/tagged.png', '40', 'light', 'height=40,width=120' },
          '\0'
        )
      )
      assert.are.equal(expected, b.key)
      local other = build_fixture('images.md', { width = 44, theme = 'dark', dir = '/fixtures' })
      assert.are_not.equal(b.key, other.blocks[4].key)
    end)

    it('reserves rows from sizes keyed by block key', function()
      local key = model.blocks[1].key
      local sized = build_fixture('images.md', {
        width = 44,
        theme = 'light',
        dir = '/fixtures',
        cell = { width_px = 10, height_px = 20 },
        sizes = { [key] = { width_px = 400, height_px = 130, image_id = 77 } },
      })
      local b = sized.blocks[1]
      assert.are.equal(400, b.width_px)
      assert.are.equal(130, b.height_px)
      assert.are.equal(77, b.image_id)
      assert.are.equal(7, b.rows)
      assert.is_nil(sized.blocks[2].rows)
      assert.is_nil(sized.blocks[2].image_id)
    end)

    it('marks every pixel block text-only when images are off', function()
      local off = build_fixture(
        'images.md',
        { width = 44, theme = 'light', dir = '/fixtures', images = false }
      )
      for _, b in ipairs(off.blocks) do
        assert.is_true(b.text_only)
      end
    end)

    it('adds an image link on the anchor line', function()
      local found = {}
      for _, link in ipairs(model.links) do
        if link.kind == 'image' then
          found[#found + 1] = { link.line, link.target }
        end
      end
      assert.are.same({
        { 2, '/fixtures/img/local.png' },
        { 4, '/abs/pic.jpg' },
        { 6, 'https://example.com/r.png' },
        { 8, '/fixtures/tagged.png' },
        { 11, '/fixtures/inline.png' },
      }, found)
    end)

    it('falls back to the buffer directory when dir is absent', function()
      local blocks = parser.parse(load_fixture('images.md'))
      local m = layout.build(blocks, { width = 44, theme = 'light' })
      assert.are.equal(vim.fs.joinpath(fixtures, 'img/local.png'), m.blocks[1].source)
    end)

    it('describes mermaid blocks with the diagram source and type', function()
      local m = build_fixture('mermaid.md')
      assert.are.same({ '  Diagram follows.', '', '', '', '' }, m.lines)
      assert.are.equal('mermaid', m.blocks[1].kind)
      assert.are.equal('%% comment\nflowchart LR\n  A --> B', m.blocks[1].source)
      assert.are.equal('flowchart', m.blocks[1].alt)
      assert.are.equal('sequenceDiagram', m.blocks[2].alt)
      assert.are.equal(2, m.blocks[1].line)
      assert.are.equal(4, m.blocks[2].line)
      assert.are.same({}, m.links)
    end)
  end)

  describe('source lines (R-205)', function()
    it('maps preview lines to source lines without decreasing', function()
      local model = build_fixture('inline.md')
      assert.are.equal(#model.lines, #model.source_lines)
      assert.are.equal(1, model.source_lines[1])
      local hard = find_line(model, 'next line')
      assert.are.equal(4, model.source_lines[hard])
      local prev = 0
      for i, src in ipairs(model.source_lines) do
        assert.is_true(src >= prev, 'line ' .. i)
        prev = src
      end
    end)

    it('maps wrapped list text to the item line', function()
      local model = build_fixture('lists.md')
      local line = find_line(model, 'item with a long')
      assert.are.equal(18, model.source_lines[line])
      assert.are.equal(18, model.source_lines[line + 1])
    end)
  end)

  describe('determinism (R-157)', function()
    -- Regenerate with `KM_UPDATE_GOLDENS=1 nvim --clean -l tests/run.lua golden`,
    -- then run `stylua tests/fixtures/golden` so the files pass lint.
    local names = { 'headings', 'lists', 'tables', 'images' }
    for _, name in ipairs(names) do
      it('matches the golden model for ' .. name, function()
        local model = build_fixture(name .. '.md')
        local again = build_fixture(name .. '.md')
        assert.are.same(model, again)
        local path = vim.fs.joinpath(fixtures, 'golden', name .. '.lua')
        if vim.env.KM_UPDATE_GOLDENS == '1' then
          vim.fn.writefile(vim.split('return ' .. vim.inspect(model), '\n'), path)
        end
        local expected = dofile(path)
        assert.are.same(expected, model)
      end)
    end
  end)

  describe('installed parsers (R-209)', function()
    local root = vim.fs.dirname(vim.fs.dirname(vim.fn.fnamemodify(here, ':p')))
    local child = vim.fs.joinpath(fixtures, 'layout_child.lua')
    local docs = {}
    for name, kind in vim.fs.dir(fixtures) do
      if kind == 'file' and name:match('%.md$') then
        docs[#docs + 1] = vim.fs.joinpath(fixtures, name)
      end
    end
    table.sort(docs)

    local function run_child(rtp)
      local cmd = { vim.v.progpath, '--clean', '-l', child, root, rtp }
      vim.list_extend(cmd, docs)
      local res = vim.system(cmd, { text = true }):wait()
      assert.are.equal(0, res.code, res.stderr)
      return vim.json.decode(res.stdout)
    end

    local function expected_models()
      local out = {}
      for i, path in ipairs(docs) do
        local buf = vim.api.nvim_create_buf(false, true)
        vim.api.nvim_buf_set_lines(buf, 0, -1, false, vim.fn.readfile(path))
        out[i] = vim.inspect(layout.build(parser.parse(buf), OPTS))
      end
      return out
    end

    local function assert_same_models(got)
      local want = expected_models()
      assert.are.equal(#want, #got.models)
      for i = 1, #want do
        assert.are.equal(want[i], got.models[i], docs[i])
      end
    end

    it('gives the same model when no parser is on the runtimepath', function()
      local got = run_child(root .. ',' .. vim.env.VIMRUNTIME)
      for _, lib in ipairs(got.parsers) do
        assert.is_nil(lib:match('markdown'), 'parser left on the runtimepath: ' .. lib)
      end
      assert_same_models(got)
    end)

    it('gives the same model when only markdown_inline is missing', function()
      local dir = vim.fn.tempname()
      vim.fn.mkdir(dir .. '/parser', 'p')
      local bundled = vim.api.nvim_get_runtime_file('parser/markdown.*', true)
      assert.is_true(#bundled > 0)
      local src = bundled[#bundled]
      vim.uv.fs_copyfile(src, dir .. '/parser/' .. vim.fs.basename(src))
      local got = run_child(dir .. ',' .. root .. ',' .. vim.env.VIMRUNTIME)
      vim.fn.delete(dir, 'rf')
      assert_same_models(got)
    end)

    it('gives the same model with another markdown parser first on the runtimepath', function()
      -- A user parser (for example from nvim-treesitter) sits in the data
      -- site directory. Without one there is nothing to shadow the bundled
      -- parser, so the check only runs where one is installed.
      local site = vim.fs.joinpath(vim.fn.stdpath('data'), 'site')
      if not vim.uv.fs_stat(vim.fs.joinpath(site, 'parser', 'markdown.so')) then
        return
      end
      local got = run_child(site .. ',' .. root .. ',' .. vim.env.VIMRUNTIME)
      assert_same_models(got)
    end)
  end)

  describe('performance (R-023)', function()
    it('lays out a 1000-line document in under 150 ms', function()
      local chunk = {
        '## Section heading',
        '',
        'A paragraph with *emphasis*, **strong**, `code`, a [link](https://x.y/z), and',
        'enough words to wrap across more than one preview line at the text width.',
        '',
        '- item one',
        '- item two with :tada: and a footnote[^n]',
        '  - nested item',
        '',
        '| a | b |',
        '|---|---|',
        '| 1 | 2 |',
        '',
        '```lua',
        'local x = 1',
        '```',
        '',
        '> quoted text',
        '',
        '![img](pic.png)',
      }
      local lines = {}
      while #lines < 1000 do
        vim.list_extend(lines, chunk)
      end
      lines[#lines + 1] = '[^n]: note'
      local buf = vim.api.nvim_create_buf(false, true)
      vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
      vim.bo[buf].filetype = 'markdown'
      local start = vim.uv.hrtime()
      local model = layout.build(parser.parse(buf), OPTS)
      local ms = (vim.uv.hrtime() - start) / 1e6
      assert.is_true(#model.lines > 1000)
      assert.is_true(ms < 150, string.format('took %.1f ms', ms))
    end)
  end)
end)
