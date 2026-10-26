local QA = require('qa')
local U = require('commands_util')
local a = QA.assert
local api = vim.api

local DOC = {
  '# Refresh Doc',
  '',
  'Original paragraph text.',
  '',
  '- a list item',
}

-- The source window of `buf`, made current.
local function to_source(buf)
  for _, w in ipairs(api.nvim_list_wins()) do
    if api.nvim_win_get_buf(w) == buf then
      api.nvim_set_current_win(w)
      return w
    end
  end
end

-- Number of virtual line blocks that show a picture.
local function placed(pbuf)
  local n = 0
  for _, v in ipairs(U.virt(pbuf)) do
    if table.concat(v.rows):find(U.PLACEHOLDER, 1, true) then
      n = n + 1
    end
  end
  return n
end

local function wait_placed(pbuf, n, ms)
  return QA.wait(ms or 8000, function()
    return api.nvim_buf_is_valid(pbuf) and placed(pbuf) == n
  end)
end

QA.describe('refresh: write and text changes', function()
  QA.before_each(function()
    U.reset({ placement = 'split' })
  end)

  QA.check('R-016', 'Writing the source buffer re-renders the preview with the new text', function()
    local buf = U.src(DOC)
    vim.cmd('KittyMarkdown open')
    local p = U.preview()
    to_source(buf)
    api.nvim_buf_set_lines(buf, 2, 3, false, { 'Replaced paragraph after edit.' })
    U.write(buf)
    a.truthy(
      QA.wait(1000, function()
        return U.text(p.buf):find('Replaced paragraph after edit.', 1, true) ~= nil
      end),
      'preview shows the written text'
    )
    a.not_contains('Original paragraph text.', U.text(p.buf))
  end)

  QA.check('R-016', 'Each later write re-renders the preview again', function()
    local buf = U.src(DOC)
    vim.cmd('KittyMarkdown open')
    local p = U.preview()
    for i = 1, 3 do
      to_source(buf)
      api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'Write number ' .. i .. ' marker.' })
      U.write(buf)
      a.truthy(
        QA.wait(1000, function()
          return U.text(p.buf):find('Write number ' .. i .. ' marker.', 1, true) ~= nil
        end),
        'write ' .. i
      )
    end
  end)

  QA.check(
    'R-016',
    'A write re-renders a tab or float preview that is not visible from the source window',
    { severity = 'medium' },
    function()
      U.reset({ placement = 'tab' })
      local buf = U.src(DOC)
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      vim.cmd('tabfirst')
      api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'Written while hidden.' })
      U.write(buf)
      a.truthy(QA.wait(1000, function()
        return U.text(p.buf):find('Written while hidden.', 1, true) ~= nil
      end))
    end
  )

  QA.check('R-018', 'A text change without a write does not re-render the preview', function()
    local buf = U.src(DOC)
    vim.cmd('KittyMarkdown open')
    local p = U.preview()
    local count = U.count_layout()
    local before = U.lines(p.buf)
    to_source(buf)
    api.nvim_buf_set_lines(buf, 2, 3, false, { 'Changed but not written.' })
    api.nvim_exec_autocmds('TextChanged', { buffer = buf })
    U.settle(600)
    a.eq(before, U.lines(p.buf), 'preview lines unchanged')
    a.eq(0, count(), 'no layout run for an unwritten change')
  end)

  QA.check('R-018', 'Typing in insert mode in the source does not re-render the preview', function()
    local buf = U.src(DOC)
    vim.cmd('KittyMarkdown open')
    local p = U.preview()
    local before = U.lines(p.buf)
    local count = U.count_layout()
    to_source(buf)
    api.nvim_win_set_cursor(0, { 3, 0 })
    api.nvim_feedkeys('Atyped words\27', 'xt', false)
    api.nvim_exec_autocmds('TextChangedI', { buffer = buf })
    api.nvim_exec_autocmds('InsertLeave', { buffer = buf })
    api.nvim_exec_autocmds('TextChanged', { buffer = buf })
    U.settle(600)
    a.contains('typed words', table.concat(U.lines(buf), '\n'), 'the keys reached the source')
    a.eq(before, U.lines(p.buf), 'preview lines unchanged')
    a.eq(0, count(), 'no layout run')
  end)

  QA.check(
    { 'R-016', 'R-018' },
    'A write after unwritten edits shows all of them at once',
    { severity = 'medium' },
    function()
      local buf = U.src(DOC)
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      to_source(buf)
      api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'First pending edit.' })
      api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'Second pending edit.' })
      U.settle(300)
      a.not_contains('First pending edit.', U.text(p.buf))
      U.write(buf)
      a.truthy(QA.wait(1000, function()
        local t = U.text(p.buf)
        return t:find('First pending edit.', 1, true) and t:find('Second pending edit.', 1, true)
      end))
    end
  )
