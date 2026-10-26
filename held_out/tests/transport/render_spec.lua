local QA = require('qa')
local U = require('transport_util')
local a = QA.assert

local function img_doc(alt, file)
  return {
    '# Pictures',
    '',
    'Intro text.',
    '',
    '![' .. alt .. '](' .. file .. ')',
    '',
    'Closing text.',
  }
end

-- Opens a document with one image, answers the probe, and waits until the
-- image shows. Returns src, the placeholder mark, and the transmission.
local function placed_image(cap, dir, w, h, opts)
  opts = opts or {}
  QA.make_png(dir .. '/a.png', w, h, opts.rgba)
  local src = U.open_doc(dir, img_doc('a picture', 'a.png'))
  U.answer_yes(cap)
  local ok, marks = U.wait_placed(src, 1)
  a.truthy(ok, 'the image shows as placeholder cells')
  local tx = U.transmissions(cap)
  a.ge(1, #tx, 'transmissions')
  return src, marks[1], tx[#tx]
end

local function grid(mark)
  local rows, cols = 0, nil
  for _, r in ipairs(mark.rows) do
    local cells = U.decode_row(r)
    if #cells > 0 then
      rows = rows + 1
      cols = cols or #cells
      if #cells ~= cols then
        return rows, cols, false
      end
    end
  end
  return rows, cols, true
end

QA.describe('transport: text-only boxes', function()
  QA.after_each(function()
    QA.reset_plugin()
  end)

  QA.check(
    'R-120',
    'In text-only mode an image shows as a bordered box that holds its alt text',
    function()
      local _, dir = U.env({ helper = false })
      QA.make_png(dir .. '/a.png', 50, 40)
      local src = U.open_doc(dir, img_doc('Harbour at dusk', 'a.png'))
      U.answer_no()
      a.truthy(U.wait_mode('text'), 'text-only mode')
      U.settle(50)
      local mark = U.mark_with(U.preview_buf(src), 'Harbour at dusk')
      a.not_nil(mark, 'virtual lines that hold the alt text')
      local ok, why = U.is_box(mark.rows)
      a.truthy(
        ok,
        'the alt text sits in a bordered box: ' .. tostring(why) .. ' ' .. vim.inspect(mark.rows)
      )
      a.eq(0, #U.placeholder_marks(U.preview_buf(src)), 'no placeholder cells')
    end
  )

  QA.check(
    'R-120',
    'In text-only mode every image of the document gets its own box, and none is transmitted',
    function()
      local cap, dir = U.env({ helper = false })
      QA.make_png(dir .. '/a.png', 50, 40)
      QA.make_png(dir .. '/b.png', 30, 30)
      local src =
        U.open_doc(dir, { '![North gate](a.png)', '', 'text', '', '![South gate](b.png)' })
      U.answer_no()
      a.truthy(U.wait_mode('text'), 'text-only mode')
      U.settle(50)
      for _, alt in ipairs({ 'North gate', 'South gate' }) do
        local mark = U.mark_with(U.preview_buf(src), alt)
        a.not_nil(mark, 'box for ' .. alt)
        a.truthy(U.is_box(mark.rows), 'bordered box for ' .. alt)
      end
      a.eq(0, #U.transmissions(cap), 'transmissions in text-only mode')
      a.eq(0, #U.placements(cap), 'placements in text-only mode')
    end
  )

  QA.check(
    'R-121',
    'In text-only mode a mermaid block shows as a bordered box that holds the diagram source',
    function()
      local _, dir = U.env({ helper = false })
      local src = U.open_doc(dir, {
        '# D',
        '',
        '```mermaid',
        'flowchart LR',
        '  Kiln --> Glaze',
        '  Glaze --> Fire',
        '```',
        '',
        'after',
      })
      U.answer_no()
      a.truthy(U.wait_mode('text'), 'text-only mode')
      U.settle(50)
      local mark = U.mark_with(U.preview_buf(src), 'Kiln --> Glaze')
      a.not_nil(mark, 'virtual lines that hold the diagram source')
      local all = table.concat(mark.rows, '\n')
      a.contains('flowchart LR', all, 'first source line')
      a.contains('Glaze --> Fire', all, 'last source line')
      local ok, why = U.is_box(mark.rows)
      a.truthy(
        ok,
        'the source sits in a bordered box: ' .. tostring(why) .. ' ' .. vim.inspect(mark.rows)
      )
      a.eq(0, #U.placeholder_marks(U.preview_buf(src)), 'no placeholder cells')
    end
  )

  QA.check(
    'R-121',
    'Each source line of the diagram keeps its own row in the text-only box',
    { severity = 'medium' },
    function()
      local _, dir = U.env({ helper = false })
      local src = U.open_doc(
        dir,
        { '```mermaid', 'sequenceDiagram', '  Ana->>Ben: hi', '  Ben-->>Ana: yo', '```' }
      )
      U.answer_no()
      a.truthy(U.wait_mode('text'), 'text-only mode')
      U.settle(50)
      local mark = U.mark_with(U.preview_buf(src), 'Ana->>Ben: hi')
      a.not_nil(mark, 'box')
      local idx = {}
      for i, r in ipairs(mark.rows) do
        for _, needle in ipairs({ 'sequenceDiagram', 'Ana->>Ben: hi', 'Ben-->>Ana: yo' }) do
          if r:find(needle, 1, true) then
            idx[needle] = i
          end
        end
      end
      a.not_nil(idx['sequenceDiagram'], 'first line')
      a.not_nil(idx['Ben-->>Ana: yo'], 'last line')
      a.truthy(
        idx['sequenceDiagram'] < idx['Ana->>Ben: hi']
          and idx['Ana->>Ben: hi'] < idx['Ben-->>Ana: yo'],
        'source lines in order on separate rows'
      )
    end
  )
end)

QA.describe('transport: image mode', function()
  local cap, dir
  QA.before_each(function()
    cap, dir = U.env({ cell_w = 10, cell_h = 20 })
  end)
  QA.after_each(function()
    QA.reset_plugin()
    vim.env.TMUX = nil
  end)

  QA.check(
    'R-123',
    'An image in a preview is transmitted as valid chunks whose decoded data is a PNG of the image size',
    function()
      local _, _, tx = placed_image(cap, dir, 120, 70)
      for i, c in ipairs(tx.chunks) do
        a.le(4096, #c.payload, 'chunk ' .. i .. ' size')
        if i < #tx.chunks then
          a.eq('1', c.control.m, 'm on chunk ' .. i)
        end
      end
      local m = tx.chunks[#tx.chunks].control.m
      a.truthy(m == '0' or m == nil, 'last chunk m=0')
      a.eq('2', tx.first.control.q, 'quiet transmission')
      local w, h = U.png_dims(tx.data)
      a.eq(120, w, 'decoded PNG width')
      a.eq(70, h, 'decoded PNG height')
    end
  )

  QA.check(
    'R-123',
    'An image is transmitted once: re-rendering an unchanged document sends no image data again',
    function()
      local src = placed_image(cap, dir, 120, 70)
      local n = #U.transmissions(cap)
      a.eq(1, n, 'transmissions after the first render')
      vim.api.nvim_set_current_win(vim.fn.bufwinid(src))
      vim.cmd('silent write')
      U.settle(300)
      a.truthy(U.wait_placed(src, 1), 'still placed after the re-render')
      a.eq(1, #U.transmissions(cap), 'transmissions after a re-render')
    end
  )

  QA.check(
    'R-189',
    'Images in a preview use direct PNG transmission and never a file path',
    function()
      local _, _, tx = placed_image(cap, dir, 60, 40)
      a.eq('d', tx.first.control.t, 't on the first chunk')
      a.eq('100', tx.first.control.f, 'f on the first chunk')
      for _, c in ipairs(U.commands(cap)) do
        local t = c.control.t
        a.truthy(t == nil or t == 'd', 'medium ' .. tostring(t))
      end
      a.not_contains(dir, cap:text(), 'a path of the document directory in the bytes')
    end
  )

  QA.check(
    'R-124',
    'The virtual placement uses the transmitted id and the same column and row counts as the placeholder grid',
    function()
      local _, mark, tx = placed_image(cap, dir, 120, 70)
      local p = U.placements(cap)
      a.eq(1, #p, 'virtual placements')
      a.eq(tostring(tx.id), p[1].control.i, 'placement id')
      local rows, cols, rect = grid(mark)
      a.truthy(rect, 'every placeholder row has the same number of cells')
      a.eq(tostring(cols), p[1].control.c, 'placement columns')
      a.eq(tostring(rows), p[1].control.r, 'placement rows')
    end
  )

  QA.check(
    { 'R-183', 'R-124' },
    'The placement size in cells follows the cell size read from the terminal window',
    function()
      QA.reset_plugin()
      cap, dir = U.env({ cell_w = 8, cell_h = 16 })
      local _, _, tx = placed_image(cap, dir, 120, 70)
      local w, h = U.png_dims(tx.data)
      local p = U.placements(cap)[1]
      a.not_nil(p, 'placement')
      a.eq(tostring(math.ceil(h / 16)), p.control.r, 'rows for the image height over a 16 px cell')
      a.eq(tostring(math.ceil(w / 8)), p.control.c, 'columns for the image width over an 8 px cell')
    end
  )

  QA.check(
    'R-183',
    'A different terminal cell size gives a different row count for the same image',
    function()
      QA.reset_plugin()
      cap, dir = U.env({ cell_w = 10, cell_h = 10 })
      local _, mark1 = placed_image(cap, dir, 60, 95)
      local r1 = grid(mark1)
      QA.reset_plugin()
      cap, dir = U.env({ cell_w = 10, cell_h = 30 })
      local _, mark2 = placed_image(cap, dir, 60, 95)
      local r2 = grid(mark2)
      a.eq(10, r1, 'rows with a 10 px cell height')
      a.eq(4, r2, 'rows with a 30 px cell height')
    end
  )

  QA.check(
    'R-125',
    'Each placeholder cell in the preview carries its own row and column diacritic',
    function()
      local _, mark = placed_image(cap, dir, 120, 70)
      local r = 0
      for _, row in ipairs(mark.rows) do
        local cells = U.decode_row(row)
        if #cells > 0 then
          for c, cell in ipairs(cells) do
            a.eq(r, cell.row, ('row diacritic at %d,%d'):format(r, c - 1))
            a.eq(c - 1, cell.col, ('column diacritic at %d,%d'):format(r, c - 1))
          end
          r = r + 1
        end
      end
      a.ge(1, r, 'placeholder rows')
    end
  )

  QA.check(
    'R-125',
    'Every placeholder cell in the preview has the image id as its foreground color',
    function()
      local _, mark, tx = placed_image(cap, dir, 120, 70)
      local cells = U.cells_with_fg(mark)
      a.ge(1, #cells, 'cells')
      for i, c in ipairs(cells) do
        a.eq(tx.id % 2 ^ 24, c.fg, 'foreground of cell ' .. i)
      end
    end
  )

  QA.check('R-188', 'Every placeholder cell in the preview carries both diacritics', function()
    local _, mark = placed_image(cap, dir, 120, 70)
    local n = 0
    for _, row in ipairs(mark.rows) do
      for _, cell in ipairs(U.decode_row(row)) do
        n = n + 1
        a.ge(2, cell.ndia, 'diacritics on cell ' .. n)
      end
      local ph = select(2, row:gsub(U.PH, ''))
      a.eq(#U.decode_row(row), ph, 'every U+10EEEE decodes')
    end
    a.ge(2, n, 'cells')
  end)

  QA.check(
    'R-126',
    'An image whose id is above 24 bits gets a third diacritic with the high byte on every cell',
    function()
      local transport = require('kitty-markdown.transport')
      QA.need(type(transport.set_next_id) == 'function', 'transport.set_next_id (test hook)')
      transport.set_next_id(3 * 2 ^ 24 + 5)
      local _, mark, tx = placed_image(cap, dir, 40, 40)
      a.ge(2 ^ 24, tx.id, 'the transmitted id is above 24 bits')
      local msb = math.floor(tx.id / 2 ^ 24)
      local cells = U.cells_with_fg(mark)
      a.ge(1, #cells, 'cells')
      for i, c in ipairs(cells) do
        a.eq(3, c.ndia, 'diacritics on cell ' .. i)
        a.eq(msb, c.msb, 'high byte on cell ' .. i)
        a.eq(tx.id % 2 ^ 24, c.fg, 'foreground carries the low 24 bits on cell ' .. i)
      end
    end
  )

  QA.check(
    'R-127',
    'Placeholder rows are extmark virtual lines below an empty anchor line, not buffer text',
    function()
      local src, mark = placed_image(cap, dir, 120, 70)
      local pbuf = U.preview_buf(src)
      local lines = vim.api.nvim_buf_get_lines(pbuf, 0, -1, false)
      for i, l in ipairs(lines) do
        a.not_contains(U.PH, l, 'placeholder in buffer line ' .. i)
      end
      local anchor = lines[mark.line + 1]
      a.eq('', vim.trim(anchor or 'x'), 'the anchor line is empty')
      local rows = grid(mark)
      a.ge(rows, #mark.rows, 'virtual rows hold the whole grid')
    end
  )

  QA.check('R-128', 'Closing the preview deletes the placed image by its id', function()
    local src, _, tx = placed_image(cap, dir, 120, 70)
    cap:clear()
    require('kitty-markdown.preview').close(src)
    U.settle(50)
    local d = U.deletes(cap)
    local hit = false
    for _, c in ipairs(d) do
      if c.control.i == tostring(tx.id) and (c.control.d == 'i' or c.control.d == 'I') then
        hit = true
      end
    end
    a.truthy(hit, 'a delete by id for the image: ' .. vim.inspect(vim.tbl_map(function(c)
      return c.raw
    end, d)))
  end)

  QA.check(
    'R-128',
    'Closing the preview window with a window command also deletes the image',
    function()
      local src, _, tx = placed_image(cap, dir, 120, 70)
      cap:clear()
      local pwin = vim.fn.bufwinid(U.preview_buf(src))
      vim.api.nvim_win_close(pwin, true)
      U.settle(100)
      local hit = false
      for _, c in ipairs(U.deletes(cap)) do
        if c.control.i == tostring(tx.id) then
          hit = true
        end
      end
      a.truthy(hit, 'a delete by id for the image')
    end
  )

  QA.check('R-128', 'When an image block changes, its old image is deleted by id', function()
    local src, _, tx = placed_image(cap, dir, 120, 70)
    QA.make_png(dir .. '/b.png', 80, 50, { 0, 0, 255, 255 })
    cap:clear()
    vim.api.nvim_set_current_win(vim.fn.bufwinid(src))
    vim.api.nvim_buf_set_lines(src, 4, 5, false, { '![a picture](b.png)' })
    vim.cmd('silent write')
    a.truthy(
      QA.wait(8000, function()
        return #U.transmissions(cap) >= 1
      end),
      'the new image is transmitted'
    )
    local hit = false
    for _, c in ipairs(U.deletes(cap)) do
      if c.control.i == tostring(tx.id) then
        hit = true
      end
    end
    a.truthy(hit, 'a delete for the old image id')
  end)

  QA.check(
    'R-128',
    'When an image block is removed from the document, its image is deleted by id',
    function()
      local src, _, tx = placed_image(cap, dir, 120, 70)
      cap:clear()
      vim.api.nvim_set_current_win(vim.fn.bufwinid(src))
      vim.api.nvim_buf_set_lines(src, 4, 5, false, { 'no picture now' })
      vim.cmd('silent write')
      U.settle(200)
      local hit = false
      for _, c in ipairs(U.deletes(cap)) do
        if c.control.i == tostring(tx.id) then
          hit = true
        end
      end
      a.truthy(hit, 'a delete for the removed image id')
    end
  )

  QA.check(
    { 'R-129', 'R-186' },
    'Two images placed at the same time have different ids, allocated in increasing order',
    function()
      QA.make_png(dir .. '/a.png', 60, 40)
      QA.make_png(dir .. '/b.png', 50, 30, { 0, 90, 0, 255 })
      local src = U.open_doc(dir, { '![one](a.png)', '', 'middle', '', '![two](b.png)' })
      U.answer_yes(cap)
      local ok, marks = U.wait_placed(src, 2)
      a.truthy(ok, 'both images placed')
      local tx = U.transmissions(cap)
      a.eq(2, #tx, 'transmissions')
      a.ne(tx[1].id, tx[2].id, 'image ids')
      a.truthy(tx[2].id > tx[1].id, 'the later id is larger')
      local fgs = {}
      for _, m in ipairs(marks) do
        local cells = U.cells_with_fg(m)
        fgs[#fgs + 1] = cells[1] and cells[1].fg
      end
      a.ne(fgs[1], fgs[2], 'the two blocks carry different ids in their foreground')
    end
  )

  QA.check(
    'R-187',
    'An image shown again after its preview closed and reopened gets an id never used before',
    function()
      local src, _, tx1 = placed_image(cap, dir, 120, 70)
      local preview = require('kitty-markdown.preview')
      preview.close(src)
      U.settle(50)
      cap:clear()
      vim.api.nvim_set_current_win(vim.fn.bufwinid(src))
      preview.open(src, 'split')
      a.truthy(U.wait_placed(src, 1), 'placed again')
      local tx = U.transmissions(cap)
      a.ge(1, #tx, 'transmitted again after reopen')
      a.ne(tx1.id, tx[#tx].id, 'the id after reopen')
      a.ge(tx1.id, tx[#tx].id, 'ids keep increasing')
    end
  )

  QA.check('R-187', 'A changed image gets a new id, not the id of the image it replaced', function()
    local src, _, tx1 = placed_image(cap, dir, 120, 70)
    QA.make_png(dir .. '/b.png', 80, 50, { 0, 0, 255, 255 })
    cap:clear()
    vim.api.nvim_set_current_win(vim.fn.bufwinid(src))
    vim.api.nvim_buf_set_lines(src, 4, 5, false, { '![a picture](b.png)' })
    vim.cmd('silent write')
    a.truthy(
      QA.wait(8000, function()
        return #U.transmissions(cap) >= 1
      end),
      'the new image is transmitted'
    )
    a.ne(tx1.id, U.transmissions(cap)[1].id, 'id of the new image')
  end)

  QA.check(
    'R-238',
    'A pixel block that needs more than 297 rows shows a failure box with the reason too tall',
    function()
      QA.reset_plugin()
      cap, dir = U.env({ cell_w = 8, cell_h = 4 })
      QA.make_png(dir .. '/tall.png', 8, 297 * 4 + 2)
      local src = U.open_doc(dir, img_doc('a tall strip', 'tall.png'))
      U.answer_yes(cap)
      local ok = QA.wait(8000, function()
        return U.mark_with(U.preview_buf(src), 'too tall') ~= nil
      end)
      a.truthy(ok, 'a box with the reason too tall')
      local mark = U.mark_with(U.preview_buf(src), 'too tall')
      a.truthy(U.is_box(mark.rows), 'the reason sits in a bordered box')
      a.eq(0, #U.placeholder_marks(U.preview_buf(src)), 'no placeholder cells')
      for _, p in ipairs(U.placements(cap)) do
        a.le(297, tonumber(p.control.r), 'rows of a placement')
      end
    end
  )

  QA.check('R-238', 'A pixel block of exactly 297 rows is still shown as an image', function()
    QA.reset_plugin()
    cap, dir = U.env({ cell_w = 8, cell_h = 4 })
    QA.make_png(dir .. '/tall.png', 8, 297 * 4)
    local src = U.open_doc(dir, img_doc('a tall strip', 'tall.png'))
    U.answer_yes(cap)
    local ok, marks = U.wait_placed(src, 1)
    a.truthy(ok, 'placed')
    a.eq(297, (grid(marks[1])), 'placeholder rows')
    a.is_nil(U.mark_with(U.preview_buf(src), 'too tall'), 'no too tall box')
  end)

  QA.check(
    'R-241',
    'After a colorscheme change every placeholder cell keeps the image id as its foreground',
    function()
      local src, _, tx = placed_image(cap, dir, 120, 70)
      vim.cmd('colorscheme blue')
      U.settle(100)
      local marks = U.placeholder_marks(U.preview_buf(src))
      a.eq(1, #marks, 'placeholder blocks after the change')
      for i, c in ipairs(U.cells_with_fg(marks[1])) do
        a.eq(tx.id % 2 ^ 24, c.fg, 'foreground of cell ' .. i .. ' after colorscheme blue')
      end
      vim.cmd('colorscheme default')
      U.settle(100)
      marks = U.placeholder_marks(U.preview_buf(src))
      for i, c in ipairs(U.cells_with_fg(marks[1])) do
        a.eq(tx.id % 2 ^ 24, c.fg, 'foreground of cell ' .. i .. ' after colorscheme default')
      end
    end
  )

  QA.check(
    'R-241',
    'A colorscheme that clears all highlights still leaves the id foreground on the placeholder cells',
    function()
      local src, _, tx = placed_image(cap, dir, 120, 70)
      local cdir = dir .. '/rtp/colors'
      vim.fn.mkdir(cdir, 'p')
      QA.write(
        cdir .. '/qa_bare.lua',
        "vim.cmd('hi clear')\nvim.g.colors_name = 'qa_bare'\nvim.api.nvim_set_hl(0, 'Normal', { fg = '#ffffff', bg = '#000000' })\n"
      )
      vim.opt.runtimepath:append(dir .. '/rtp')
      vim.cmd('colorscheme qa_bare')
      U.settle(100)
      local marks = U.placeholder_marks(U.preview_buf(src))
      a.eq(1, #marks, 'placeholder blocks')
      for i, c in ipairs(U.cells_with_fg(marks[1])) do
        a.eq(tx.id % 2 ^ 24, c.fg, 'foreground of cell ' .. i)
      end
    end
  )

  QA.check(
    'R-122',
    'Inside tmux the probe and every image sequence of a preview go through the passthrough',
    function()
      QA.reset_plugin()
      cap, dir = U.env({ tmux = '/tmp/tmux-501/default,999,0' })
      local src = placed_image(cap, dir, 120, 70)
      local wraps, outside = U.split_tmux(cap:text())
      a.no_match('\27_G', outside, 'bare graphics sequence outside a passthrough')
      for i, w in ipairs(wraps) do
        a.truthy(w.ok, 'wrapper ' .. i .. ' doubles its ESC bytes')
      end
      a.ge(1, #U.queries(cap), 'the probe query inside a wrapper')
      a.ge(1, #U.placements(cap), 'the placement inside a wrapper')
      a.truthy(U.wait_placed(src, 1), 'placed')
    end
  )
end)
