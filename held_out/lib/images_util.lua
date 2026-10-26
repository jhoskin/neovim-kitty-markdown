-- Helpers for the hidden images and cache checks: a local HTTP server, a
-- preview driver with a faked kitty terminal, and readers for the boxes
-- and placeholder rows that the preview hangs below its anchor lines.

local QA = require('qa')

local M = {}

local uv = vim.uv

M.PLACEHOLDER = vim.fn.nr2char(0x10EEEE)

-- Every failure box reason that the spec lists. A box that shows one of
-- them has settled.
M.REASONS = {
  'timeout',
  'too large',
  'fetch failed',
  'unsupported format',
  'cannot read file',
  'remote images off',
  'unsupported diagram type',
  'unsupported:',
  'Parse error on line',
  'helper exited',
  'helper error',
  'too tall',
}

-- HTTP server -----------------------------------------------------------------

-- routes: path -> { body = s, status = n, no_length = bool, stall = bool,
-- ctype = s }. A path that is not in routes gets a 404.
function M.server(routes)
  local srv = uv.new_tcp()
  assert(srv:bind('127.0.0.1', 0))
  local s = { hits = {}, clients = {}, routes = routes or {} }
  s.port = srv:getsockname().port
  function s.url(path)
    return ('http://127.0.0.1:%d%s'):format(s.port, path)
  end
  function s.count(path)
    return s.hits[path] or 0
  end
  local function close(c)
    if c and not c:is_closing() then
      c:close()
    end
  end
  assert(srv:listen(64, function(err)
    if err then
      return
    end
    local c = uv.new_tcp()
    srv:accept(c)
    table.insert(s.clients, c)
    local buf = ''
    c:read_start(function(rerr, chunk)
      if rerr or not chunk then
        close(c)
        return
      end
      buf = buf .. chunk
      if not buf:find('\r\n\r\n', 1, true) then
        return
      end
      c:read_stop()
      local path = buf:match('^%u+ (%S+)') or '/'
      s.hits[path] = (s.hits[path] or 0) + 1
      local r = s.routes[path] or { status = 404, body = 'not found' }
      if r.stall then
        return
      end
      local status = r.status or 200
      local body = r.body or ''
      local head = {
        ('HTTP/1.1 %d %s'):format(status, status == 200 and 'OK' or 'Error'),
        'Content-Type: ' .. (r.ctype or 'application/octet-stream'),
        'Connection: close',
      }
      if not r.no_length then
        table.insert(head, 'Content-Length: ' .. #body)
      end
      c:write(table.concat(head, '\r\n') .. '\r\n\r\n' .. body, function()
        c:shutdown(function()
          close(c)
        end)
      end)
    end)
  end))
  function s.stop()
    for _, c in ipairs(s.clients) do
      close(c)
    end
    s.clients = {}
    close(srv)
  end
  return s
end

-- Preview driver --------------------------------------------------------------

local function preview_buf()
  for _, b in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_valid(b) and vim.bo[b].filetype == 'kitty-markdown' then
      return b
    end
  end
  return nil
end
M.preview_buf = preview_buf

-- The virtual-line groups of a preview buffer, in row order. Each entry:
-- { row = 0-based anchor row, lines = { text, ... }, text = joined }.
function M.boxes(pbuf)
  local out = {}
  if not pbuf or not vim.api.nvim_buf_is_valid(pbuf) then
    return out
  end
  local marks = vim.api.nvim_buf_get_extmarks(pbuf, -1, 0, -1, { details = true })
  for _, m in ipairs(marks) do
    local d = m[4]
    if d and d.virt_lines then
      local lines = {}
      for _, vl in ipairs(d.virt_lines) do
        local parts = {}
        for _, chunk in ipairs(vl) do
          table.insert(parts, chunk[1])
        end
        table.insert(lines, table.concat(parts))
      end
      table.insert(out, { row = m[2], lines = lines, text = table.concat(lines, '\n') })
    end
  end
  table.sort(out, function(x, y)
    return x.row < y.row
  end)
  return out
end

function M.is_placed(box)
  return box.text:find(M.PLACEHOLDER, 1, true) ~= nil
end

function M.reason_in(box)
  for _, r in ipairs(M.REASONS) do
    if box.text:find(r, 1, true) then
      return r
    end
  end
  return nil
end

local function settled_box(box)
  return M.is_placed(box) or M.reason_in(box) ~= nil
end

-- Waits until `n` boxes exist and each is placed or shows a reason.
function M.settle(pbuf, n, ms)
  QA.wait(ms or 5000, function()
    local bs = M.boxes(pbuf or preview_buf())
    if #bs < (n or 1) then
      return false
    end
    for _, b in ipairs(bs) do
      if not settled_box(b) then
        return false
      end
    end
    return true
  end)
  return M.boxes(pbuf or preview_buf())
end

-- Placeholder rows of a placed box: rows and the cells on the widest row.
function M.grid(box)
  local rows, cols = 0, 0
  for _, l in ipairs(box.lines) do
    local _, n = l:gsub(M.PLACEHOLDER, '')
    if n > 0 then
      rows = rows + 1
      cols = math.max(cols, n)
    end
  end
  return rows, cols
end

-- The `c=` and `r=` of every placement command in the captured bytes.
function M.placements(cap)
  local out = {}
  for ctl in cap:text():gmatch('\27_G([^;\27]*)') do
    if ctl:find('a=p', 1, true) then
      table.insert(
        out,
        { cols = tonumber(ctl:match('c=(%d+)')), rows = tonumber(ctl:match('r=(%d+)')), ctl = ctl }
      )
    end
  end
  return out
end

function M.messages()
  return vim.fn.execute('messages')
end

-- New :messages text since `before` (a string from M.messages()).
function M.new_messages(before)
  local now = M.messages()
  if now:sub(1, #before) == before then
    return now:sub(#before + 1)
  end
  return now
end

-- Opens a preview of `lines` (written to `<dir>/<name>`) with a faked
-- kitty terminal that answers the graphics probe.
-- o: dir, name, cache_dir (false = leave the option out), remote (table
-- merged into remote_images), config (extra setup options), cell {w, h},
-- helper (path, or false for no helper), n (boxes to wait for), probe
-- (default true), wait (false = do not settle).
function M.open(lines, o)
  o = o or {}
  vim.o.termguicolors = true
  local dir = o.dir or QA.tmpdir()
  local cache = o.cache_dir
  if cache == nil then
    cache = QA.tmpdir()
  end
  local cfg = vim.deepcopy(o.config or {})
  if cache then
    cfg.cache = vim.tbl_extend('force', cfg.cache or {}, { dir = cache })
  end
  if o.remote then
    cfg.remote_images = vim.tbl_extend('force', cfg.remote_images or {}, o.remote)
  end
  local ok, err = pcall(require('kitty-markdown').setup, cfg)
  QA.need(ok, 'setup() accepts the cache and remote image options: ' .. tostring(err))
  local cap = QA.capture_terminal()
  local cell = o.cell or { 10, 20 }
  QA.fake_cell(cell[1], cell[2])
  if o.helper == false then
    local helper = require('kitty-markdown.helper')
    QA.need(type(helper.set_locator) == 'function', 'a helper locator hook')
    helper.set_locator(function(cb)
      cb(nil)
    end)
  else
    QA.use_helper(o.helper)
  end
  local name = dir .. '/' .. (o.name or 'doc.md')
  local buf = QA.buf(lines, { name = name, write = true })
  vim.api.nvim_set_current_buf(buf)
  require('kitty-markdown').open('tab')
  if o.probe ~= false then
    QA.answer_probe(true)
  end
  local pbuf
  QA.wait(2000, function()
    pbuf = preview_buf()
    return pbuf ~= nil
  end)
  QA.need(pbuf, 'a preview buffer with filetype kitty-markdown')
  local res = { src = buf, pbuf = pbuf, cap = cap, cache = cache, dir = dir, name = name }
  for _, w in ipairs(vim.api.nvim_list_wins()) do
    if vim.api.nvim_win_get_buf(w) == pbuf then
      res.win = w
    end
  end
  if o.wait ~= false then
    res.boxes = M.settle(pbuf, o.n or 1, o.ms)
  end
  return res
end

-- Text width of the preview window: width minus 4, capped at 100.
function M.text_width(win)
  return math.min(vim.api.nvim_win_get_width(win) - 4, 100)
end

-- Files ------------------------------------------------------------------------

-- Every regular file under dir, as paths relative to dir.
function M.files(dir)
  local out = {}
  for name, kind in vim.fs.dir(dir, { depth = 10 }) do
    if kind == 'file' then
      table.insert(out, name)
    end
  end
  table.sort(out)
  return out
end

function M.sha_file(path)
  return vim.fn.sha256(assert(QA.read(path)))
end

function M.set_mtime(path, t)
  assert(uv.fs_utime(path, t, t))
end

function M.mtime(path)
  local st = uv.fs_stat(path)
  return st and st.mtime.sec
end

function M.exists(path)
  return uv.fs_stat(path) ~= nil
end

-- Base name without extension.
function M.stem(path)
  return (vim.fs.basename(path):gsub('%..*$', ''))
end

-- PNG decoding through the system zlib (LuaJIT FFI). Supports 8-bit
-- gray, RGB, palette, gray-alpha, and RGBA without interlace.
local zlib
local function load_zlib()
  if zlib ~= nil then
    return zlib
  end
  local ffi = require('ffi')
  pcall(
    ffi.cdef,
    'int uncompress(unsigned char *dest, unsigned long *destLen, const unsigned char *source, unsigned long sourceLen);'
  )
  for _, name in ipairs({ 'z', 'libz.so.1', '/usr/lib/libz.dylib' }) do
    local ok, lib = pcall(ffi.load, name)
    if ok then
      zlib = lib
      return zlib
    end
  end
  zlib = false
  return zlib
end

function M.decode_png(path)
  local s = QA.read(path)
  if not s or s:sub(1, 8) ~= '\137PNG\r\n\26\n' then
    return nil, 'not a PNG'
  end
  local lib = load_zlib()
  QA.need(lib, 'the system zlib through LuaJIT FFI to decode PNG pixels')
  local ffi = require('ffi')
  local function u32(i)
    local a1, b, c, d = s:byte(i, i + 3)
    return ((a1 * 256 + b) * 256 + c) * 256 + d
  end
  local pos, idat, plte, trns = 9, {}, nil, nil
  local w, h, depth, ctype, interlace
  while pos + 8 <= #s do
    local len = u32(pos)
    local typ = s:sub(pos + 4, pos + 7)
    local data = s:sub(pos + 8, pos + 7 + len)
    if typ == 'IHDR' then
      w, h = u32(pos + 8), u32(pos + 12)
      depth, ctype, interlace = data:byte(9), data:byte(10), data:byte(13)
    elseif typ == 'PLTE' then
      plte = data
    elseif typ == 'tRNS' then
      trns = data
    elseif typ == 'IDAT' then
      table.insert(idat, data)
    elseif typ == 'IEND' then
      break
    end
    pos = pos + 12 + len
  end
  if depth ~= 8 or interlace ~= 0 then
    return nil, 'unsupported PNG layout'
  end
  local chans = ({ [0] = 1, [2] = 3, [3] = 1, [4] = 2, [6] = 4 })[ctype]
  local stride = w * chans
  local size = h * (stride + 1)
  local src = table.concat(idat)
  local out = ffi.new('unsigned char[?]', size)
  local outlen = ffi.new('unsigned long[1]', size)
  if lib.uncompress(out, outlen, src, #src) ~= 0 then
    return nil, 'inflate failed'
  end
  local px = {}
  local prev = {}
  for y = 0, h - 1 do
    local base = y * (stride + 1)
    local f = out[base]
    local row = {}
    for i = 0, stride - 1 do
      local x = out[base + 1 + i]
      local left = i >= chans and row[i - chans] or 0
      local up = prev[i] or 0
      local ul = i >= chans and (prev[i - chans] or 0) or 0
      if f == 1 then
        x = x + left
      elseif f == 2 then
        x = x + up
      elseif f == 3 then
        x = x + math.floor((left + up) / 2)
      elseif f == 4 then
        local p = left + up - ul
        local pa, pb, pc = math.abs(p - left), math.abs(p - up), math.abs(p - ul)
        if pa <= pb and pa <= pc then
          x = x + left
        elseif pb <= pc then
          x = x + up
        else
          x = x + ul
        end
      end
      row[i] = x % 256
    end
    px[y] = row
    prev = row
  end
  local img = { width = w, height = h }
  function img.at(x, y)
    local row = px[y]
    local i = x * chans
    if ctype == 6 then
      return { row[i], row[i + 1], row[i + 2], row[i + 3] }
    elseif ctype == 2 then
      return { row[i], row[i + 1], row[i + 2], 255 }
    elseif ctype == 0 then
      return { row[i], row[i], row[i], 255 }
    elseif ctype == 4 then
      return { row[i], row[i], row[i], row[i + 1] }
    end
    local k = row[i]
    local alpha = trns and trns:byte(k + 1) or 255
    return { plte:byte(k * 3 + 1), plte:byte(k * 3 + 2), plte:byte(k * 3 + 3), alpha }
  end
  return img
end

-- RGBA of pixel (x, y) of a PNG file.
function M.pixel(path, x, y)
  local img, err = M.decode_png(path)
  QA.need(img, 'a decodable PNG output (' .. tostring(err) .. ')')
  return img.at(x, y)
end

-- A shell wrapper around the helper that logs each request line.
function M.logging_helper(log)
  local dir = QA.tmpdir()
  local path = dir .. '/helper-wrap.sh'
  QA.write(path, ('#!/bin/sh\ntee -a %q | %q "$@"\n'):format(log, QA.helper_bin()))
  vim.fn.setfperm(path, 'rwx------')
  return path
end

function M.count_lines(path, needle)
  local s = QA.read(path) or ''
  local n = 0
  for line in s:gmatch('[^\n]+') do
    if line:find(needle, 1, true) then
      n = n + 1
    end
  end
  return n
end

M.fixtures = QA.evals .. '/images'

return M