end)

QA.describe('refresh: the refresh subcommand', function()
  QA.before_each(function()
    U.reset({ placement = 'split' })
  end)

  QA.check('R-017', 'Refresh re-renders the preview from the current buffer text', function()
    local buf = U.src(DOC)
    vim.cmd('KittyMarkdown open')
    local p = U.preview()
    to_source(buf)
    api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'Shown after refresh only.' })
    U.settle(100)
    a.not_contains('Shown after refresh only.', U.text(p.buf))
    vim.cmd('KittyMarkdown refresh')
    a.truthy(QA.wait(1000, function()
      return U.text(p.buf):find('Shown after refresh only.', 1, true) ~= nil
    end))
  end)

  QA.check('R-017', 'Refresh sends a new kitty graphics query to the terminal', function()
    local cap = U.image_mode()
    local buf = U.src(DOC)
    vim.cmd('KittyMarkdown open')
    U.settle(50)
    cap:clear()
    to_source(buf)
    vim.cmd('KittyMarkdown refresh')
    QA.answer_probe(true)
    U.settle(100)
    a.ge(1, U.queries(cap), 'graphics query after refresh')
  end)

  QA.check('R-017', 'Refresh transmits every placed image again', function()
    local cap = U.image_mode()
    local dir = QA.tmpdir()
    QA.make_png(dir .. '/one.png', 60, 30)
    QA.make_png(dir .. '/two.png', 30, 60)
    local buf = U.src(
      { '# Imgs', '', 'x', '', '![one](one.png)', '', 'y', '', '![two](two.png)' },
      { dir = dir }
    )
    vim.cmd('KittyMarkdown open')
    local p = U.preview()
    a.truthy(wait_placed(p.buf, 2), 'both pictures placed')
    local ids = U.transmits(cap)
    a.eq(2, #ids, 'one transmit per picture before refresh')
    cap:clear()
    to_source(buf)
    vim.cmd('KittyMarkdown refresh')
    QA.answer_probe(true)
    QA.wait(3000, function()
      return #U.transmits(cap) >= 2 and placed(p.buf) == 2
    end)
    local again = U.transmits(cap)
    a.eq(2, #again, 'every placed picture transmitted again')
    a.truthy(wait_placed(p.buf, 2), 'pictures still shown after refresh')
  end)

  QA.check(
    'R-017',
    'Refresh transmits the images of every open preview, not only the preview of the current source',
    function()
      U.reset({ placement = 'tab' })
      local cap = U.image_mode()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/first.png', 60, 30)
      QA.make_png(dir .. '/second.png', 30, 60, { 0, 120, 200, 255 })
      local one = U.src({ 'One.', '', '![first](first.png)' }, { dir = dir, name = 'one.md' })
      vim.cmd('KittyMarkdown open')
      vim.cmd('tabfirst')
      U.src({ 'Two.', '', '![second](second.png)' }, { dir = dir, name = 'two.md' })
      vim.cmd('KittyMarkdown open')
      local bufs = U.preview_bufs()
      a.eq(2, #bufs, 'two previews')
      a.truthy(
        QA.wait(8000, function()
          return placed(bufs[1]) == 1 and placed(bufs[2]) == 1
        end),
        'both pictures placed'
      )
      local ids = U.transmits(cap)
      a.eq(2, #ids, 'two pictures transmitted')
      cap:clear()
      vim.cmd('tabfirst')
      api.nvim_set_current_buf(one)
      vim.cmd('KittyMarkdown refresh')
      QA.answer_probe(true)
      QA.wait(3000, function()
        return #U.transmits(cap) >= 2
      end)
      local again = U.transmits(cap)
      for _, id in ipairs(ids) do
        a.contains(id, again, 'image ' .. id .. ' transmitted again')
      end
    end
  )

  QA.check(
    'R-017',
    'Refresh re-renders the preview of the current source and leaves another preview text as it was',
    { severity = 'medium' },
    function()
      U.reset({ placement = 'tab' })
      local one = U.src({ '# One', '', 'First body.' }, { name = 'one.md' })
      vim.cmd('KittyMarkdown open')
      local p1 = U.preview()
      vim.cmd('tabfirst')
      local two = U.src({ '# Two', '', 'Second body.' }, { name = 'two.md' })
      vim.cmd('KittyMarkdown open')
      api.nvim_buf_set_lines(one, -1, -1, false, { '', 'Pending in one.' })
      api.nvim_buf_set_lines(two, -1, -1, false, { '', 'Pending in two.' })
      vim.cmd('tabfirst')
      api.nvim_set_current_buf(two)
      vim.cmd('KittyMarkdown refresh')
      U.settle(100)
      local p2
      for _, b in ipairs(U.preview_bufs()) do
        if b ~= p1.buf then
          p2 = b
        end
      end
      a.contains('Pending in two.', U.text(p2), 'preview of the current source re-rendered')
      a.not_contains('Pending in one.', U.text(p1.buf), 'the other preview text is not re-rendered')
    end
  )
end)

QA.describe('refresh: pixel blocks while rendering', function()
  QA.before_each(function()
    U.reset({ placement = 'split' })
  end)

  -- A document with a remote picture that the local server serves after a
  -- delay, so the block stays pending for a while.
  local function slow_doc(delay)
    local dir = QA.tmpdir()
    local png = QA.make_png(dir .. '/served.png', 64, 32)
    local srv = U.http_server(QA.read(png), delay)
    local lines = {
      '# Slow Picture Doc',
      '',
      'Paragraph above the picture.',
      '',
      '![remote alt words](' .. srv.url .. '/pic.png)',
      '',
      'Paragraph below the picture.',
    }
    return lines, dir, srv
  end

  QA.check(
    'R-019',
    'The document text shows in the preview before a slow pixel block is ready',
    function()
      U.image_mode()
      local lines, dir = slow_doc(1500)
      U.src(lines, { dir = dir })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      local shown = QA.wait(400, function()
        local t = U.text(p.buf)
        return t:find('Paragraph above the picture.', 1, true)
          and t:find('Paragraph below the picture.', 1, true)
      end, 5)
      a.truthy(shown, 'text shown')
      a.falsy(U.has_placeholders(p.buf), 'the picture is not ready yet')
    end
  )

  QA.check(
    'R-020',
    'A pending pixel block shows a loading box with its alt text and the word loading',
    function()
      U.image_mode()
      local lines, dir = slow_doc(1500)
      U.src(lines, { dir = dir })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      U.settle(100)
      local vt = U.virt(p.buf)
      a.eq(1, #vt, 'one virtual block')
      local box = table.concat(vt[1].rows, '\n')
      a.contains('loading', box)
      a.contains('remote alt words', box)
    end
  )

  QA.check(
    'R-020',
    'The loading box is a bordered box three rows tall',
    { severity = 'medium' },
    function()
      U.image_mode()
      local lines, dir = slow_doc(1500)
      U.src(lines, { dir = dir })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      U.settle(100)
      local vt = U.virt(p.buf)
      a.eq(1, #vt)
      a.eq(3, #vt[1].rows, 'box rows')
      local first, last = vt[1].rows[1], vt[1].rows[3]
      a.truthy(
        first:find('[─━═%-┌╭+]') and last:find('[─━═%-└╰+]'),
        'top and bottom borders: ' .. vim.inspect(vt[1].rows)
      )
    end
  )

  QA.check(
    'R-020',
    'A pending mermaid block shows a loading box with the diagram type',
    { severity = 'medium' },
    function()
      U.image_mode()
      local buf =
        U.src({ '# M', '', 'before', '', '```mermaid', 'flowchart LR', '  A --> B', '```' })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      local vt = U.virt(p.buf)
      a.eq(1, #vt, 'one virtual block right after open')
      local box = table.concat(vt[1].rows, '\n')
      if not U.has_placeholders(p.buf) then
        a.contains('loading', box)
        a.truthy(
          box:find('flowchart', 1, true) or box:find('mermaid', 1, true),
          'diagram type in the box: ' .. box
        )
      end
      to_source(buf)
    end
  )

  QA.check(
    'R-021',
    'When the picture arrives, it replaces the loading box and the text lines stay the same',
    function()
      U.image_mode()
      local lines, dir = slow_doc(600)
      U.src(lines, { dir = dir })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      U.settle(50)
      local before = U.lines(p.buf)
      local box_row = U.virt(p.buf)[1].row
      a.truthy(
        QA.wait(8000, function()
          return U.has_placeholders(p.buf)
        end),
        'picture shown'
      )
      a.eq(before, U.lines(p.buf), 'buffer lines unchanged')
      local vt = U.virt(p.buf)
      a.eq(1, #vt, 'one virtual block')
      a.eq(box_row, vt[1].row, 'same anchor line')
      a.not_contains('loading', table.concat(vt[1].rows, '\n'), 'loading box gone')
    end
  )

  QA.check(
    'R-022',
    'Opening does not wait for a slow pixel block, and source input is handled meanwhile',
    function()
      U.image_mode()
      local lines, dir = slow_doc(2000)
      local buf = U.src(lines, { dir = dir })
      local t0 = vim.uv.hrtime()
      vim.cmd('KittyMarkdown open')
      local ms = (vim.uv.hrtime() - t0) / 1e6
      a.le(1000, ms, 'open returned while the picture was pending (ms)')
      local p = U.preview()
      to_source(buf)
      api.nvim_win_set_cursor(0, { 3, 0 })
      local t1 = vim.uv.hrtime()
      api.nvim_feedkeys('Aextra typed\27', 'xt', false)
      local typed_ms = (vim.uv.hrtime() - t1) / 1e6
      a.contains('extra typed', table.concat(U.lines(buf), '\n'))
      a.le(500, typed_ms, 'keys handled quickly (ms)')
      a.falsy(U.has_placeholders(p.buf), 'the picture was still pending while typing')
    end
  )

  QA.check(
    'R-019',
    'After a write that adds a slow pixel block, the new text shows before the picture is ready',
    function()
      U.image_mode()
      local dir = QA.tmpdir()
      local png = QA.make_png(dir .. '/served.png', 64, 32)
      local srv = U.http_server(QA.read(png), 1500)
      local buf = U.src({ '# Doc', '', 'Old text.' }, { dir = dir })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      to_source(buf)
      api.nvim_buf_set_lines(
        buf,
        -1,
        -1,
        false,
        { '', '![late](' .. srv.url .. '/late.png)', '', 'Text after the late picture.' }
      )
      local t0 = vim.uv.hrtime()
      U.write(buf)
      local shown = QA.wait(400, function()
        return U.text(p.buf):find('Text after the late picture.', 1, true) ~= nil
      end, 5)
      a.truthy(shown, 'new text shown')
      a.le(1000, (vim.uv.hrtime() - t0) / 1e6, 'write returned before the picture (ms)')
      a.falsy(U.has_placeholders(p.buf), 'picture not ready yet')
    end
  )

  QA.check(
    'R-021',
    'When one picture arrives, other pending blocks keep their loading boxes and the text stays the same',
    { severity = 'medium' },
    function()
      U.image_mode()
      local dir = QA.tmpdir()
      local png = QA.make_png(dir .. '/served.png', 64, 32)
      local slow = U.http_server(QA.read(png), 2500)
      QA.make_png(dir .. '/fast.png', 40, 20)
      U.src({
        'Top text.',
        '',
        '![fast one](fast.png)',
        '',
        'Middle text.',
        '',
        '![slow one](' .. slow.url .. '/s.png)',
        '',
        'End text.',
      }, { dir = dir })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      U.settle(30)
      local before = U.lines(p.buf)
      a.truthy(wait_placed(p.buf, 1, 2000), 'fast picture shown')
      a.eq(before, U.lines(p.buf), 'text lines unchanged')
      local loading = 0
      for _, v in ipairs(U.virt(p.buf)) do
        local t = table.concat(v.rows, '\n')
        if t:find('loading', 1, true) and t:find('slow one', 1, true) then
          loading = loading + 1
        end
      end
      a.eq(1, loading, 'the slow block still shows its loading box')
    end
  )

  QA.check(
    'R-169',
    'Loading boxes are virtual lines below the anchor line and add no buffer lines',
    function()
      U.image_mode()
      local lines, dir = slow_doc(1500)
      local buf = U.src(lines, { dir = dir })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      U.settle(50)
      local vt = U.virt(p.buf)
      a.eq(1, #vt)
      a.falsy(vt[1].above, 'below the anchor, not above')
      local anchor = U.lines(p.buf)[vt[1].row]
      a.eq('', vim.trim(anchor), 'the anchor line is empty')
      a.not_contains('loading', U.text(p.buf), 'no box text in real lines')
      a.not_contains('remote alt words', U.text(p.buf), 'no box title in real lines')
      local below = U.find_line(p.buf, 'Paragraph below the picture.')
      a.le(
        vt[1].row + 2,
        below,
        'no real rows reserved for the box between the anchor and the next block'
      )
      to_source(buf)
    end
  )

  QA.check(
    'R-169',
    'Placeholder rows of a shown picture are virtual lines below the anchor line',
    function()
      U.image_mode()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/p.png', 80, 60)
      U.src({ 'Above.', '', '![p](p.png)', '', 'Below.' }, { dir = dir })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      a.truthy(wait_placed(p.buf, 1))
      local vt = U.virt(p.buf)
      a.falsy(vt[1].above)
      a.eq('', vim.trim(U.lines(p.buf)[vt[1].row]), 'anchor line empty')
      a.not_contains(U.PLACEHOLDER, U.text(p.buf), 'no placeholder cells in real lines')
    end
  )

  QA.check(
    'R-169',
    'Text-only boxes and failure boxes are virtual lines below the anchor line',
    function()
      local dir = QA.tmpdir()
      -- text-only mode: no probe answer, the preview starts as text
      U.src({ 'Above.', '', '![alt of missing](nothing-here.png)', '', 'Below.' }, { dir = dir })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      U.settle(100)
      local vt = U.virt(p.buf)
      a.eq(1, #vt, 'text-only box')
      a.falsy(vt[1].above)
      a.contains('alt of missing', table.concat(vt[1].rows, '\n'))
      a.not_contains('alt of missing', U.text(p.buf), 'box text not in real lines')
      U.reset({ placement = 'split' })
      U.image_mode()
      U.src({ 'Above.', '', '![alt of missing](nothing-here.png)', '', 'Below.' }, { dir = dir })
      vim.cmd('KittyMarkdown open')
      p = U.preview()
      QA.wait(5000, function()
        local t = U.virt_text(p.buf)
        return t ~= '' and not t:find('loading', 1, true)
      end)
      vt = U.virt(p.buf)
      a.eq(1, #vt, 'failure box')
      a.falsy(vt[1].above)
      a.eq('', vim.trim(U.lines(p.buf)[vt[1].row]), 'anchor line empty')
      a.not_contains('alt of missing', U.text(p.buf), 'failure box text not in real lines')
    end
  )
end)

QA.describe('refresh: unchanged pixel blocks', function()
  QA.before_each(function()
    U.reset({ placement = 'split' })
  end)

  QA.check(
    'R-026',
    'A write that changes only text keeps the shown pictures and transmits nothing again',
    function()
      local cap = U.image_mode()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/keep.png', 70, 35)
      local buf = U.src(
        { '# Keep', '', 'Some text.', '', '![keep](keep.png)', '', 'Tail text.' },
        { dir = dir }
      )
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      a.truthy(wait_placed(p.buf, 1))
      local ids = U.transmits(cap)
      a.eq(1, #ids)
      cap:clear()
      to_source(buf)
      api.nvim_buf_set_lines(buf, 2, 3, false, { 'Some edited text.' })
      U.write(buf)
      QA.wait(1000, function()
        return U.text(p.buf):find('Some edited text.', 1, true) ~= nil
      end)
      U.settle(300)
      a.contains('Some edited text.', U.text(p.buf), 'the write re-rendered')
      a.eq({}, U.transmits(cap), 'no transmit')
      a.eq({}, U.deletes(cap), 'no delete')
      a.eq(1, placed(p.buf), 'picture still shown')
    end
  )

  QA.check(
    'R-026',
    'A write with no change at all keeps the pictures and transmits or deletes nothing',
    function()
      local cap = U.image_mode()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/same.png', 70, 35)
      local buf = U.src(
        { 'A.', '', '![same](same.png)', '', '```mermaid', 'flowchart LR', '  X --> Y', '```' },
        { dir = dir }
      )
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      a.truthy(wait_placed(p.buf, 2, 15000), 'both blocks shown')
      cap:clear()
      to_source(buf)
      U.write(buf)
      U.settle(400)
      a.eq({}, U.transmits(cap), 'no transmit')
      a.eq({}, U.deletes(cap), 'no delete')
      a.eq(2, placed(p.buf), 'both still shown')
    end
  )

  QA.check(
    'R-170',
    'A picture that moves to another line with the same block key is not requested or transmitted again',
    function()
      local cap = U.image_mode()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/move.png', 70, 35)
      local buf = U.src({ '# Move', '', 'Intro.', '', '![move](move.png)' }, { dir = dir })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      a.truthy(wait_placed(p.buf, 1))
      local helper = require('kitty-markdown.helper')
      local requests = 0
      if type(helper.request) == 'function' then
        local orig = helper.request
        helper.request = function(...)
          requests = requests + 1
          return orig(...)
        end
      end
      cap:clear()
      to_source(buf)
      api.nvim_buf_set_lines(
        buf,
        3,
        3,
        false,
        { 'New paragraph pushes the picture down.', '', 'And another one.', '' }
      )
      U.write(buf)
      QA.wait(1000, function()
        return U.text(p.buf):find('And another one.', 1, true) ~= nil
      end)
      U.settle(300)
      a.eq({}, U.transmits(cap), 'no transmit')
      a.eq(0, requests, 'no new helper request')
      a.eq(1, placed(p.buf), 'picture still shown')
    end
  )

  QA.check(
    'R-170',
    'A pixel block whose source changes is rendered and transmitted again',
    function()
      local cap = U.image_mode()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/first.png', 70, 35)
      QA.make_png(dir .. '/second.png', 35, 70, { 10, 200, 10, 255 })
      local buf = U.src({ '# Swap', '', 'Intro.', '', '![pic](first.png)' }, { dir = dir })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      a.truthy(wait_placed(p.buf, 1))
      local old = U.transmits(cap)
      cap:clear()
      to_source(buf)
      api.nvim_buf_set_lines(buf, 4, 5, false, { '![pic](second.png)' })
      U.write(buf)
      a.truthy(
        QA.wait(5000, function()
          return #U.transmits(cap) == 1 and placed(p.buf) == 1
        end),
        'new picture transmitted and shown'
      )
      a.contains(old[1], U.deletes(cap), 'old picture deleted')
    end
  )

  QA.check('R-216', 'A write re-requests a pixel block whose last render failed', function()
    U.image_mode()
    local dir = QA.tmpdir()
    local buf = U.src({ '# Retry', '', 'Intro.', '', '![later](later.png)' }, { dir = dir })
    vim.cmd('KittyMarkdown open')
    local p = U.preview()
    QA.wait(5000, function()
      local t = U.virt_text(p.buf)
      return t ~= '' and not t:find('loading', 1, true)
    end)
    a.falsy(U.has_placeholders(p.buf), 'first render failed')
    QA.make_png(dir .. '/later.png', 50, 25)
    to_source(buf)
    U.write(buf)
    a.truthy(
      QA.wait(5000, function()
        return U.has_placeholders(p.buf)
      end),
      'the block was requested again and now shows'
    )
  end)

  QA.check(
    'R-216',
    'A text-only write re-requests a failed block while it keeps a shown one',
    { severity = 'medium' },
    function()
      local cap = U.image_mode()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/ok.png', 50, 25)
      local buf = U.src(
        { 'One.', '', '![ok](ok.png)', '', 'Two.', '', '![bad](bad.png)' },
        { dir = dir }
      )
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      a.truthy(wait_placed(p.buf, 1), 'the good picture shows')
      QA.wait(3000, function()
        return not U.virt_text(p.buf):find('loading', 1, true)
      end)
      QA.make_png(dir .. '/bad.png', 40, 20, { 0, 0, 200, 255 })
      cap:clear()
      to_source(buf)
      api.nvim_buf_set_lines(buf, 0, 1, false, { 'One edited.' })
      U.write(buf)
      a.truthy(wait_placed(p.buf, 2, 5000), 'both pictures show after the write')
      a.eq(1, #U.transmits(cap), 'only the retried picture is transmitted')
    end
  )
end)

QA.describe('refresh: anchor lines in the layout model', function()
  QA.before_each(function()
    U.reset()
  end)

  local function count_pixel(model)
    local n = 0
    for _, b in ipairs(model.blocks) do
      if b.kind == 'image' or b.kind == 'mermaid' then
        n = n + 1
      end
    end
    return n
  end

  QA.check(
    'R-168',
    'The layout model has one empty anchor line for each pixel block and none for other blocks',
    function()
      local text = {
        '# Head',
        '',
        'Para.',
        '',
        '![one](one.png)',
        '',
        '```lua',
        'x = 1',
        '```',
        '',
        '```mermaid',
        'flowchart LR',
        '  A --> B',
        '```',
        '',
        '| a | b |',
        '| - | - |',
        '| 1 | 2 |',
        '',
        '![two](https://example.invalid/two.png)',
        '',
        'End.',
      }
      for _, images in ipairs({ true, false }) do
        local model = QA.layout(text, { width = 80, images = images })
        a.eq(3, #model.blocks, 'block descriptors (images=' .. tostring(images) .. ')')
        a.eq(3, count_pixel(model), 'all are pixel blocks')
        local seen = {}
        for _, b in ipairs(model.blocks) do
          a.eq('', vim.trim(model.lines[b.line] or 'missing'), 'anchor line is empty')
          a.falsy(seen[b.line], 'distinct anchor lines')
          seen[b.line] = true
        end
      end
    end
  )

  QA.check(
    'R-168',
    'A document without pixel blocks has no block descriptors',
    { severity = 'medium' },
    function()
      local model = QA.layout(
        { '# H', '', 'text', '', '```', 'code', '```', '', '> quote' },
        { width = 80 }
      )
      a.eq(0, #model.blocks)
    end
  )

  QA.check(
    'R-217',
    'The anchor line of a pixel block takes the place of the blank line before it',
    function()
      local model = QA.layout({ 'Leading paragraph.', '', '![img](x.png)' }, { width = 80 })
      a.eq(1, #model.blocks)
      local para
      for i, l in ipairs(model.lines) do
        if l:find('Leading paragraph.', 1, true) then
          para = i
        end
      end
      a.not_nil(para)
      a.eq(
        para + 1,
        model.blocks[1].line,
        'anchor directly after the paragraph, no extra blank line'
      )
    end
  )

  QA.check(
    'R-217',
    'A pixel block between paragraphs adds no blank line of its own before it',
    { severity = 'medium' },
    function()
      local with = QA.layout(
        { 'Before.', '', '```mermaid', 'flowchart LR', '  A-->B', '```', '', 'After.' },
        { width = 80 }
      )
      a.eq(1, #with.blocks)
      local before
      for i, l in ipairs(with.lines) do
        if l:find('Before.', 1, true) then
          before = i
        end
      end
      a.eq(before + 1, with.blocks[1].line, 'anchor right after the previous block')
    end
  )

  QA.check(
    'R-217',
    'Two pixel blocks in a row have at most one blank line between their anchor lines',
    { severity = 'medium' },
    function()
      local model = QA.layout({ 'Text.', '', '![a](a.png)', '', '![b](b.png)' }, { width = 80 })
      a.eq(2, #model.blocks)
      a.le(
        model.blocks[1].line + 2,
        model.blocks[2].line,
        'no run of blank lines between two pixel blocks'
      )
    end
  )

  QA.check('R-258', 'A pixel block that opens the document has its anchor on line 1', function()
    local img = QA.layout({ '![first](first.png)', '', 'Text after.' }, { width = 80 })
    a.eq(1, #img.blocks)
    a.eq(1, img.blocks[1].line, 'image anchor line')
    local mer = QA.layout(
      { '```mermaid', 'flowchart LR', '  A --> B', '```', '', 'After.' },
      { width = 80 }
    )
    a.eq(1, #mer.blocks)
    a.eq(1, mer.blocks[1].line, 'mermaid anchor line')
  end)

  QA.check(
    'R-258',
    'In the preview, a leading pixel block hangs its box below line 1',
    { severity = 'medium' },
    function()
      U.reset({ placement = 'split' })
      U.src({ '![lead alt](lead.png)', '', 'Body text.' })
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      U.settle(50)
      local vt = U.virt(p.buf)
      a.eq(1, #vt, 'one box')
      a.eq(1, vt[1].row, 'anchor on line 1')
      a.eq('', vim.trim(U.lines(p.buf)[1]), 'line 1 is the empty anchor')
    end
  )

  QA.check('R-259', 'One blank line follows a pixel block before the next block', function()
    local model = QA.layout({ 'Before.', '', '![mid](mid.png)', '', 'After text.' }, { width = 80 })
    a.eq(1, #model.blocks)
    local n = model.blocks[1].line
    a.eq('', vim.trim(model.lines[n + 1] or 'missing'), 'blank line after the block')
    a.matches('After text%.', model.lines[n + 2] or '', 'next block right after one blank line')
  end)

  QA.check('R-259', 'One blank line follows a pixel block that ends the document', function()
    for _, last in ipairs({
      { '![end](end.png)' },
      { '```mermaid', 'flowchart LR', '  A --> B', '```' },
    }) do
      local text = { 'Before.', '' }
      vim.list_extend(text, last)
      local model = QA.layout(text, { width = 80 })
      a.eq(1, #model.blocks)
      local n = model.blocks[1].line
      a.eq(n + 1, #model.lines, 'exactly one line after the anchor')
      a.eq('', vim.trim(model.lines[n + 1]), 'that line is blank')
    end
  end)

  QA.check(
    'R-259',
    'A pixel block that is the only block gets its anchor and one blank line',
    { severity = 'medium' },
    function()
      local model = QA.layout({ '![solo](solo.png)' }, { width = 80 })
      a.eq(1, #model.blocks)
      a.eq(1, model.blocks[1].line)
      a.eq(2, #model.lines)
      a.eq('', vim.trim(model.lines[2]))
    end
  )
end)
