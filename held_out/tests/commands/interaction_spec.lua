local QA = require('qa')
local U = require('commands_util')
local a = QA.assert
local api = vim.api

-- Puts the cursor of the preview window on the first occurrence of
-- `needle` in the preview text. Returns the line.
local function cursor_on(p, needle)
  local line, text = U.find_line(p.buf, needle)
  a.not_nil(line, 'preview shows ' .. needle)
  local col = text:find(needle, 1, true) - 1
  api.nvim_set_current_win(p.win)
  api.nvim_win_set_cursor(p.win, { line, col + 1 })
  return line
end

local function press(keys)
  api.nvim_feedkeys(api.nvim_replace_termcodes(keys, true, false, true), 'xt', false)
  U.settle(50)
end

local function same_file(arg, expected)
  if type(arg) ~= 'string' then
    return false
  end
  arg = arg:gsub('^file://', '')
  local full = vim.fn.fnamemodify(arg, ':p')
  return (vim.uv.fs_realpath(full) or full) == (vim.uv.fs_realpath(expected) or expected)
end

QA.describe('interaction: q closes', function()
  QA.before_each(function()
    U.reset()
  end)

  for _, place in ipairs({ 'split', 'tab', 'float' }) do
    QA.check('R-158', 'Pressing q in the preview closes the preview', function()
      local buf = U.src({ '# Quit', '', 'Some text.' })
      vim.cmd('KittyMarkdown open ' .. place)
      local p = U.preview()
      api.nvim_set_current_win(p.win)
      press('q')
      U.settle(50)
      a.eq(0, #U.previews(), place .. ': preview closed')
      a.eq(0, #U.preview_bufs(), place .. ': preview buffer gone')
      a.truthy(api.nvim_buf_is_loaded(buf), place .. ': source still loaded')
    end)
  end

  QA.check(
    'R-158',
    'The q key is mapped only in the preview buffer, not globally or in the source',
    { severity = 'medium' },
    function()
      local buf = U.src({ '# Quit', '', 'Some text.' })
      vim.cmd('KittyMarkdown open split')
      local p = U.preview()
      local found = false
      for _, m in ipairs(api.nvim_buf_get_keymap(p.buf, 'n')) do
        if m.lhs == 'q' then
          found = true
        end
      end
      a.truthy(found, 'buffer-local q in the preview')
      for _, m in ipairs(api.nvim_buf_get_keymap(buf, 'n')) do
        a.ne('q', m.lhs, 'no q mapping in the source buffer')
      end
      for _, m in ipairs(api.nvim_get_keymap('n')) do
        a.ne('q', m.lhs, 'no global q mapping')
      end
    end
  )

  QA.check(
    'R-158',
    'After q closes the preview, open shows a fresh preview again',
    { severity = 'medium' },
    function()
      U.src({ '# Again', '', 'Body text.' })
      vim.cmd('KittyMarkdown open float')
      local p = U.preview()
      api.nvim_set_current_win(p.win)
      press('q')
      U.settle(50)
      vim.cmd('KittyMarkdown open float')
      local q = U.preview()
      a.not_nil(q, 'preview reopened')
      a.contains('Body text.', U.text(q.buf))
    end
  )
end)

QA.describe('interaction: gx follows links', function()
  QA.before_each(function()
    U.reset()
  end)

  QA.check(
    'R-159',
    'gx on the text of an inline link opens its URL with the system opener',
    function()
      local opened = U.stub_open()
      U.src({ '# Links', '', 'Visit [the project site](https://example.com/some/page?x=1) today.' })
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      cursor_on(p, 'project site')
      press('gx')
      a.eq({ 'https://example.com/some/page?x=1' }, opened)
    end
  )

  QA.check(
    'R-159',
    'gx on an autolink URL opens that URL with the system opener',
    { severity = 'medium' },
    function()
      local opened = U.stub_open()
      U.src({ '# Auto', '', 'See <https://example.org/auto/link> for more.' })
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      cursor_on(p, 'example.org/auto')
      press('gx')
      a.eq({ 'https://example.org/auto/link' }, opened)
    end
  )

  QA.check(
    'R-159',
    'gx on a reference-style link opens the URL of its definition',
    { severity = 'medium' },
    function()
      local opened = U.stub_open()
      U.src({
        '# Ref',
        '',
        'Read the [reference text][ref] now.',
        '',
        '[ref]: https://example.net/ref-target',
      })
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      cursor_on(p, 'reference text')
      press('gx')
      a.eq({ 'https://example.net/ref-target' }, opened)
    end
  )

  QA.check(
    'R-159',
    'gx on any character of a link text, the first and last included, opens the link',
    { severity = 'low' },
    function()
      local opened = U.stub_open()
      U.src({ '# Edge', '', 'Go [endlink](https://example.com/end) now.' })
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      local line, text = U.find_line(p.buf, 'endlink')
      local s = text:find('endlink', 1, true)
      api.nvim_set_current_win(p.win)
      api.nvim_win_set_cursor(p.win, { line, s - 1 + #'endlink' - 1 })
      press('gx')
      a.eq({ 'https://example.com/end' }, opened)
    end
  )

  QA.check(
    { 'R-160', 'R-344' },
    'gx on a link to a local file opens that file, resolved from the document directory',
    function()
      local opened = U.stub_open()
      local dir = QA.tmpdir()
      QA.write(dir .. '/notes.txt', 'notes\n')
      U.src({ '# Files', '', 'Open [the notes file](notes.txt) please.' }, { dir = dir })
      local elsewhere = QA.tmpdir()
      vim.fn.chdir(elsewhere)
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      cursor_on(p, 'notes file')
      press('gx')
      vim.fn.chdir(QA.root)
      a.eq(1, #opened, 'one call to the system opener')
      a.truthy(same_file(opened[1], dir .. '/notes.txt'), 'opened ' .. vim.inspect(opened[1]))
    end
  )

  QA.check(
    'R-160',
    'gx on a link to a file in a subdirectory opens that file',
    { severity = 'medium' },
    function()
      local opened = U.stub_open()
      local dir = QA.tmpdir()
      QA.write(dir .. '/sub/deep/info.md', '# info\n')
      U.src({ '# Files', '', 'See [deep info](sub/deep/info.md).' }, { dir = dir })
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      cursor_on(p, 'deep info')
      press('gx')
      a.eq(1, #opened)
      a.truthy(
        same_file(opened[1], dir .. '/sub/deep/info.md'),
        'opened ' .. vim.inspect(opened[1])
      )
    end
  )

  QA.check(
    'R-344',
    'gx on a relative file link in a source without a file name resolves it against the current directory',
    function()
      local opened = U.stub_open()
      local cwd = QA.tmpdir()
      cwd = vim.uv.fs_realpath(cwd) or cwd
      QA.write(cwd .. '/here.txt', 'here\n')
      vim.fn.chdir(cwd)
      local buf = api.nvim_create_buf(true, false)
      api.nvim_buf_set_lines(
        buf,
        0,
        -1,
        false,
        { '# No name', '', 'Open [the here file](here.txt).' }
      )
      api.nvim_set_current_buf(buf)
      vim.bo[buf].filetype = 'markdown'
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      cursor_on(p, 'here file')
      press('gx')
      a.eq(1, #opened, 'one call to the system opener')
      local ok = same_file(opened[1], cwd .. '/here.txt')
      vim.fn.chdir(QA.root)
      a.truthy(ok, 'opened ' .. vim.inspect(opened[1]))
    end
  )

  QA.check(
    'R-344',
    'gx on a relative file link uses the source directory even after the current directory changes',
    { severity = 'medium' },
    function()
      local opened = U.stub_open()
      local dir = QA.tmpdir()
      QA.write(dir .. '/doc-side.txt', 'x\n')
      U.src({ '# Files', '', 'Open [side file](doc-side.txt).' }, { dir = dir })
      vim.cmd('KittyMarkdown open tab')
      local other = QA.tmpdir()
      QA.write(other .. '/doc-side.txt', 'decoy\n')
      vim.fn.chdir(other)
      local p = U.preview()
      cursor_on(p, 'side file')
      press('gx')
      vim.fn.chdir(QA.root)
      a.eq(1, #opened)
      a.truthy(same_file(opened[1], dir .. '/doc-side.txt'), 'opened ' .. vim.inspect(opened[1]))
    end
  )

  QA.check('R-160', 'gx on an image in the preview opens the image file', function()
    local opened = U.stub_open()
    local dir = QA.tmpdir()
    QA.make_png(dir .. '/picture.png', 20, 10)
    U.src(
      { '# Img', '', 'Before it.', '', '![a picture](picture.png)', '', 'After it.' },
      { dir = dir }
    )
    vim.fn.chdir(QA.tmpdir())
    vim.cmd('KittyMarkdown open tab')
    local p = U.preview()
    U.settle(50)
    local vt = U.virt(p.buf)
    a.eq(1, #vt, 'one image box')
    api.nvim_set_current_win(p.win)
    api.nvim_win_set_cursor(p.win, { vt[1].row, 0 })
    press('gx')
    vim.fn.chdir(QA.root)
    a.eq(1, #opened, 'one call to the system opener')
    a.truthy(same_file(opened[1], dir .. '/picture.png'), 'opened ' .. vim.inspect(opened[1]))
  end)

  QA.check(
    'R-161',
    'gx on a link to a heading anchor scrolls the preview to that heading',
    function()
      local opened = U.stub_open()
      local lines = { '# Top', '', 'Jump: [go to the far part](#far-away-heading).', '' }
      for i = 1, 120 do
        lines[#lines + 1] = 'Filler paragraph number ' .. i .. '.'
        lines[#lines + 1] = ''
      end
      lines[#lines + 1] = '## Far Away Heading'
      lines[#lines + 1] = ''
      lines[#lines + 1] = 'Text under the far heading.'
      U.src(lines)
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      local target = U.find_line(p.buf, 'Far Away Heading')
      cursor_on(p, 'far part')
      press('gx')
      local top = U.topline(p.win)
      local h = api.nvim_win_get_height(p.win)
      a.truthy(
        top <= target and target < top + h,
        ('heading line %d visible in %d..%d'):format(target, top, top + h - 1)
      )
      a.ne(1, top, 'the preview scrolled')
      a.eq({}, opened, 'the system opener is not used for an anchor')
    end
  )

  QA.check(
    'R-161',
    'gx on a heading anchor follows the GitHub slug of a heading with punctuation and case',
    { severity = 'medium' },
    function()
      U.stub_open()
      local lines = { '# Top', '', 'Jump to [the setup part](#setup--use-part-2).', '' }
      for i = 1, 100 do
        lines[#lines + 1] = 'Line of filler ' .. i .. '.'
        lines[#lines + 1] = ''
      end
      lines[#lines + 1] = '## Setup & Use: Part 2'
      lines[#lines + 1] = ''
      lines[#lines + 1] = 'Body.'
      U.src(lines)
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      local target = U.find_line(p.buf, 'Use: Part 2')
      cursor_on(p, 'setup part')
      press('gx')
      local top = U.topline(p.win)
      local h = api.nvim_win_get_height(p.win)
      a.truthy(
        top <= target and target < top + h,
        ('heading line %d visible in %d..%d'):format(target, top, top + h - 1)
      )
    end
  )

  QA.check(
    'R-161',
    'gx on a heading anchor that points up the document scrolls back to the heading',
    { severity = 'medium' },
    function()
      U.stub_open()
      local lines = { '# Start Here', '' }
      for i = 1, 100 do
        lines[#lines + 1] = 'Filler ' .. i .. '.'
        lines[#lines + 1] = ''
      end
      lines[#lines + 1] = 'Back to [the beginning](#start-here).'
      U.src(lines)
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      cursor_on(p, 'the beginning')
      U.settle(20)
      press('gx')
      local target = U.find_line(p.buf, 'Start Here')
      local top = U.topline(p.win)
      a.truthy(top <= target, 'heading visible at the top: top=' .. top)
    end
  )
end)

-- A long document of sections. Each section has a heading and a paragraph
-- of several source lines whose first word is unique.
local function sections(n, para_lines)
  local lines = { '# Scroll Document', '' }
  local where = {}
  for i = 1, n do
    where[i] = { heading = #lines + 1 }
    lines[#lines + 1] = '## Section ' .. i
    lines[#lines + 1] = ''
    where[i].para = #lines + 1
    for k = 1, para_lines do
      if k == 1 then
        lines[#lines + 1] = 'Start' .. i .. 'word begins this paragraph.'
      else
        lines[#lines + 1] = 'continuation line ' .. k .. ' of section ' .. i .. '.'
      end
    end
    where[i].para_last = #lines
    lines[#lines + 1] = ''
  end
  return lines, where
end

-- Moves the source cursor and lets the debounced sync run.
local function move(src_win, line)
  api.nvim_set_current_win(src_win)
  api.nvim_win_set_cursor(src_win, { line, 0 })
  api.nvim_exec_autocmds('CursorMoved', { buffer = api.nvim_win_get_buf(src_win) })
  U.settle(350)
end

-- R-343: floor(height / 3) window lines above the target, or fewer when
-- the window is at the top of the buffer.
local function in_top_third(p, target)
  local top = U.topline(p.win)
  local h = api.nvim_win_get_height(p.win)
  local want = math.floor(h / 3)
  local above = target - top
  return above == want or (top == 1 and above >= 0 and above <= want), top, h
end

QA.describe('interaction: scroll sync', function()
  QA.before_each(function()
    U.reset({ placement = 'split' })
  end)

  QA.check(
    'R-162',
    'In a split, moving the source cursor scrolls the preview so the block under it is visible',
    function()
      local lines, where = sections(40, 3)
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      move(src_win, where[30].heading)
      local target = U.find_line(p.buf, 'Section 30')
      local top = U.topline(p.win)
      local h = api.nvim_win_get_height(p.win)
      a.truthy(
        top <= target and target < top + h,
        ('block line %d visible in %d..%d'):format(target, top, top + h - 1)
      )
      move(src_win, where[5].para + 1)
      target = U.find_line(p.buf, 'Start5word')
      top = U.topline(p.win)
      a.truthy(
        top <= target and target < top + h,
        ('moving back up: line %d visible in %d..%d'):format(target, top, top + h - 1)
      )
    end
  )

  QA.check(
    'R-162',
    'Scroll sync keeps the focus in the source window',
    { severity = 'medium' },
    function()
      local lines, where = sections(30, 2)
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      move(src_win, where[25].heading)
      a.eq(src_win, api.nvim_get_current_win())
    end
  )

  QA.check(
    'R-162',
    'Scroll sync follows the cursor into a fenced code block',
    { severity = 'medium' },
    function()
      local lines = { '# Code', '' }
      for i = 1, 60 do
        lines[#lines + 1] = 'Para ' .. i .. '.'
        lines[#lines + 1] = ''
      end
      local fence = #lines + 1
      vim.list_extend(lines, { '```', 'codeblockmarker = 1', 'two = 2', 'three = 3', '```', '' })
      for i = 1, 30 do
        lines[#lines + 1] = 'Tail ' .. i .. '.'
        lines[#lines + 1] = ''
      end
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      move(src_win, fence + 2)
      local target = U.find_line(p.buf, 'codeblockmarker')
      local top = U.topline(p.win)
      local h = api.nvim_win_get_height(p.win)
      a.truthy(
        top <= target and target < top + h,
        ('code line %d visible in %d..%d'):format(target, top, top + h - 1)
      )
    end
  )

  QA.check(
    { 'R-211', 'R-343' },
    'Scroll sync leaves the window height divided by 3, rounded down, lines above the target',
    function()
      local lines, where = sections(40, 3)
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      for _, i in ipairs({ 20, 33, 12 }) do
        move(src_win, where[i].heading)
        local target = U.find_line(p.buf, 'Section ' .. i)
        local ok, top, h = in_top_third(p, target)
        a.truthy(ok, ('section %d: line %d, top %d, height %d'):format(i, target, top, h))
      end
    end
  )

  QA.check(
    { 'R-211', 'R-343' },
    'Scroll sync leaves the specified number of lines above the target after the preview window is resized',
    function()
      local lines, where = sections(40, 3)
      U.src(lines)
      -- a horizontal split above, so the row that holds the source and the
      -- preview can take any height
      vim.cmd('split')
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      api.nvim_win_set_height(p.win, 18)
      local h = api.nvim_win_get_height(p.win)
      QA.need(h % 3 == 0, 'a preview window height divisible by three')
      move(src_win, where[25].heading)
      local target = U.find_line(p.buf, 'Section 25')
      local ok, top = in_top_third(p, target)
      a.truthy(
        ok,
        ('line %d, top %d, height %d: row %d of %d'):format(target, top, h, target - top + 1, h)
      )
    end
  )

  QA.check(
    'R-248',
    'Scroll sync targets the first line of the block under the cursor, not the cursor line inside it',
    function()
      -- one paragraph taller than the window
      local lines, where = sections(30, 3)
      local long_at = where[20].para
      local long = { 'Longpara begins here with its first sentence.' }
      for k = 1, 60 do
        long[#long + 1] = 'more sentence number ' .. k .. ' inside the long paragraph body.'
      end
      for k = where[20].para_last, where[20].para, -1 do
        table.remove(lines, k)
      end
      for k = #long, 1, -1 do
        table.insert(lines, long_at, long[k])
      end
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      move(src_win, long_at + 50)
      local first = U.find_line(p.buf, 'Longpara begins')
      local ok, top, h = in_top_third(p, first)
      a.truthy(ok, ('first block line %d, top %d, height %d'):format(first, top, h))
    end
  )

  QA.check(
    'R-248',
    'Scroll sync on an item inside a long list targets the first line of the list',
    { severity = 'medium' },
    function()
      local lines, where = sections(30, 2)
      local at = where[20].para_last + 2
      local list = { '- Listhead first item of the long list' }
      for k = 2, 40 do
        list[#list + 1] = '- list item number ' .. k
      end
      list[#list + 1] = ''
      for k = #list, 1, -1 do
        table.insert(lines, at, list[k])
      end
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      move(src_win, at + 30)
      local first = U.find_line(p.buf, 'Listhead first item')
      local ok, top, h = in_top_third(p, first)
      a.truthy(ok, ('first block line %d, top %d, height %d'):format(first, top, h))
    end
  )

  QA.check(
    'R-340',
    'With the source cursor on a blank line between two blocks, scroll sync targets the block above',
    function()
      local lines, where = sections(40, 1)
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      -- the blank line after the paragraph of a section, before the next heading
      move(src_win, where[24].para_last + 1)
      local first = U.find_line(p.buf, 'Start24word')
      local ok, top, h = in_top_third(p, first)
      a.truthy(ok, ('paragraph above: line %d, top %d, height %d'):format(first, top, h))
      -- the blank line between a heading and its paragraph
      move(src_win, where[31].heading + 1)
      local head = U.find_line(p.buf, 'Section 31')
      ok, top, h = in_top_third(p, head)
      a.truthy(ok, ('heading above: line %d, top %d, height %d'):format(head, top, h))
    end
  )

  QA.check(
    'R-340',
    'With the source cursor on one of several blank lines between blocks, scroll sync targets the block above',
    { severity = 'medium' },
    function()
      local lines, where = sections(40, 1)
      table.insert(lines, where[27].para_last + 1, '')
      table.insert(lines, where[27].para_last + 1, '')
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      move(src_win, where[27].para_last + 3)
      local first = U.find_line(p.buf, 'Start27word')
      local ok, top, h = in_top_third(p, first)
      a.truthy(ok, ('line %d, top %d, height %d'):format(first, top, h))
    end
  )

  QA.check(
    'R-341',
    'With the source cursor in hidden front matter, scroll sync moves the preview to line 1',
    function()
      local body = sections(40, 3)
      local lines = { '---', 'title: Front', 'tags: [a, b]', 'draft: true', '---', '' }
      vim.list_extend(lines, body)
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      move(src_win, 200)
      a.ne(1, U.topline(p.win), 'the preview scrolled down first')
      move(src_win, 3)
      a.eq(1, U.topline(p.win), 'preview top line')
      move(src_win, 200)
      move(src_win, 1)
      a.eq(1, U.topline(p.win), 'opening fence line')
    end
  )

  QA.check('R-164', 'A tab preview does not scroll when the source cursor moves', function()
    U.reset({ placement = 'tab' })
    local lines, where = sections(40, 3)
    local buf = U.src(lines)
    vim.cmd('KittyMarkdown open')
    local p = U.preview()
    local top0 = U.topline(p.win)
    vim.cmd('tabfirst')
    local src_win = vim.fn.bufwinid(buf)
    move(src_win, where[35].heading)
    move(src_win, where[25].para)
    a.eq(top0, U.topline(p.win), 'tab preview top line')
  end)

  QA.check('R-164', 'A float preview does not scroll when the source cursor moves', function()
    U.reset({ placement = 'float' })
    local lines, where = sections(40, 3)
    local buf = U.src(lines)
    local src_win = api.nvim_get_current_win()
    vim.cmd('KittyMarkdown open')
    local p = U.preview()
    local top0 = U.topline(p.win)
    a.eq(buf, api.nvim_win_get_buf(src_win))
    move(src_win, where[35].heading)
    move(src_win, where[22].para)
    a.eq(top0, U.topline(p.win), 'float preview top line')
  end)

  QA.check(
    'R-164',
    'A split preview reopened as a tab stops syncing',
    { severity = 'medium' },
    function()
      local lines, where = sections(40, 3)
      local buf = U.src(lines)
      vim.cmd('KittyMarkdown open split')
      vim.cmd('KittyMarkdown close')
      U.settle(50)
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      local top0 = U.topline(p.win)
      vim.cmd('tabfirst')
      move(vim.fn.bufwinid(buf), where[30].heading)
      a.eq(top0, U.topline(p.win))
    end
  )
end)
