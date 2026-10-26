-- Helpers for the hidden transport checks: parse the bytes the plugin
-- writes to the terminal, drive the probe, and read the preview rows.

local QA = require('qa')

local U = {}

U.ESC = '\27'
U.ST = '\27\\'
U.PLACEHOLDER = 0x10EEEE

-- kitty rowcolumn-diacritics.txt, index 0 first.
function U.diacritics()
  if U._diacritics then
    return U._diacritics
  end
  local list = {}
  for line in (QA.read(QA.evals .. '/transport/diacritics.txt') or ''):gmatch('[^\n]+') do
    if not line:match('^#') then
      table.insert(list, tonumber(line, 16))
    end
  end
  U._diacritics = list
  return list
end

-- Index (0-based) of a diacritic code point, or nil.
function U.diacritic_index(cp)
  if not U._dindex then
    U._dindex = {}
    for i, c in ipairs(U.diacritics()) do
      U._dindex[c] = i - 1
    end
  end
  return U._dindex[cp]
end

-- Code points of a UTF-8 string.
function U.codepoints(s)
  local out = {}
  local n = vim.fn.strchars(s)
  for i = 0, n - 1 do
    table.insert(out, vim.fn.strgetchar(s, i))
  end
  return out
end

-- Parses a control string `k=v,k=v` into a table plus the key order.
local function parse_control(ctrl)
  local t, order = {}, {}
  for kv in ctrl:gmatch('[^,]+') do
    local k, v = kv:match('^(%w+)=(.*)$')
    if k then
      t[k] = v
      table.insert(order, k)
    else
      t[kv] = true
      table.insert(order, kv)
    end
  end
  return t, order
end

-- Splits tmux DCS passthrough wrappers out of a byte stream. Returns the
-- list of { inner = <unwrapped bytes>, ok = <every inner ESC doubled> } and
-- the bytes outside any wrapper.
function U.split_tmux(text)
  local wraps, outside = {}, {}
  local i = 1
  local open = U.ESC .. 'Ptmux;'
  while i <= #text do
    local s = text:find(open, i, true)
    if not s then
      table.insert(outside, text:sub(i))
      break
    end
    table.insert(outside, text:sub(i, s - 1))
    local j = s + #open
    local inner = {}
    local ok, closed = true, false
    while j <= #text do
      local c = text:sub(j, j)
      if c == U.ESC then
        local d = text:sub(j + 1, j + 1)
        if d == U.ESC then
          table.insert(inner, U.ESC)
          j = j + 2
        elseif d == '\\' then
          closed = true
          j = j + 2
          break
        else
          -- An ESC that is not doubled inside the passthrough.
          ok = false
          table.insert(inner, c)
          j = j + 1
        end
      else
        table.insert(inner, c)
        j = j + 1
      end
    end
    table.insert(wraps, { inner = table.concat(inner), ok = ok and closed })
    i = j
  end
  return wraps, table.concat(outside)
end

-- All APC graphics sequences in a byte stream (not unwrapped from tmux).
-- Each is { raw, control = {k=v}, order = {keys}, payload }.
function U.parse_apc(text)
  local out = {}
  local i = 1
  while true do
    local s = text:find(U.ESC .. '_G', i, true)
    if not s then
      break
    end
    local e = text:find(U.ST, s + 3, true)
    if not e then
      table.insert(
        out,
        { raw = text:sub(s), unterminated = true, control = {}, order = {}, payload = '' }
      )
      break
    end
    local body = text:sub(s + 3, e - 1)
    local semi = body:find(';', 1, true)
    local ctrl = semi and body:sub(1, semi - 1) or body
    local payload = semi and body:sub(semi + 1) or ''
    local control, order = parse_control(ctrl)
    table.insert(
      out,
      { raw = text:sub(s, e + 1), control = control, order = order, payload = payload }
    )
    i = e + 2
  end
  return out
end

