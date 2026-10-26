local QA = require('qa')
local U = require('transport_util')
local a = QA.assert

local function T()
  return require('kitty-markdown.transport')
end

local function need_fn(name)
  QA.need(type(T()[name]) == 'function', 'transport.' .. name .. ' (module contract)')
  return T()[name]
end

-- A PNG large enough to need several chunks (stored zlib blocks, so the
-- file is about 4 bytes per pixel).
local function big_png(dir)
  return QA.make_png(dir .. '/big.png', 90, 60, { 10, 200, 90, 255 })
end

QA.describe('transport: sequences', function()
  local cap, dir
  QA.before_each(function()
    cap, dir = U.env({ helper = false })
  end)
  QA.after_each(function()
    QA.reset_plugin()
    vim.env.TMUX = nil
  end)

  QA.check(
    'R-123',
    'A transmitted image is split into base64 chunks of at most 4096 bytes',
    function()
      local transmit = need_fn('transmit')
      local path = big_png(dir)
      transmit(4242, path)
      local tx = U.transmissions(cap)
      a.eq(1, #tx, 'transmissions')
      a.ge(3, #tx[1].chunks, 'a large image needs several chunks')
      for i, c in ipairs(tx[1].chunks) do
        a.le(4096, #c.payload, 'payload size of chunk ' .. i)
        a.truthy(#c.payload > 0, 'chunk ' .. i .. ' is not empty')
      end
    end
  )

  QA.check('R-123', 'Every chunk but the last carries m=1 and the last carries m=0', function()
    local transmit = need_fn('transmit')
    transmit(4243, big_png(dir))
    local chunks = U.transmissions(cap)[1].chunks
    for i = 1, #chunks - 1 do
      a.eq('1', chunks[i].control.m, 'm on chunk ' .. i)
    end
    local last = chunks[#chunks].control.m
    a.truthy(last == '0' or last == nil, 'the last chunk has m=0 (or no m), got ' .. tostring(last))
  end)

  QA.check('R-123', 'The decoded chunks give back the PNG file byte for byte', function()
    local transmit = need_fn('transmit')
    local path = big_png(dir)
    transmit(4244, path)
    local tx = U.transmissions(cap)[1]
    a.not_nil(tx.data, 'the joined payload is valid base64')
    a.eq(#QA.read(path), #tx.data, 'decoded size')
    a.truthy(tx.data == QA.read(path), 'decoded bytes equal the file')
  end)

  QA.check('R-123', 'A transmission is quiet: every chunk carries q=2', function()
    local transmit = need_fn('transmit')
    transmit(4245, big_png(dir))
    local tx = U.transmissions(cap)[1]
    a.ge(3, #tx.chunks, 'chunks')
    for i, c in ipairs(tx.chunks) do
      a.eq('2', c.control.q, 'q on chunk ' .. i)
    end
  end)

  QA.check(
    'R-123',
    'Chunks other than the last hold a multiple of 4 base64 bytes, as kitty requires',
    { severity = 'medium' },
    function()
      local transmit = need_fn('transmit')
      transmit(4246, big_png(dir))
      local chunks = U.transmissions(cap)[1].chunks
      for i = 1, #chunks - 1 do
        a.eq(0, #chunks[i].payload % 4, 'size of chunk ' .. i .. ' modulo 4')
      end
    end
  )

  QA.check(
    'R-123',
    'Continuation chunks carry only the m and q keys, as kitty requires',
    { severity = 'medium' },
    function()
      local transmit = need_fn('transmit')
      transmit(4247, big_png(dir))
      local chunks = U.transmissions(cap)[1].chunks
      for i = 2, #chunks do
        for _, k in ipairs(chunks[i].order) do
          a.truthy(k == 'm' or k == 'q', 'key ' .. k .. ' on continuation chunk ' .. i)
        end
      end
    end
  )

  QA.check('R-123', 'A small image goes in one chunk that is also the last chunk', function()
    local transmit = need_fn('transmit')
    local path = QA.make_png(dir .. '/small.png', 4, 4)
    transmit(4248, path)
    local tx = U.transmissions(cap)
    a.eq(1, #tx, 'transmissions')
    a.eq(1, #tx[1].chunks, 'chunks')
    local m = tx[1].first.control.m
    a.truthy(m == '0' or m == nil, 'a single chunk has m=0 or no m, got ' .. tostring(m))
    a.truthy(tx[1].data == QA.read(path), 'payload equals the file')
  end)

  QA.check(
    'R-123',
    'Every graphics sequence of a transmission is terminated with the string terminator',
    function()
      local transmit = need_fn('transmit')
      transmit(4249, big_png(dir))
      for _, c in ipairs(U.commands(cap)) do
        a.falsy(c.unterminated, 'unterminated APC')
      end
      local text = cap:text()
      local opens = select(2, text:gsub('\27_G', ''))
      a.eq(#U.commands(cap), opens, 'every APC start parses as one command')
    end
  )

  QA.check(
    'R-189',
    'Image data goes with direct transmission (t=d) in PNG format (f=100)',
    function()
      local transmit = need_fn('transmit')
      transmit(4250, big_png(dir))
      local first = U.transmissions(cap)[1].first
      a.eq('d', first.control.t, 't on the first chunk')
      a.eq('100', first.control.f, 'f on the first chunk')
    end
  )

  QA.check(
    'R-189',
    'No transmission uses a file, temporary file, or shared memory medium, and no path appears in the bytes',
    function()
      local transmit = need_fn('transmit')
      local path = big_png(dir)
      transmit(4251, path)
      for _, c in ipairs(U.commands(cap)) do
        local t = c.control.t
        a.truthy(t == nil or t == 'd', 'transmission medium ' .. tostring(t))
      end
      local b64path = vim.base64.encode(path)
      a.not_contains(path, cap:text(), 'the file path in the bytes')
      a.not_contains(b64path, cap:text(), 'the base64 file path in the bytes')
    end
  )

  QA.check(
    'R-189',
    'Transmission does not display the image at the cursor: the action is a=t, or a=T only with a virtual placement',
    function()
      local transmit = need_fn('transmit')
      transmit(4252, big_png(dir))
      local first = U.transmissions(cap)[1].first
      local act = first.control.a
      a.truthy(act == 't' or (act == 'T' and first.control.U == '1'), 'action ' .. tostring(act))
    end
  )

  QA.check(
    'R-124',
    'A placement is a virtual placement with U=1, the image id, and the column and row counts',
    function()
      local place = need_fn('place')
      place(777, 13, 5)
      local p = U.placements(cap)
      a.eq(1, #p, 'virtual placements')
      a.eq('777', p[1].control.i, 'image id')
      a.eq('13', p[1].control.c, 'columns')
      a.eq('5', p[1].control.r, 'rows')
    end
  )

  QA.check(
    'R-124',
    'A placement sends no image data and is quiet',
    { severity = 'medium' },
    function()
      local place = need_fn('place')
      place(778, 3, 2)
      local p = U.placements(cap)[1]
      a.not_nil(p, 'placement')
      if p.control.a == 'p' then
        a.eq('', p.payload, 'payload of a=p')
      end
      a.eq('2', p.control.q, 'quiet flag so no reply reaches Neovim')
    end
  )

  QA.check(
    { 'R-128', 'R-308' },
    'Deleting an image sends a delete action with d=I for that image id',
    function()
      local delete = need_fn('delete')
      delete(9001)
      local d = U.deletes(cap)
      a.eq(1, #d, 'delete commands')
      local how = d[1].control.d
      a.eq('I', how, 'delete by id that also frees the data')
      a.eq('9001', d[1].control.i, 'image id')
    end
  )

  QA.check(
    'R-125',
    'Placeholder rows hold one U+10EEEE cell per column, with the row and column diacritics from the kitty table',
    function()
      local rows_fn = need_fn('placeholder_rows')
      local id = 1234567
      local rows = rows_fn(id, 7, 4)
      a.eq(4, #rows, 'rows')
      for r, line in ipairs(rows) do
        local cells = U.decode_row(line)
        a.eq(7, #cells, 'cells in row ' .. r)
        for c, cell in ipairs(cells) do
          a.eq(r - 1, cell.row, ('row diacritic at %d,%d'):format(r, c))
          a.eq(c - 1, cell.col, ('column diacritic at %d,%d'):format(r, c))
        end
      end
    end
  )

  QA.check(
    'R-125',
    'The first cells match the kitty example: U+0305 for 0 and U+030D for 1',
    function()
      local rows_fn = need_fn('placeholder_rows')
      local rows = rows_fn(42, 2, 2)
      local ph = vim.fn.nr2char(0x10EEEE, 1)
      local function ch(cp)
        return vim.fn.nr2char(cp, 1)
      end
      local want1 = ph .. ch(0x305) .. ch(0x305) .. ph .. ch(0x305) .. ch(0x30D)
      local want2 = ph .. ch(0x30D) .. ch(0x305) .. ph .. ch(0x30D) .. ch(0x30D)
      a.eq(want1, vim.trim(rows[1]), 'row 0')
      a.eq(want2, vim.trim(rows[2]), 'row 1')
    end
  )

  QA.check(
    'R-125',
    'Placeholder rows reach the far end of the diacritic table correctly',
    function()
      local rows_fn = need_fn('placeholder_rows')
      local rows = rows_fn(99, 297, 297)
      a.eq(297, #rows, 'rows')
      local last = U.decode_row(rows[297])
      a.eq(297, #last, 'cells in the last row')
      a.eq(296, last[297].row, 'row diacritic of the last cell')
      a.eq(296, last[297].col, 'column diacritic of the last cell')
      local mid = U.decode_row(rows[150])
      a.eq(149, mid[200].row, 'row diacritic in the middle')
      a.eq(199, mid[200].col, 'column diacritic in the middle')
    end
  )

  QA.check(
    'R-125',
    'The diacritic table matches the 297 entries of kitty rowcolumn-diacritics.txt in order',
    function()
      local kitty = U.diacritics()
      a.eq(297, #kitty, 'entries in the kitty table fixture')
      a.eq(
        { 0x305, 0x30D, 0x30E },
        { kitty[1], kitty[2], kitty[3] },
        'the fixture starts as the kitty docs say'
      )
      local mine = T().DIACRITICS
      QA.need(type(mine) == 'table', 'transport.DIACRITICS (module contract)')
      a.eq(#kitty, #mine, 'table length')
      for i = 1, #kitty do
        a.eq(kitty[i], mine[i], 'diacritic for number ' .. (i - 1))
      end
    end
  )

  QA.check(
    'R-125',
    'The highlight group for an image id has the id as its foreground color',
    function()
      local hl = need_fn('hl_group')
      for _, id in ipairs({ 1, 42, 0xABCDEF, 8388608 }) do
        local fg = U.chunk_fg(hl(id))
        a.eq(id, fg, 'foreground for id ' .. id)
      end
    end
  )

  QA.check(
    'R-188',
    'Every placeholder cell carries both a row and a column diacritic, also when they repeat the cell on the left',
    function()
      local rows_fn = need_fn('placeholder_rows')
      for _, id in ipairs({ 5, 300000 }) do
        local rows = rows_fn(id, 9, 3)
        for r, line in ipairs(rows) do
          local cells = U.decode_row(line)
          for c, cell in ipairs(cells) do
            a.ge(2, cell.ndia, ('diacritics on cell %d,%d'):format(r, c))
          end
          local n = select(2, line:gsub(U.PH, ''))
          a.eq(#cells, n, 'every U+10EEEE in the row decodes with diacritics')
        end
      end
    end
  )

  QA.check(
    'R-126',
    'An image id above 24 bits puts its high byte in a third diacritic, as in the kitty example',
    function()
      local rows_fn = need_fn('placeholder_rows')
      local hl = need_fn('hl_group')
      local id = 42 + 2 * 2 ^ 24
      local rows = rows_fn(id, 2, 2)
      for r, line in ipairs(rows) do
        local cells = U.decode_row(line)
        a.eq(2, #cells, 'cells')
        for c, cell in ipairs(cells) do
          a.eq(3, cell.ndia, ('diacritics on cell %d,%d'):format(r, c))
          a.eq(2, cell.msb, ('high byte diacritic on cell %d,%d'):format(r, c))
        end
      end
      a.eq(42, U.chunk_fg(hl(id)), 'foreground carries the low 24 bits')
    end
  )

  QA.check('R-126', 'The largest high byte (255) is encoded with the matching diacritic', function()
    local rows_fn = need_fn('placeholder_rows')
    local hl = need_fn('hl_group')
    local id = 255 * 2 ^ 24 + 77
    local cells = U.decode_row(rows_fn(id, 3, 1)[1])
    a.eq(3, #cells, 'cells')
    for _, cell in ipairs(cells) do
      a.eq(255, cell.msb, 'high byte diacritic')
    end
    a.eq(77, U.chunk_fg(hl(id)), 'foreground')
  end)

  QA.check('R-126', 'An id of exactly 2^24 carries the high byte 1', function()
    local rows_fn = need_fn('placeholder_rows')
    local id = 2 ^ 24 + 1
    local cells = U.decode_row(rows_fn(id, 1, 1)[1])
    a.eq(1, cells[1].msb, 'high byte diacritic')
  end)

  QA.check(
    'R-122',
    'Inside tmux every graphics sequence is wrapped in a DCS passthrough with the inner escape bytes doubled',
    function()
      local transmit, place, delete = need_fn('transmit'), need_fn('place'), need_fn('delete')
      vim.env.TMUX = '/tmp/tmux-501/default,1234,0'
      local path = big_png(dir)
      transmit(5001, path)
      place(5001, 4, 3)
      delete(5001)
      local text = cap:text()
      local wraps, outside = U.split_tmux(text)
      a.no_match('\27_G', outside, 'a bare graphics sequence outside the passthrough')
      a.ge(5, #wraps, 'passthrough wrappers')
      for i, w in ipairs(wraps) do
        a.truthy(w.ok, 'wrapper ' .. i .. ' doubles every inner ESC and is terminated')
        a.matches('^\27_G', w.inner, 'wrapper ' .. i .. ' holds a graphics sequence')
        a.eq(w.inner:sub(-2), '\27\\', 'wrapper ' .. i .. ' inner sequence ends with ST')
      end
      local tx = U.transmissions(cap)
      a.eq(1, #tx, 'transmissions after unwrapping')
      a.truthy(tx[1].data == QA.read(path), 'the unwrapped payload equals the file')
      a.eq(1, #U.placements(cap), 'placement after unwrapping')
      a.eq(1, #U.deletes(cap), 'delete after unwrapping')
    end
  )

  QA.check(
    'R-122',
    'Each wrapper holds exactly one graphics sequence',
    { severity = 'medium' },
    function()
      local transmit = need_fn('transmit')
      vim.env.TMUX = '/tmp/tmux-501/default,1234,0'
      transmit(5002, big_png(dir))
      local wraps = U.split_tmux(cap:text())
      for i, w in ipairs(wraps) do
        a.eq(1, #U.parse_apc(w.inner), 'graphics sequences in wrapper ' .. i)
      end
    end
  )

  QA.check('R-122', 'Outside tmux no graphics sequence is wrapped', function()
    local transmit, place = need_fn('transmit'), need_fn('place')
    vim.env.TMUX = nil
    transmit(5003, big_png(dir))
    place(5003, 2, 2)
    a.not_contains('\27Ptmux;', cap:text(), 'tmux passthrough without TMUX')
    a.ge(2, #U.commands(cap), 'graphics commands')
  end)

  QA.check(
    'R-183',
    'The cell size is the terminal window pixel size divided by its rows and columns',
    function()
      local cell_size = need_fn('cell_size')
      T().set_winsize_reader(function()
        return 30, 100, 900, 1800
      end)
      local c, h = cell_size()
      local w
      if type(c) == 'table' then
        w, h = c.width_px or c.width or c[1], c.height_px or c.height or c[2]
      else
        w = c
      end
      a.eq(9, w, 'cell width')
      a.eq(60, h, 'cell height')
    end
  )

  QA.check('R-184', 'A zero pixel size gives no cell size', function()
    local cell_size = need_fn('cell_size')
    T().set_winsize_reader(function()
      return 24, 80, 0, 0
    end)
    local c = cell_size()
    local ok = c == nil
      or c == false
      or (type(c) == 'table' and not ((c.width_px or 0) > 0 and (c.height_px or 0) > 0))
    a.truthy(ok, 'cell size from a zero pixel size: ' .. vim.inspect(c))
  end)

  QA.check(
    'R-186',
    'Image ids are allocated in increasing order, starting between 1 and 2^23',
    function()
      local alloc = need_fn('alloc_id')
      local first = alloc()
      a.ge(1, first, 'first id')
      a.le(2 ^ 23, first, 'first id')
      local prev = first
      for _ = 1, 50 do
        local id = alloc()
        a.truthy(id > prev, ('id %d after %d'):format(id, prev))
        prev = id
      end
    end
  )

  QA.check('R-186', 'Each new Neovim instance starts its ids between 1 and 2^23', function()
    local script = dir .. '/first_id.lua'
    QA.write(
      script,
      table.concat({
        'vim.opt.runtimepath:prepend(' .. vim.inspect(QA.root) .. ')',
        'local id = require("kitty-markdown.transport").alloc_id()',
        'io.stdout:write(tostring(id))',
      }, '\n')
    )
    for _ = 1, 3 do
      local res = QA.sh({ vim.v.progpath, '--clean', '-l', script }, { timeout = 10000 })
      local id = tonumber(vim.trim(res.stdout or ''))
      a.not_nil(id, 'first id printed: ' .. tostring(res.stdout) .. tostring(res.stderr))
      a.ge(1, id, 'first id')
      a.le(2 ^ 23, id, 'first id')
    end
  end)

  QA.check('R-187', 'No image id repeats over many allocations, deletes included', function()
    local alloc, delete = need_fn('alloc_id'), need_fn('delete')
    local seen = {}
    for i = 1, 3000 do
      local id = alloc()
      a.falsy(seen[id], 'id ' .. id .. ' allocated twice')
      seen[id] = true
      if i % 7 == 0 then
        delete(id)
      end
    end
  end)

  QA.check(
    'R-129',
    'Allocated ids are valid kitty image ids: positive and within 32 bits',
    function()
      local alloc = need_fn('alloc_id')
      for _ = 1, 20 do
        local id = alloc()
        a.truthy(id == math.floor(id), 'integer id')
        a.ge(1, id, 'id')
        a.le(4294967295, id, 'id')
      end
    end
  )
end)
