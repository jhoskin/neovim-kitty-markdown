local QA = require('qa')
local U = require('core_util')
local a = QA.assert

local IMG_DOC = {
  'intro text',
  '',
  '![alt one](pics/one.png)',
  '',
  'middle',
  '',
  '```mermaid',
  'graph TD',
  '  A-->B',
  '```',
  '',
  'end',
}

local function km()
  return require('kitty-markdown')
end

local function find_block(model, kind_pat)
  for _, b in ipairs(model.blocks or {}) do
    if type(b.kind) == 'string' and b.kind:find(kind_pat) then
      return b
    end
  end
end

-- The block key of a descriptor: the `key` field (R-335).
local function block_key(desc)
  local v = desc and desc.key
  if type(v) == 'string' and #v == 64 and v:match('^%x+$') then
    return v
  end
end

-- A size entry with the fields of R-334.
local function size(w, h, id)
  return { width_px = w, height_px = h, image_id = id }
end

-- A cell size with the fields of R-333.
local CELL = { width_px = 10, height_px = 20 }

local function line_of(model, needle)
  for i, l in ipairs(model.lines) do
    if l:find(needle, 1, true) then
      return i
    end
  end
end

QA.describe('core: test hooks', function()
  QA.before_each(function()
    QA.reset_plugin()
    vim.cmd('set background=light')
  end)

  -- R-153 ------------------------------------------------------------------

  QA.check(
    'R-153',
    'layout() without opts returns a model for a buffer that is not shown in any window',
    function()
      local buf = QA.buf({ '# T', '', 'para' })
      local model
      a.no_error(function()
        model = km().layout(buf)
      end)
      a.eq('table', type(model))
      a.eq('table', type(model.lines))
    end
  )

  QA.check(
    'R-153',
    'layout() writes nothing to the terminal, even for a document with images and diagrams',
    function()
      local cap = QA.capture_terminal()
      QA.layout(IMG_DOC, { width = 60 })
      QA.wait(100, function()
        return false
      end)
      a.eq('', cap:text())
    end
  )

  QA.check(
    'R-153',
    'layout() opens no window or buffer and starts no job or helper process',
    function()
      local wins, bufs, chans =
        #vim.api.nvim_list_wins(), #vim.api.nvim_list_bufs(), #vim.api.nvim_list_chans()
      local buf = QA.buf(IMG_DOC)
      bufs = bufs + 1
      km().layout(buf, { width = 60, cell = CELL })
      QA.wait(100, function()
        return false
      end)
      a.eq(wins, #vim.api.nvim_list_wins(), 'windows')
      a.eq(bufs, #vim.api.nvim_list_bufs(), 'buffers')
      a.eq(chans, #vim.api.nvim_list_chans(), 'channels or jobs')
      local helper = package.loaded['kitty-markdown.helper']
      if helper and type(helper.is_running) == 'function' then
        a.falsy(helper.is_running(), 'helper running')
      end
    end
  )

  QA.check(
    'R-153',
    'The model holds only plain tables and values, with no metatables, functions, or userdata',
    function()
      local model = QA.layout(
        vim.list_extend(
          vim.deepcopy(IMG_DOC),
          { '', '# H', '', '[l](https://x.y)', '', '```lua', 'x', '```' }
        ),
        { width = 60 }
      )
      local ok, why = U.plain(model)
      a.truthy(ok, 'non-plain value: ' .. tostring(why))
    end
  )

  QA.check('R-153', 'layout() does not change the source buffer', function()
    local text = { '# T', '', 'a *b* $x^2$', '', '![i](x.png)' }
    local buf = QA.buf(text)
    local tick = vim.api.nvim_buf_get_changedtick(buf)
    km().layout(buf, { width = 50 })
    a.eq(text, vim.api.nvim_buf_get_lines(buf, 0, -1, false))
    a.eq(tick, vim.api.nvim_buf_get_changedtick(buf))
  end)

  -- R-154 ------------------------------------------------------------------

  QA.check(
    'R-154',
    'The model holds lines as strings, highlights, conceals, and blocks as lists',
    function()
      local model = QA.layout({ '# T', '', 'a **b** c', '', '![i](x.png)' }, { width = 50 })
      for _, k in ipairs({ 'lines', 'highlights', 'conceals', 'blocks' }) do
        a.eq('table', type(model[k]), k)
      end
      a.ge(1, #model.lines)
      for i, l in ipairs(model.lines) do
        a.eq('string', type(l), 'line ' .. i)
      end
    end
  )

  QA.check(
    'R-154',
    'Each highlight carries a line, a start column, an end column, and a group within the model lines',
    function()
      local model = QA.layout(
        { '# Title', '', 'some **bold** and `code` and [link](https://x.y)', '', '- item' },
        { width = 60 }
      )
      a.ge(1, #model.highlights)
      local n = #model.lines
      for i, h in ipairs(model.highlights) do
        local line, cs, ce, g = U.hl_parts(h)
        a.eq('number', type(line), 'highlight ' .. i .. ' line')
        a.eq('number', type(cs), 'highlight ' .. i .. ' start col')
        a.eq('number', type(ce), 'highlight ' .. i .. ' end col')
        a.eq('string', type(g), 'highlight ' .. i .. ' group')
        a.truthy(line >= 1 and line <= n, 'highlight ' .. i .. ' line in range (1-based)')
        a.truthy(cs >= 0 and cs <= ce, 'highlight ' .. i .. ' start <= end')
        local text = model.lines[line] or ''
        a.le(#text, ce, 'highlight ' .. i .. ' end within the line bytes')
      end
    end
  )

  QA.check('R-154', 'A highlight span covers exactly the styled text', function()
    local model = QA.layout({ 'aa **boldword** zz' }, { width = 60 })
    local l = model.lines[line_of(model, 'boldword')]
    local s = l:find('boldword', 1, true)
    local e = s + #'boldword' - 1
    local found = false
    for _, h in ipairs(model.highlights) do
      local _, cs, ce = U.hl_parts(h)
      -- 0-based byte columns with an exclusive end (R-337)
      if cs == s - 1 and ce == e then
        found = true
      end
    end
    a.truthy(found, 'a span with exactly the bounds of the bold word')
  end)

  QA.check(
    'R-154',
    'conceals is a list (it may be empty) whose entries reference model lines',
    { severity = 'medium' },
    function()
      local model = QA.layout({ 'a [link](https://x.y) b' }, { width = 60 })
      a.eq('table', type(model.conceals))
      a.truthy(vim.islist(model.conceals) or next(model.conceals) == nil, 'conceals is a list')
    end
  )

  -- R-155 ------------------------------------------------------------------

  QA.check(
    'R-155',
    'An image block descriptor holds kind, line, source, and width in cells',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      local b = find_block(model, 'image')
      a.not_nil(b, 'image descriptor')
      a.eq('number', type(b.line))
      a.matches('one%.png$', b.source)
      a.eq('number', type(b.width_cells))
      a.ge(1, b.width_cells)
      a.le(60, b.width_cells)
    end
  )

  QA.check('R-155', 'A mermaid block descriptor holds the diagram source as its source', function()
    local model = QA.layout(IMG_DOC, { width = 60 })
    local b = find_block(model, 'mermaid')
    a.not_nil(b, 'mermaid descriptor')
    a.contains('graph TD', b.source)
    a.contains('A-->B', b.source)
    a.no_match('```', b.source)
  end)

  QA.check(
    'R-155',
    'A block descriptor line points at the empty anchor line of that block',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      a.eq(2, #model.blocks, 'one descriptor per pixel block')
      for _, b in ipairs(model.blocks) do
        local l = model.lines[b.line]
        a.truthy(l and vim.trim(l) == '', 'anchor line is empty: ' .. vim.inspect(l))
      end
      a.truthy(model.blocks[1].line < model.blocks[2].line, 'descriptors in document order')
    end
  )

  QA.check(
    { 'R-155', 'R-332' },
    'The width in cells of a block is the text width of the layout',
    function()
      local m1 = QA.layout(IMG_DOC, { width = 60 })
      local m2 = QA.layout(IMG_DOC, { width = 80 })
      -- opts.width is the window width (R-332), so the text width is
      -- width - 4 (a margin of 2 cells on each side)
      a.eq(56, find_block(m1, 'image').width_cells)
      a.eq(76, find_block(m2, 'image').width_cells)
      local m3 = QA.layout(IMG_DOC, { width = 300 })
      a.eq(100, find_block(m3, 'image').width_cells, 'capped at the maximum text width')
    end
  )

  QA.check(
    'R-155',
    'Without sizes, a descriptor has no pixel size, no rows, and no image id',
    function()
      local model = QA.layout(IMG_DOC, { width = 60, cell = CELL })
      for _, b in ipairs(model.blocks) do
        a.is_nil(b.width_px, 'width_px')
        a.is_nil(b.height_px, 'height_px')
        a.is_nil(b.rows, 'rows')
        a.is_nil(b.image_id, 'image_id')
      end
    end
  )

  QA.check(
    { 'R-155', 'R-334' },
    'A size with an image id sets image_id on that descriptor only',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      local key = block_key(find_block(model, 'image'))
      QA.need(key, 'the block key in the descriptor')
      local m2 =
        QA.layout(IMG_DOC, { width = 60, cell = CELL, sizes = { [key] = size(100, 50, 4242) } })
      local img, mer = find_block(m2, 'image'), find_block(m2, 'mermaid')
      a.eq(4242, img.image_id)
      a.eq(100, img.width_px)
      a.eq(50, img.height_px)
      a.is_nil(mer.image_id, 'the other block has no image id')
    end
  )

  QA.check('R-155', 'A size without an image id leaves image_id unset', function()
    local model = QA.layout(IMG_DOC, { width = 60 })
    local key = block_key(find_block(model, 'image'))
    QA.need(key, 'the block key in the descriptor')
    local m2 = QA.layout(IMG_DOC, {
      width = 60,
      cell = CELL,
      sizes = { [key] = { width_px = 100, height_px = 50, width = 100, height = 50 } },
    })
    a.is_nil(find_block(m2, 'image').image_id)
  end)

  -- R-156 ------------------------------------------------------------------

  QA.check(
    { 'R-156', 'R-333' },
    'A block with a pixel size gets rows reserved from its height and the cell height',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      local key = block_key(find_block(model, 'image'))
      QA.need(key, 'the block key in the descriptor')
      local m2 = QA.layout(IMG_DOC, { width = 60, cell = CELL, sizes = { [key] = size(200, 90) } })
      local b = find_block(m2, 'image')
      a.eq('number', type(b.rows))
      -- 90 px at 20 px per row needs 5 rows; an image narrower than the
      -- target width is not scaled
      a.eq(5, b.rows)
    end
  )

  QA.check(
    'R-156',
    'A height that is an exact number of rows reserves exactly that many rows',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      local key = block_key(find_block(model, 'image'))
      QA.need(key, 'the block key in the descriptor')
      local m2 = QA.layout(IMG_DOC, { width = 60, cell = CELL, sizes = { [key] = size(100, 100) } })
      a.eq(5, find_block(m2, 'image').rows)
    end
  )

  QA.check(
    'R-156',
    'Only the blocks with a size get rows; the other blocks stay without rows',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      local key = block_key(find_block(model, 'mermaid'))
      QA.need(key, 'the block key in the descriptor')
      local m2 = QA.layout(IMG_DOC, { width = 60, cell = CELL, sizes = { [key] = size(300, 200) } })
      a.ge(1, find_block(m2, 'mermaid').rows or 0)
      a.is_nil(find_block(m2, 'image').rows)
    end
  )

  QA.check(
    'R-156',
    'Reserving rows keeps the model lines the same: the rows hang below the anchor line',
    { severity = 'medium' },
    function()
      local model = QA.layout(IMG_DOC, { width = 60, cell = CELL })
      local key = block_key(find_block(model, 'image'))
      QA.need(key, 'the block key in the descriptor')
      local m2 = QA.layout(IMG_DOC, { width = 60, cell = CELL, sizes = { [key] = size(200, 300) } })
      a.eq(model.lines, m2.lines)
    end
  )

  QA.check(
    { 'R-156', 'R-336' },
    'A block wider than the target width is not scaled again by the layout',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      local key = block_key(find_block(model, 'image'))
      QA.need(key, 'the block key in the descriptor')
      local m2 =
        QA.layout(IMG_DOC, { width = 60, cell = CELL, sizes = { [key] = size(4000, 400) } })
      -- height_px in opts.sizes is already the height after scaling (R-336)
      a.eq(20, find_block(m2, 'image').rows)
    end
  )

  -- R-157 ------------------------------------------------------------------

  local RICH = {
    '---',
    'title: x',
    '---',
    '',
    '# Head',
    '',
    'para *em* **st** `c` [l](#head) $\\alpha^2$ ref[^n]',
    '',
    '> [!TIP]',
    '> tip',
    '',
    '- [x] a',
    '  - b',
    '',
    '| a | b |',
    '|:-|-:|',
    '| 1 | 2 |',
    '',
    '```lua',
    'local x = 1',
    '```',
    '',
    '![p](p.png)',
    '',
    '```mermaid',
    'graph LR',
    'A-->B',
    '```',
    '',
    '[^n]: note',
  }

  QA.check('R-157', 'Two buffers with the same content and options give equal models', function()
    local opts = { width = 70, theme = 'light', cell = CELL }
    local m1 = QA.layout(RICH, opts)
    local m2 = QA.layout(RICH, vim.deepcopy(opts))
    a.eq(m1, m2)
  end)

  QA.check(
    'R-157',
    'Repeated layout calls on one buffer give equal models and do not share mutable state',
    function()
      local buf = QA.buf(RICH)
      local opts = { width = 70, theme = 'dark' }
      local m1 = km().layout(buf, opts)
      local snapshot = vim.deepcopy(m1)
      local m2 = km().layout(buf, opts)
      a.eq(m1, m2)
      a.eq(snapshot, m1, 'the first model changed after the second call')
      table.insert(m2.lines, 'mutated')
      local m3 = km().layout(buf, opts)
      a.eq(snapshot, m3, 'mutating one model changed the next')
    end
  )

  QA.check(
    'R-157',
    'The model does not depend on the cursor, the current window, or the current buffer',
    function()
      local buf = QA.buf(RICH)
      local opts = { width = 70, theme = 'light' }
      local m1 = km().layout(buf, opts)
      vim.cmd('vsplit')
      vim.api.nvim_win_set_buf(0, buf)
      pcall(vim.api.nvim_win_set_cursor, 0, { 10, 0 })
      vim.cmd('vertical resize 30')
      local m2 = km().layout(buf, opts)
      vim.cmd('enew')
      local m3 = km().layout(buf, opts)
      a.eq(m1, m2)
      a.eq(m1, m3)
    end
  )

  QA.check(
    'R-157',
    'The model does not depend on the time of the call',
    { severity = 'medium' },
    function()
      local buf = QA.buf(RICH)
      local m1 = km().layout(buf, { width = 70, theme = 'light' })
      QA.wait(1100, function()
        return false
      end)
      local m2 = km().layout(buf, { width = 70, theme = 'light' })
      a.eq(m1, m2)
    end
  )

  -- R-203 ------------------------------------------------------------------

  QA.check('R-203', 'layout() accepts all six option keys together', function()
    local dir = QA.tmpdir()
    local model
    a.no_error(function()
      model = QA.layout(
        IMG_DOC,
        { width = 60, cell = CELL, sizes = {}, theme = 'dark', images = true, dir = dir }
      )
    end)
    a.eq('table', type(model.lines))
  end)

  QA.check('R-203', 'The width option changes where paragraphs wrap', function()
    local long = { string.rep('word ', 40) }
    local narrow = QA.layout(long, { width = 40 })
    local wide = QA.layout(long, { width = 120 })
    a.truthy(#narrow.lines > #wide.lines, 'narrow layout has more lines')
    for _, l in ipairs(narrow.lines) do
      a.le(40, vim.api.nvim_strwidth(l), 'line fits the window width')
    end
  end)

  QA.check('R-203', 'The cell option is used to turn pixel heights into rows', function()
    local model = QA.layout(IMG_DOC, { width = 60 })
    local key = block_key(find_block(model, 'image'))
    QA.need(key, 'the block key in the descriptor')
    local c10 = { width_px = 5, height_px = 10 }
    local m1 = QA.layout(IMG_DOC, { width = 60, cell = CELL, sizes = { [key] = size(100, 100) } })
    local m2 = QA.layout(IMG_DOC, { width = 60, cell = c10, sizes = { [key] = size(100, 100) } })
    a.ne(find_block(m1, 'image').rows, find_block(m2, 'image').rows)
  end)

  QA.check('R-203', 'The theme option changes the block keys of pixel blocks', function()
    local m1 = QA.layout(IMG_DOC, { width = 60, theme = 'light' })
    local m2 = QA.layout(IMG_DOC, { width = 60, theme = 'dark' })
    local k1, k2 = block_key(find_block(m1, 'mermaid')), block_key(find_block(m2, 'mermaid'))
    QA.need(k1, 'the block key in the descriptor')
    a.ne(k1, k2)
  end)

  QA.check(
    'R-203',
    'The dir option is the base for a relative image path',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local model = QA.layout({ '![a](sub/pic.png)' }, { width = 60, dir = dir })
      local b = find_block(model, 'image')
      a.not_nil(b)
      local resolved = vim.fs.normalize(dir .. '/sub/pic.png')
      local hit = vim.fs.normalize(b.source) == resolved
      for _, l in ipairs(model.links or {}) do
        local _, _, _, _, target = U.link_parts(l)
        if type(target) == 'string' and vim.fs.normalize(target) == resolved then
          hit = true
        end
      end
      a.truthy(hit, 'resolved against dir: ' .. tostring(b.source))
    end
  )

  QA.check('R-203', 'The images option is accepted with both true and false', function()
    a.no_error(function()
      QA.layout(IMG_DOC, { width = 60, images = false })
      QA.layout(IMG_DOC, { width = 60, images = true })
    end)
  end)

  -- R-204 ------------------------------------------------------------------

  QA.check(
    { 'R-204', 'R-335' },
    'Each block descriptor exposes its block key as a lowercase SHA-256 hex string',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      for _, b in ipairs(model.blocks) do
        local key = block_key(b)
        a.not_nil(key, 'block key on the ' .. tostring(b.kind) .. ' descriptor')
        a.matches('^[0-9a-f]+$', key)
      end
    end
  )

  QA.check('R-204', 'A size keyed by a value other than the block key does not apply', function()
    local wrong = string.rep('ab', 32)
    local m = QA.layout(IMG_DOC, {
      width = 60,
      cell = CELL,
      sizes = {
        [wrong] = size(100, 100),
        [1] = size(100, 100),
        ['pics/one.png'] = size(100, 100),
      },
    })
    for _, b in ipairs(m.blocks) do
      a.is_nil(b.rows, 'rows on ' .. tostring(b.kind))
      a.is_nil(b.height_px, 'height_px on ' .. tostring(b.kind))
    end
  end)

  QA.check('R-204', 'The block key stays the same when content above the block changes', function()
    local m1 = QA.layout(IMG_DOC, { width = 60, theme = 'light' })
    local moved =
      vim.list_extend({ '# New heading', '', 'new paragraph', '' }, vim.deepcopy(IMG_DOC))
    local m2 = QA.layout(moved, { width = 60, theme = 'light' })
    local k1, k2 = block_key(find_block(m1, 'image')), block_key(find_block(m2, 'image'))
    QA.need(k1, 'the block key in the descriptor')
    a.eq(k1, k2)
    a.eq(block_key(find_block(m1, 'mermaid')), block_key(find_block(m2, 'mermaid')))
  end)

  QA.check('R-204', 'The block key changes with the source and with the width in cells', function()
    local m1 = QA.layout(IMG_DOC, { width = 60, theme = 'light' })
    local k = block_key(find_block(m1, 'mermaid'))
    QA.need(k, 'the block key in the descriptor')
    local changed = vim.deepcopy(IMG_DOC)
    changed[9] = '  A-->C'
    local m2 = QA.layout(changed, { width = 60, theme = 'light' })
    local m3 = QA.layout(IMG_DOC, { width = 70, theme = 'light' })
    a.ne(k, block_key(find_block(m2, 'mermaid')), 'source change')
    a.ne(k, block_key(find_block(m3, 'mermaid')), 'width change')
  end)

  QA.check(
    'R-204',
    'The block keys of an image and a mermaid block differ',
    { severity = 'medium' },
    function()
      local m = QA.layout(IMG_DOC, { width = 60 })
      a.ne(block_key(find_block(m, 'image')), block_key(find_block(m, 'mermaid')))
    end
  )

  -- R-205 ------------------------------------------------------------------

  QA.check('R-205', 'source_lines has one entry for every preview line', function()
    local model = QA.layout(RICH, { width = 50 })
    a.eq('table', type(model.source_lines))
    for i = 1, #model.lines do
      local v = model.source_lines[i]
      a.eq('number', type(v), 'source line of preview line ' .. i)
      a.truthy(v >= 1 and v <= #RICH, 'source line in range (1-based): ' .. tostring(v))
    end
    a.is_nil(model.source_lines[0], 'no entry for a line 0')
  end)

  -- Preview lines and source lines are both 1-based (R-337).
  local function maps_to(model, needle, src1)
    local i = line_of(model, needle)
    a.not_nil(i, 'preview line with ' .. needle)
    local v = model.source_lines[i]
    return v == src1,
      ('preview line %d -> %s, expected source line %d'):format(i, tostring(v), src1)
  end

  QA.check('R-205', 'A heading after front matter maps to its own source line', function()
    local ok, msg = maps_to(QA.layout(RICH, { width = 50 }), 'Head', 5)
    a.truthy(ok, msg)
  end)

  QA.check('R-205', 'Each code line in a fence maps to its own source line', function()
    local doc = { 'para', '', '```lua', 'first_code()', 'second_code()', 'third_code()', '```' }
    local model = QA.layout(doc, { width = 50 })
    for k, name in ipairs({ 'first_code', 'second_code', 'third_code' }) do
      local ok, msg = maps_to(model, name, 3 + k)
      a.truthy(ok, msg)
    end
  end)

  QA.check(
    'R-205',
    'Every wrapped line of a paragraph maps into the source lines of that paragraph',
    function()
      local doc = {
        '# T',
        '',
        'alpha ' .. string.rep('word ', 20),
        'beta ' .. string.rep('more ', 20),
        'gamma end',
        '',
        'next',
      }
      local model = QA.layout(doc, { width = 30 })
      local first = line_of(model, 'alpha')
      local last = line_of(model, 'end')
      a.truthy(first and last and last > first + 1, 'paragraph wraps')
      for i = first, last do
        local v = model.source_lines[i]
        a.truthy(v and v >= 3 and v <= 5, ('line %d -> %s'):format(i, tostring(v)))
      end
      local ok, msg = maps_to(model, 'next', 7)
      a.truthy(ok, msg)
    end
  )

  QA.check('R-205', 'List items and table rows map to their own source lines', function()
    local doc =
      { '- item one', '- item two', '', '| h1 | h2 |', '| -- | -- |', '| c1 | c2 |', '| d1 | d2 |' }
    local model = QA.layout(doc, { width = 50 })
    for needle, src in pairs({ ['item one'] = 1, ['item two'] = 2, h1 = 4, c1 = 6, d1 = 7 }) do
      local ok, msg = maps_to(model, needle, src)
      a.truthy(ok, msg)
    end
  end)

  QA.check(
    'R-205',
    'The anchor line of a pixel block maps to the source line of that block',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      local b = find_block(model, 'image')
      a.eq(3, model.source_lines[b.line], 'anchor line source')
    end
  )

  -- R-206 ------------------------------------------------------------------

  local function anchor(model, slug)
    a.eq('table', type(model.anchors), 'anchors table')
    return model.anchors[slug]
  end

  local function check_anchor(model, slug, needle)
    local v = anchor(model, slug)
    a.not_nil(v, 'anchor ' .. slug .. ' in ' .. vim.inspect(vim.tbl_keys(model.anchors)))
    local l = model.lines[v]
    a.truthy(l and l:find(needle, 1, true), 'anchor line shows the heading ' .. needle)
  end

  QA.check('R-206', 'A heading slug is lower case with spaces turned into hyphens', function()
    local model = QA.layout(
      { '# Hello World', '', 'text', '', '## Second Part Here' },
      { width = 60 }
    )
    check_anchor(model, 'hello-world', 'Hello World')
    check_anchor(model, 'second-part-here', 'Second Part Here')
  end)

  QA.check('R-206', 'Duplicate headings get -1, -2 suffixes in document order', function()
    local model = QA.layout(
      { '# Notes', '', 'a', '', '## Notes', '', 'b', '', '### Notes' },
      { width = 60 }
    )
    local v0, v1, v2 = anchor(model, 'notes'), anchor(model, 'notes-1'), anchor(model, 'notes-2')
    a.not_nil(v0, 'notes')
    a.not_nil(v1, 'notes-1')
    a.not_nil(v2, 'notes-2')
    a.truthy(v0 < v1 and v1 < v2, 'in document order')
  end)

  QA.check(
    'R-206',
    'Punctuation other than hyphens and underscores is dropped from a slug',
    function()
      local model = QA.layout(
        { "# What's New?", '', '## a-b_c (draft)!', '', '## C++ & Rust' },
        { width = 60 }
      )
      check_anchor(model, 'whats-new', 'New')
      check_anchor(model, 'a-b_c-draft', 'draft')
      check_anchor(model, 'c--rust', 'Rust')
    end
  )

  QA.check('R-206', 'Inline markup in a heading does not appear in its slug', function()
    local model = QA.layout(
      { '# Use `foo()` *now*', '', '## A [link](https://x.y) here' },
      { width = 60 }
    )
    check_anchor(model, 'use-foo-now', 'foo()')
    check_anchor(model, 'a-link-here', 'here')
  end)

  QA.check('R-206', 'Digits and non-ASCII letters stay in a slug', function()
    local model = QA.layout({ '# 1. Intro', '', '## Café Über' }, { width = 60 })
    check_anchor(model, '1-intro', 'Intro')
    check_anchor(model, 'café-über', 'Caf')
  end)

  QA.check('R-206', 'Setext headings and all six ATX levels get anchors', function()
    local model = QA.layout(
      { 'Setext Title', '============', '', 'Sub Title', '---------', '', '###### Deep One' },
      { width = 60 }
    )
    check_anchor(model, 'setext-title', 'Setext Title')
    check_anchor(model, 'sub-title', 'Sub Title')
    check_anchor(model, 'deep-one', 'Deep One')
  end)

  QA.check(
    { 'R-206', 'R-275' },
    'A slug already taken by a suffixed duplicate gets the next free suffix, as github-slugger does',
    function()
      local model = QA.layout({ '# Item', '', '# Item', '', '# Item 1' }, { width = 60 })
      local third
      for i, l in ipairs(model.lines) do
        if l:find('Item 1', 1, true) then
          third = i
        end
      end
      a.eq(1, model.anchors['item'], vim.inspect(model.anchors))
      a.eq(3, model.anchors['item-1'], vim.inspect(model.anchors))
      a.eq(third, model.anchors['item-1-1'], vim.inspect(model.anchors))
    end
  )

  -- R-207 ------------------------------------------------------------------

  local function link_with(model, pred)
    for _, l in ipairs(model.links or {}) do
      local line, cs, ce, kind, target = U.link_parts(l)
      if pred(target, kind) then
        return { line = line, cs = cs, ce = ce, kind = kind, target = target }
      end
    end
  end

  local function covers(model, link, needle)
    local i = line_of(model, needle)
    if not i then
      return false
    end
    local l = model.lines[i]
    local s = l:find(needle, 1, true)
    local e = s + #needle - 1
    -- 1-based lines with 0-based end-exclusive byte columns (R-337)
    local line_ok = link.line == i
    local col_ok = link.cs == s - 1 and link.ce == e
    return line_ok and col_ok
  end

  QA.check(
    'R-207',
    'A URL link appears in links with its line, columns over the link text, kind, and target',
    function()
      local model = QA.layout(
        { 'see the [docs page](https://example.com/docs) now' },
        { width = 80 }
      )
      a.eq('table', type(model.links))
      local l = link_with(model, function(t)
        return t == 'https://example.com/docs'
      end)
      a.not_nil(l, 'link entry: ' .. vim.inspect(model.links))
      a.eq('string', type(l.kind))
      a.truthy(covers(model, l, 'docs page'), 'columns cover the link text: ' .. vim.inspect(l))
    end
  )

  QA.check(
    'R-207',
    'Links to a URL, a heading anchor, and a local file have different kinds',
    function()
      local model = QA.layout(
        { '# Top', '', 'a [u](https://x.y/z) b [h](#top) c [f](other.md) d' },
        { width = 80 }
      )
      local u = link_with(model, function(t)
        return t == 'https://x.y/z'
      end)
      local h = link_with(model, function(t)
        return type(t) == 'string' and t:match('top$') ~= nil
      end)
      local f = link_with(model, function(t)
        return type(t) == 'string' and t:match('other%.md$') ~= nil
      end)
      a.not_nil(u, 'url link')
      a.not_nil(h, 'anchor link')
      a.not_nil(f, 'file link')
      a.ne(u.kind, h.kind)
      a.ne(u.kind, f.kind)
      a.ne(h.kind, f.kind)
    end
  )

  QA.check('R-207', 'Every image, inline or block, appears in links', function()
    local model = QA.layout(
      { 'text ![inl](inline.png) more', '', '![blk](block.png)' },
      { width = 80 }
    )
    a.not_nil(
      link_with(model, function(t)
        return type(t) == 'string' and t:match('inline%.png$') ~= nil
      end),
      'inline image'
    )
    a.not_nil(
      link_with(model, function(t)
        return type(t) == 'string' and t:match('block%.png$') ~= nil
      end),
      'block image'
    )
  end)

  QA.check(
    'R-207',
    'Autolinks, bare URLs, and reference links appear in links with their targets',
    function()
      local model = QA.layout({
        'a <https://auto.example> b https://bare.example c [ref text][r]',
        '',
        '[r]: https://ref.example',
      }, { width = 120 })
      for _, t in ipairs({ 'https://auto.example', 'https://bare.example', 'https://ref.example' }) do
        a.not_nil(
          link_with(model, function(x)
            return x == t
          end),
          'link to ' .. t .. ': ' .. vim.inspect(model.links)
        )
      end
    end
  )

  QA.check('R-207', 'A link on a later line has that line in links', function()
    local model = QA.layout(
      { '# H', '', 'p1', '', 'p2', '', 'go [there](https://later.example)' },
      { width = 80 }
    )
    local l = link_with(model, function(t)
      return t == 'https://later.example'
    end)
    a.not_nil(l)
    a.truthy(covers(model, l, 'there'), vim.inspect(l))
  end)

  QA.check(
    'R-207',
    'A link with non-ASCII text before it has columns that still cover the link text',
    { severity = 'medium' },
    function()
      local model = QA.layout({ 'Größe café [ziel](https://z.example)' }, { width = 80 })
      local l = link_with(model, function(t)
        return t == 'https://z.example'
      end)
      a.not_nil(l)
      a.truthy(covers(model, l, 'ziel'), vim.inspect(l))
    end
  )

  -- R-208 ------------------------------------------------------------------

  local function region_ok(model, r, first_needle, last_needle)
    local fs, ls, lang = U.region_parts(r)
    local f = line_of(model, first_needle)
    local l = line_of(model, last_needle)
    -- 1-based line numbers (R-337)
    local ok = fs == f and ls == l
    return ok, lang, ('region %s-%s, code lines %d-%d'):format(tostring(fs), tostring(ls), f, l)
  end

  QA.check(
    'R-208',
    'A fenced code block has a region with its first line, last line, and language',
    function()
      local model = QA.layout(
        { 'para', '', '```python', 'line_a = 1', 'line_b = 2', 'line_c = 3', '```', '', 'after' },
        { width = 60 }
      )
      a.eq('table', type(model.code_regions))
      a.eq(1, #model.code_regions)
      local ok, lang, msg = region_ok(model, model.code_regions[1], 'line_a', 'line_c')
      a.truthy(ok, msg)
      a.eq('python', lang)
    end
  )

  QA.check(
    'R-208',
    'Each fenced code block gets its own region in document order, tilde fences included',
    function()
      local model = QA.layout({
        '```lua',
        'lua_one()',
        '```',
        '',
        'mid',
        '',
        '~~~rust',
        'rust_one()',
        'rust_two()',
        '~~~',
      }, { width = 60 })
      a.eq(2, #model.code_regions)
      local ok1, lang1, m1 = region_ok(model, model.code_regions[1], 'lua_one', 'lua_one')
      local ok2, lang2, m2 = region_ok(model, model.code_regions[2], 'rust_one', 'rust_two')
      a.truthy(ok1, m1)
      a.truthy(ok2, m2)
      a.eq('lua', lang1)
      a.eq('rust', lang2)
    end
  )

  QA.check('R-208', 'A fence without a language has a region with no language', function()
    local model = QA.layout({ '```', 'plain_code', '```' }, { width = 60 })
    a.eq(1, #model.code_regions)
    local ok, lang, msg = region_ok(model, model.code_regions[1], 'plain_code', 'plain_code')
    a.truthy(ok, msg)
    a.truthy(lang == nil or lang == '', 'language: ' .. vim.inspect(lang))
  end)

  QA.check(
    'R-208',
    'The language of a region is the info string word without attributes',
    { severity = 'medium' },
    function()
      local model = QA.layout({ '```js {linenos=true}', 'js_code()', '```' }, { width = 60 })
      local _, _, lang = U.region_parts(model.code_regions[1] or {})
      a.eq('js', lang)
    end
  )

  QA.check('R-338', 'An indented code block gets a code region over its lines', function()
    local model = QA.layout(
      { 'para', '', '    indented_one()', '    indented_two()', '', 'after' },
      { width = 60 }
    )
    a.eq(1, #model.code_regions, vim.inspect(model.code_regions))
    local ok, _, msg = region_ok(model, model.code_regions[1], 'indented_one', 'indented_two')
    a.truthy(ok, msg)
  end)

  QA.check('R-339', 'An indented code block has no language in its code region', function()
    local model = QA.layout({ 'para', '', '    indented_code()' }, { width = 60 })
    a.eq(1, #model.code_regions)
    local _, _, lang = U.region_parts(model.code_regions[1])
    a.truthy(lang == nil or lang == '', 'language: ' .. vim.inspect(lang))
  end)

  QA.check('R-338', 'A mermaid fence gets no code region', function()
    local model = QA.layout(IMG_DOC, { width = 60 })
    a.eq(0, #model.code_regions, vim.inspect(model.code_regions))
  end)

  QA.check(
    'R-338',
    'An empty fenced code block gets no code region; the others still do',
    function()
      local model = QA.layout(
        { '```lua', '```', '', '```lua', 'only_line()', '```' },
        { width = 60 }
      )
      a.eq(1, #model.code_regions, vim.inspect(model.code_regions))
      local ok, _, msg = region_ok(model, model.code_regions[1], 'only_line', 'only_line')
      a.truthy(ok, msg)
    end
  )

  -- R-332 to R-337 -----------------------------------------------------------

  QA.check(
    'R-332',
    'The width option is the preview window width: text wraps inside it with the margins',
    function()
      local W = 50
      local model = QA.layout({ string.rep('abcd ', 60) }, { width = W })
      local widest = 0
      for _, l in ipairs(model.lines) do
        local w = vim.api.nvim_strwidth((l:gsub('%s+$', '')))
        widest = math.max(widest, w)
        a.le(W - 2, w, 'line ends before the right margin')
      end
      a.ge(W - 8, widest, 'lines use the text width')
    end
  )

  QA.check(
    'R-333',
    'A cell size given as width_px and height_px turns heights into rows',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      local key = block_key(find_block(model, 'image'))
      QA.need(key, 'the block key in the descriptor')
      local m2 = QA.layout(IMG_DOC, {
        width = 60,
        cell = { width_px = 8, height_px = 16 },
        sizes = { [key] = size(80, 33) },
      })
      a.eq(3, find_block(m2, 'image').rows)
    end
  )

  QA.check(
    { 'R-334', 'R-155' },
    'A size entry with width_px, height_px, and image_id is copied to its descriptor',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      local key = block_key(find_block(model, 'mermaid'))
      QA.need(key, 'the block key in the descriptor')
      local m2 =
        QA.layout(IMG_DOC, { width = 60, cell = CELL, sizes = { [key] = size(320, 240, 77) } })
      local b = find_block(m2, 'mermaid')
      a.eq(320, b.width_px)
      a.eq(240, b.height_px)
      a.eq(77, b.image_id)
      a.eq(12, b.rows)
    end
  )

  QA.check(
    'R-335',
    'Every block descriptor has a key field with the lowercase SHA-256 hex block key',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      a.eq(2, #model.blocks)
      for _, b in ipairs(model.blocks) do
        a.eq('string', type(b.key), 'key on the ' .. tostring(b.kind) .. ' descriptor')
        a.matches('^[0-9a-f]+$', b.key)
        a.eq(64, #b.key)
      end
    end
  )

  QA.check(
    'R-336',
    'Reserved rows depend only on height_px and the cell height, not on width_px',
    function()
      local model = QA.layout(IMG_DOC, { width = 60 })
      local key = block_key(find_block(model, 'image'))
      QA.need(key, 'the block key in the descriptor')
      local narrow =
        QA.layout(IMG_DOC, { width = 60, cell = CELL, sizes = { [key] = size(50, 210) } })
      local wide =
        QA.layout(IMG_DOC, { width = 60, cell = CELL, sizes = { [key] = size(5000, 210) } })
      a.eq(11, find_block(narrow, 'image').rows)
      a.eq(11, find_block(wide, 'image').rows)
    end
  )

  QA.check(
    'R-337',
    'Model lines are 1-based: the first preview line is line 1 in highlights, anchors, and source lines',
    function()
      local model = QA.layout({ '# First Heading', '', 'text' }, { width = 60 })
      a.eq(1, model.anchors['first-heading'])
      a.eq(1, model.source_lines[1])
      local on_first = false
      for _, h in ipairs(model.highlights) do
        local line = U.hl_parts(h)
        a.truthy(line >= 1, 'no highlight on line 0')
        if line == 1 then
          on_first = true
        end
      end
      a.truthy(on_first, 'the heading highlight is on line 1')
    end
  )

  QA.check(
    'R-337',
    'Columns are 0-based byte offsets with an exclusive end, also after multibyte text',
    function()
      local model = QA.layout({ 'äöü €€ **target** end' }, { width = 80 })
      local i = line_of(model, 'target')
      local l = model.lines[i]
      local s = l:find('target', 1, true)
      local found = false
      for _, h in ipairs(model.highlights) do
        local line, cs, ce = U.hl_parts(h)
        if line == i and cs == s - 1 and ce == s - 1 + #'target' then
          found = true
        end
      end
      a.truthy(found, 'a highlight at byte columns ' .. (s - 1) .. '-' .. (s - 1 + #'target'))
    end
  )

  QA.check(
    'R-337',
    'Link columns are 0-based byte offsets with an exclusive end after multibyte text',
    function()
      local model = QA.layout({ '€ ä [here](https://m.example) z' }, { width = 80 })
      local l = link_with(model, function(t)
        return t == 'https://m.example'
      end)
      a.not_nil(l)
      a.truthy(covers(model, l, 'here'), vim.inspect(l))
    end
  )

  -- Block key definition -----------------------------------------------------

  QA.check('R-204', 'The block key of an image changes with its img tag attributes', function()
    local m1 = QA.layout({ '<img src="pic.png" width="100">' }, { width = 60, theme = 'light' })
    local m2 = QA.layout({ '<img src="pic.png" width="200">' }, { width = 60, theme = 'light' })
    local b1, b2 = find_block(m1, 'image'), find_block(m2, 'image')
    a.not_nil(b1, 'an img tag makes an image block')
    a.not_nil(b2, 'an img tag makes an image block')
    a.ne(b1.key, b2.key)
  end)

  QA.check(
    'R-204',
    'The block key of an image uses the resolved path: one relative path in two directories gives two keys',
    function()
      local d1, d2 = QA.tmpdir(), QA.tmpdir()
      local m1 = QA.layout(
        { '![a](pic.png)' },
        { width = 60, theme = 'light' },
        { name = d1 .. '/doc.md' }
      )
      local m2 = QA.layout(
        { '![a](pic.png)' },
        { width = 60, theme = 'light' },
        { name = d2 .. '/doc.md' }
      )
      local b1, b2 = find_block(m1, 'image'), find_block(m2, 'image')
      a.eq(
        vim.fs.normalize(d1 .. '/pic.png'),
        vim.fs.normalize(b1.source),
        'source is the resolved path'
      )
      a.eq(
        vim.fs.normalize(d2 .. '/pic.png'),
        vim.fs.normalize(b2.source),
        'source is the resolved path'
      )
      a.ne(b1.key, b2.key)
    end
  )

  QA.check(
    'R-204',
    'Two images with the same resolved path and attributes share one block key',
    { severity = 'medium' },
    function()
      local d = QA.tmpdir()
      local m = QA.layout(
        { '![a](pic.png)', '', 'text', '', '![b](' .. d .. '/pic.png)' },
        { width = 60, theme = 'light' },
        { name = d .. '/doc.md' }
      )
      a.eq(2, #m.blocks)
      a.eq(m.blocks[1].key, m.blocks[2].key)
    end
  )

  -- R-209 ------------------------------------------------------------------

  local TS_DOC = {
    '# Title',
    '',
    'para *em* **st** `code` [link](#title) $\\alpha$',
    '',
    '- a',
    '- b',
    '',
    '| x | y |',
    '| - | - |',
    '| 1 | 2 |',
    '',
    '```lua',
    'local x = 1',
    '```',
    '',
    '```python',
    'def f(x):',
    '    return x',
    '```',
    '',
    '```vim',
    'set nonumber',
    '```',
    '',
    '<div>html</div>',
    '',
    '> quote',
    '',
    '![i](i.png)',
  }

  local function child_model(mode)
    local dir = QA.tmpdir()
    local doc = dir .. '/doc.md'
    local out = dir .. '/model.txt'
    vim.fn.writefile(TS_DOC, doc)
    local script = dir .. '/child.lua'
    QA.write(
      script,
      table.concat({
        'local root, doc, out, mode = _G.arg[1], _G.arg[2], _G.arg[3], _G.arg[4]',
        'local keep = {}',
        'for _, p in ipairs(vim.opt.rtp:get()) do',
        '  local has = vim.fn.glob(p .. "/parser/*", false, true)',
        '  if #has == 0 or mode ~= "onlymd" then keep[#keep + 1] = p',
        '  else',
        '    -- keep only the bundled markdown parsers (the Constraints require them)',
        '    local d = vim.fn.tempname()',
        '    vim.fn.mkdir(d .. "/parser", "p")',
        '    for _, f in ipairs(has) do',
        '      if vim.fs.basename(f):match("^markdown") then vim.uv.fs_symlink(f, d .. "/parser/" .. vim.fs.basename(f)) end',
        '    end',
        '    keep[#keep + 1] = d',
        '  end',
        'end',
        'vim.opt.rtp = keep',
        'vim.opt.rtp:prepend(root)',
        'if mode == "extra" then',
        '  -- a parser for languages that have none in a clean Neovim',
        '  vim.treesitter.language.register("lua", { "python", "rust" })',
        'end',
        'local function has(lang) local ok, r = pcall(vim.treesitter.language.add, lang) return ok and r or false end',
        'local function parses(ft) local lang = vim.treesitter.language.get_lang(ft) return lang ~= nil and lang ~= ft and has(lang) end',
        'local probe = { md = has("markdown"), inl = has("markdown_inline"), lua = has("lua"), vim = has("vim"), python = parses("python") }',
        'local buf = vim.api.nvim_create_buf(true, false)',
        'vim.api.nvim_buf_set_lines(buf, 0, -1, false, vim.fn.readfile(doc))',
        'vim.bo[buf].filetype = "markdown"',
        'local ok, model = pcall(function() return require("kitty-markdown").layout(buf, { width = 60, theme = "light" }) end)',
        'local f = io.open(out, "w")',
        'f:write(vim.inspect({ probe = probe, ok = ok, model = ok and model or tostring(model) }))',
        'f:close()',
      }, '\n')
    )
    local res = QA.sh(
      { vim.v.progpath, '--clean', '-l', script, QA.root, doc, out, mode },
      { timeout = 20000 }
    )
    local text = QA.read(out)
    a.not_nil(text, 'child output; stderr: ' .. tostring(res.stderr))
    local fn = loadstring('return ' .. text)
    return fn()
  end

  local function parent_model()
    local buf = QA.buf(TS_DOC)
    return km().layout(buf, { width = 60, theme = 'light' })
  end

  local function control()
    local r = child_model('all')
    QA.need(
      r.ok and vim.deep_equal(r.model, parent_model()),
      'a child Neovim with all parsers that reproduces the parent model'
    )
  end

  QA.check(
    'R-209',
    'layout() gives the same model when only the bundled markdown parsers are installed',
    function()
      control()
      local r = child_model('onlymd')
      QA.need(
        r.probe.md and r.probe.inl and not r.probe.lua and not r.probe.vim,
        'a child Neovim with the markdown parsers and no code language parsers'
      )
      a.truthy(r.ok, 'layout raised an error: ' .. tostring(r.model))
      a.eq(parent_model(), r.model)
    end
  )

  QA.check(
    'R-209',
    'layout() gives the same model when extra code language parsers are installed',
    function()
      control()
      local r = child_model('extra')
      QA.need(r.probe.md and r.probe.python, 'a child Neovim with an extra code language parser')
      a.truthy(r.ok, 'layout raised an error: ' .. tostring(r.model))
      a.eq(parent_model(), r.model)
    end
  )

  -- R-210 ------------------------------------------------------------------

  local function suite(extra)
    local dir = QA.tmpdir()
    vim.fn.mkdir(dir .. '/tests', 'p')
    QA.sh({ 'cp', '-R', QA.root .. '/lua', dir .. '/lua' })
    QA.sh({ 'cp', QA.root .. '/tests/run.lua', dir .. '/tests/run.lua' })
    for name, body in pairs(extra or {}) do
      QA.write(dir .. '/tests/' .. name, body)
    end
    return dir
  end

  local function run_suite(dir, filter)
    local cmd = { vim.v.progpath, '-l', 'tests/run.lua' }
    if filter then
      cmd[#cmd + 1] = filter
    end
    local xdg = dir .. '/xdg'
    return QA.sh(cmd, {
      cwd = dir,
      timeout = 60000,
      env = {
        XDG_CONFIG_HOME = xdg .. '/config',
        XDG_DATA_HOME = xdg .. '/data',
        XDG_STATE_HOME = xdg .. '/state',
        XDG_CACHE_HOME = xdg .. '/cache',
      },
    })
  end

  local PASS =
    "describe('qa pass group', function()\n  it('qa passing case', function()\n    assert.are.same(1, 1)\n  end)\nend)\n"
  local FAIL =
    "describe('qa fail group', function()\n  it('qa failing case', function()\n    assert.are.same(1, 2)\n  end)\nend)\n"
  local ERR =
    "describe('qa error group', function()\n  it('qa erroring case', function()\n    error('qa runtime error')\n  end)\nend)\n"

  QA.check('R-210', 'The unit test runner exits with status 0 when every test passes', function()
    local dir = suite({ ['qa_pass_spec.lua'] = PASS })
    local res = run_suite(dir)
    a.eq(0, res.code, (res.stdout or '') .. (res.stderr or ''))
  end)

  QA.check('R-210', 'The unit test runner exits non-zero when a test assertion fails', function()
    local dir = suite({ ['qa_pass_spec.lua'] = PASS, ['qa_fail_spec.lua'] = FAIL })
    local res = run_suite(dir)
    a.ne(0, res.code, 'exit status')
  end)

  QA.check(
    'R-210',
    'The unit test runner exits non-zero when a test raises a runtime error',
    function()
      local dir = suite({ ['qa_error_spec.lua'] = ERR })
      local res = run_suite(dir)
      a.ne(0, res.code, 'exit status')
    end
  )

  QA.check('R-210', 'The unit test runner exits non-zero when a spec file does not load', function()
    local dir = suite({ ['qa_pass_spec.lua'] = PASS, ['qa_broken_spec.lua'] = 'describe(\n' })
    local res = run_suite(dir)
    a.ne(0, res.code, 'exit status')
  end)

  QA.check('R-210', 'The filter argument narrows the run to the matching tests', function()
    local dir = suite({ ['qa_pass_spec.lua'] = PASS, ['qa_fail_spec.lua'] = FAIL })
    local only_pass = run_suite(dir, 'qa_pass')
    local out = (only_pass.stdout or '') .. (only_pass.stderr or '')
    a.eq(0, only_pass.code, 'a filter that excludes the failing test: ' .. out)
    a.contains('qa passing case', out)
    a.not_contains('qa failing case', out)
    local only_fail = run_suite(dir, 'qa_fail')
    a.ne(0, only_fail.code, 'a filter that selects the failing test')
  end)

  QA.check(
    'R-210',
    'The runner runs the repository unit tests and fails the run when one more failing test is added',
    { timeout_ms = 90000 },
    function()
      local dir = QA.tmpdir()
      QA.sh({ 'cp', '-R', QA.root .. '/lua', QA.root .. '/plugin', QA.root .. '/tests', dir .. '/' })
      local quick = run_suite(dir, 'config')
      local out = (quick.stdout or '') .. (quick.stderr or '')
      a.eq(0, quick.code, 'the repository config tests pass: ' .. out:sub(-400))
      a.matches('[1-9]%d* passed', out, 'some repository tests ran')
      QA.write(dir .. '/tests/zz_qa_fail_spec.lua', FAIL)
      local full = run_suite(dir)
      a.ne(0, full.code, 'exit status with one failing test added')
    end
  )
end)