-- The graphics commands in a capture, unwrapping tmux passthroughs.
function U.commands(cap)
  local text = cap:text()
  local wraps, outside = U.split_tmux(text)
  local cmds = U.parse_apc(outside)
  for _, w in ipairs(wraps) do
    vim.list_extend(cmds, U.parse_apc(w.inner))
  end
  return cmds
end

-- Commands in order of appearance (no tmux).
function U.commands_where(cap, pred)
  local out = {}
  for _, c in ipairs(U.commands(cap)) do
    if pred(c) then
      table.insert(out, c)
    end
  end
  return out
end

local function action(c)
  return c.control.a or 't'
end
U.action = action

-- Transmissions grouped into images: a first chunk with a=t or a=T, then
-- the continuation chunks up to the one with m=0 (or no m).
function U.transmissions(cap)
  local out, cur = {}, nil
  for _, c in ipairs(U.commands(cap)) do
    local a = c.control.a
    if cur then
      table.insert(cur.chunks, c)
      if c.control.m ~= '1' then
        cur = nil
      end
    elseif (a == 't' or a == 'T' or (a == nil and c.control.f)) and c.control.a ~= 'q' then
      cur = { first = c, chunks = { c }, id = tonumber(c.control.i) }
      table.insert(out, cur)
      if c.control.m ~= '1' then
        cur = nil
      end
    end
  end
  for _, t in ipairs(out) do
    local parts = {}
    for _, c in ipairs(t.chunks) do
      table.insert(parts, c.payload)
    end
    t.base64 = table.concat(parts)
    local ok, data = pcall(vim.base64.decode, t.base64)
    t.data = ok and data or nil
  end
  return out
end

-- Placements with U=1 (a=p, or a=T with U=1).
function U.placements(cap)
  return U.commands_where(cap, function(c)
    return (c.control.a == 'p' or c.control.a == 'T') and c.control.U == '1'
  end)
end

function U.deletes(cap)
  return U.commands_where(cap, function(c)
    return c.control.a == 'd'
  end)
end

function U.queries(cap)
  return U.commands_where(cap, function(c)
    return c.control.a == 'q'
  end)
end

-- Width and height of PNG bytes.
function U.png_dims(s)
  if not s or #s < 24 or s:sub(1, 8) ~= '\137PNG\r\n\26\n' then
    return nil
  end
  local function u32(i)
    local a, b, c, d = s:byte(i, i + 3)
    return ((a * 256 + b) * 256 + c) * 256 + d
  end
  return u32(17), u32(21)
end

-- Fires a TermResponse event with a sequence, now.
function U.fire(seq)
  vim.api.nvim_exec_autocmds('TermResponse', { data = { sequence = seq }, modeline = false })
end

-- The id the plugin used in its graphics query (31 when it cannot be read).
function U.query_id(cap)
  local q = U.queries(cap)[1]
  return q and q.control.i or '31'
end

-- Answers the pending probe as kitty does (OK for the query id, then DA1).
function U.answer_yes(cap)
  U.fire(U.ESC .. '_Gi=' .. U.query_id(cap) .. ';OK' .. U.ST)
  U.fire(U.ESC .. '[?62;c')
end

-- Answers as a terminal without graphics: only the DA1 reply.
function U.answer_no()
  U.fire(U.ESC .. '[?62;c')
end

function U.preview()
  return require('kitty-markdown.preview')
end

function U.mode()
  local p = U.preview()
  return type(p.mode) == 'function' and p.mode() or nil
end

function U.wait_mode(want, ms)
  return QA.wait(ms or 2000, function()
    return U.mode() == want
  end)
end

-- Clears :messages and returns a reader for new lines.
function U.messages_reader()
  vim.cmd('messages clear')
  return function()
    local out = vim.api.nvim_exec2('messages', { output = true }).output
    return out == '' and {} or vim.split(out, '\n', { plain = true })
  end
end

-- Lines in a list that contain a plain substring.
function U.count_with(lines, needle)
  local n = 0
  for _, l in ipairs(lines) do
    if l:find(needle, 1, true) then
      n = n + 1
    end
  end
  return n
end

U.IMAGES_MSG = 'kitty-markdown: kitty graphics on, images render inline'
U.TEXT_MSG = 'kitty-markdown: text-only mode: '

-- Lines in a list that equal a string exactly (trailing spaces ignored).
function U.count_line(lines, want)
  local n = 0
  for _, l in ipairs(lines) do
    if vim.trim(l) == want then
      n = n + 1
    end
  end
  return n
end

-- Records vim.notify calls and still passes them on, so :messages keeps
-- them. Returns the log and a restore function.
function U.notify_log()
  local log = {}
  local orig = vim.notify
  vim.notify = function(msg, level, o)
    table.insert(log, { msg = msg, level = level })
    return orig(msg, level, o)
  end
  return log, function()
    vim.notify = orig
  end
end

-- Common setup: a fresh plugin, true colors, a fake cell size, a terminal
-- capture, no tmux, the real helper, and a scratch cache. Returns the
-- capture and the temp dir.
function U.env(opts)
  opts = opts or {}
  QA.reset_plugin()
  if vim.fn.exists(':KittyMarkdown') ~= 2 then
    vim.cmd('runtime plugin/kitty-markdown.lua')
  end
  vim.env.TMUX = opts.tmux
  vim.o.termguicolors = opts.termguicolors ~= false
  local cap = QA.capture_terminal()
  QA.fake_cell(opts.cell_w or 10, opts.cell_h or 20)
  local dir = QA.tmpdir()
  local cfg =
    { placement = 'split', cache = { dir = dir .. '/cache' }, remote_images = { enabled = false } }
  if opts.helper ~= false then
    local bin = QA.helper_bin()
    cfg.helper = { path = bin, download = false }
    QA.use_helper(bin)
  end
  require('kitty-markdown').setup(cfg)
  return cap, dir
end

-- Writes a markdown file with the lines into dir, opens it in the current
-- window, and opens a preview. Returns the source buffer and the preview
-- state (or nil).
function U.open_doc(dir, lines, name)
  local path = dir .. '/' .. (name or 'doc.md')
  vim.fn.writefile(lines, path)
  vim.cmd('silent! only')
  vim.cmd('edit ' .. vim.fn.fnameescape(path))
  local buf = vim.api.nvim_get_current_buf()
  vim.bo[buf].filetype = 'markdown'
  local p = U.preview().open(buf, 'split')
  return buf, p
end

-- The preview buffer of a source buffer.
function U.preview_buf(src)
  local p = U.preview().get(src)
  return p and p.buf
end

-- Every virtual-line extmark of a buffer, in line order. Each entry is
-- { line (0-based), rows = { text }, chunks = { { {text, hl}, ... } } }.
function U.virt(buf)
  local out = {}
  if not buf or not vim.api.nvim_buf_is_valid(buf) then
    return out
  end
  local marks = vim.api.nvim_buf_get_extmarks(buf, -1, 0, -1, { details = true })
  for _, m in ipairs(marks) do
    local d = m[4]
    if d.virt_lines then
      local rows, chunks = {}, {}
      for _, row in ipairs(d.virt_lines) do
        local parts = {}
        for _, ch in ipairs(row) do
          table.insert(parts, ch[1])
        end
        table.insert(rows, table.concat(parts))
        table.insert(chunks, row)
      end
      table.insert(out, { line = m[2], rows = rows, chunks = chunks, above = d.virt_lines_above })
    end
  end
  table.sort(out, function(x, y)
    return x.line < y.line
  end)
  return out
end

local PH = vim.fn.nr2char(0x10EEEE, 1)
U.PH = PH

-- Virtual-line groups that hold placeholder cells.
function U.placeholder_marks(buf)
  local out = {}
  for _, v in ipairs(U.virt(buf)) do
    for _, r in ipairs(v.rows) do
      if r:find(PH, 1, true) then
        table.insert(out, v)
        break
      end
    end
  end
  return out
end

-- Waits until the preview of src shows n placeholder blocks.
function U.wait_placed(src, n, ms)
  n = n or 1
  local ok = QA.wait(ms or 8000, function()
    return #U.placeholder_marks(U.preview_buf(src)) >= n
  end)
  return ok, U.placeholder_marks(U.preview_buf(src))
end

-- Decodes the placeholder cells in one row string (any text around them
-- is ignored). Returns a list of { row, col, msb, ndia } per cell.
function U.decode_row(s)
  local cps = U.codepoints(s)
  local cells = {}
  local i = 1
  while i <= #cps do
    if cps[i] == U.PLACEHOLDER then
      local cell = { dia = {} }
      local j = i + 1
      while j <= #cps and U.diacritic_index(cps[j]) ~= nil do
        table.insert(cell.dia, U.diacritic_index(cps[j]))
        j = j + 1
      end
      cell.row, cell.col, cell.msb = cell.dia[1], cell.dia[2], cell.dia[3]
      cell.ndia = #cell.dia
      table.insert(cells, cell)
      i = j
    else
      i = i + 1
    end
  end
  return cells
end

-- The effective foreground color of a virt_lines chunk highlight.
function U.chunk_fg(hl)
  local groups = type(hl) == 'table' and hl or { hl }
  local fg
  for _, g in ipairs(groups) do
    local name = type(g) == 'number' and vim.fn.synIDattr(g, 'name') or g
    if name and name ~= '' then
      local def = vim.api.nvim_get_hl(0, { name = name, link = false })
      if def.fg then
        fg = def.fg
      end
    end
  end
  return fg
end

-- For one placeholder mark: every placeholder cell with its effective fg.
function U.cells_with_fg(mark)
  local out = {}
  for r, row in ipairs(mark.chunks) do
    for _, ch in ipairs(row) do
      if ch[1]:find(PH, 1, true) then
        local fg = U.chunk_fg(ch[2])
        for _, c in ipairs(U.decode_row(ch[1])) do
          c.fg = fg
          c.vrow = r
          table.insert(out, c)
        end
      end
    end
  end
  return out
end

-- A box made of border characters: the first and the last row start and
-- end with a corner, the rows between start and end with a vertical bar.
local CORNERS = {
  '╭',
  '╮',
  '╰',
  '╯',
  '┌',
  '┐',
  '└',
  '┘',
  '+',
  '┏',
  '┓',
  '┗',
  '┛',
  '╔',
  '╗',
  '╚',
  '╝',
}
local BARS = { '│', '┃', '║', '|' }

local function first_char(s)
  s = vim.trim(s)
  return vim.fn.strcharpart(s, 0, 1)
end

local function last_char(s)
  s = vim.trim(s)
  return vim.fn.strcharpart(s, vim.fn.strchars(s) - 1, 1)
end

function U.is_box(rows)
  if #rows < 3 then
    return false, 'fewer than 3 rows'
  end
  local function corner(c)
    return vim.tbl_contains(CORNERS, c)
  end
  local function bar(c)
    return vim.tbl_contains(BARS, c)
  end
  if not (corner(first_char(rows[1])) and corner(last_char(rows[1]))) then
    return false, 'top row has no corners'
  end
  if not (corner(first_char(rows[#rows])) and corner(last_char(rows[#rows]))) then
    return false, 'bottom row has no corners'
  end
  for i = 2, #rows - 1 do
    if not (bar(first_char(rows[i])) and bar(last_char(rows[i]))) then
      return false, 'side row ' .. i .. ' has no bars'
    end
  end
  return true
end

-- The virtual-line group whose rows contain a plain substring.
function U.mark_with(buf, needle)
  for _, v in ipairs(U.virt(buf)) do
    for _, r in ipairs(v.rows) do
      if r:find(needle, 1, true) then
        return v
      end
    end
  end
  return nil
end

-- Lets scheduled callbacks run.
function U.settle(ms)
  vim.wait(ms or 50, function()
    return false
  end, 5)
end

return U
