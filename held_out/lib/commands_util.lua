-- Helpers for the hidden checks of the commands area (commands, placement,
-- refresh, interaction). Everything here goes through the public surface
-- (the :KittyMarkdown command, the Lua module, the preview buffer and its
-- window) plus the QA terminal hooks.

local QA = require('qa')

local U = {}

local api = vim.api
local PLACEHOLDER = vim.fn.nr2char(0x10EEEE, 1)
U.PLACEHOLDER = PLACEHOLDER

-- Loads the plugin file the way a plugin manager does.
function U.load()
  if vim.fn.exists(':KittyMarkdown') ~= 2 then
    vim.cmd('runtime plugin/kitty-markdown.lua')
  end
  return require('kitty-markdown')
end

local servers = {}
local ui_open_orig = nil

-- Full reset between checks: previews, helper, windows, tabs, stray
-- buffers, the terminal hooks, and the system opener stub.
function U.reset(setup_opts)
  for _, s in ipairs(servers) do
    pcall(s.stop)
  end
  servers = {}
  if ui_open_orig then
    vim.ui.open = ui_open_orig
    ui_open_orig = nil
  end
  QA.reset_plugin()
  pcall(vim.cmd, 'silent! only! | silent! tabonly!')
  for _, b in ipairs(api.nvim_list_bufs()) do
    if b ~= api.nvim_get_current_buf() then
      pcall(api.nvim_buf_delete, b, { force = true })
    end
  end
  local scratch = api.nvim_create_buf(true, true)
  local old = api.nvim_get_current_buf()
  api.nvim_set_current_buf(scratch)
  pcall(api.nvim_buf_delete, old, { force = true })
  vim.fn.chdir(QA.root)
  vim.env.TMUX = nil
  vim.o.termguicolors = true
  vim.o.cmdheight = 1
  local km = U.load()
  local opts = vim.tbl_deep_extend('force', {
    cache = { dir = QA.tmpdir() .. '/cache' },
  }, setup_opts or {})
  local bin = vim.env.QA_HELPER_BIN or (QA.out .. '/target/release/kitty-markdown-helper')
  if vim.fn.executable(bin) == 1 then
    opts.helper = vim.tbl_deep_extend('force', { path = bin }, opts.helper or {})
    QA.use_helper(bin)
  end
  km.setup(opts)
  return km
end

-- A markdown file on disk, loaded in the current window. Returns the
-- buffer, the file path, and the directory.
function U.src(text, opts)
  opts = opts or {}
  local dir = opts.dir or QA.tmpdir()
  dir = vim.uv.fs_realpath(dir) or dir
  local path = dir .. '/' .. (opts.name or 'doc.md')
  local lines = type(text) == 'table' and text or vim.split(text, '\n', { plain = true })
  QA.write(path, table.concat(lines, '\n') .. '\n')
  vim.cmd('silent edit ' .. vim.fn.fnameescape(path))
  local buf = api.nvim_get_current_buf()
  vim.bo[buf].filetype = opts.ft or 'markdown'
  return buf, path, dir
end

-- Every window (all tabpages) that shows a buffer with the preview
-- filetype, as { win, buf, tab } entries.
function U.previews()
  local out = {}
  for _, tab in ipairs(api.nvim_list_tabpages()) do
    for _, win in ipairs(api.nvim_tabpage_list_wins(tab)) do
      local b = api.nvim_win_get_buf(win)
      if vim.bo[b].filetype == 'kitty-markdown' then
        table.insert(out, { win = win, buf = b, tab = tab })
      end
    end
  end
  return out
end

-- Every loaded buffer with the preview filetype.
function U.preview_bufs()
  local out = {}
  for _, b in ipairs(api.nvim_list_bufs()) do
    if api.nvim_buf_is_valid(b) and vim.bo[b].filetype == 'kitty-markdown' then
      table.insert(out, b)
    end
  end
  return out
end

-- The single preview window entry, or nil.
function U.preview()
  local p = U.previews()
  return p[1], #p
end

function U.lines(buf)
  return api.nvim_buf_get_lines(buf, 0, -1, false)
end

function U.text(buf)
  return table.concat(U.lines(buf), '\n')
end

-- The first buffer line (1-based) that contains `needle`.
function U.find_line(buf, needle)
  for i, l in ipairs(U.lines(buf)) do
    if l:find(needle, 1, true) then
      return i, l
    end
  end
  return nil
end

-- Virtual line blocks in every namespace: { row = 1-based anchor line,
-- rows = { string per virtual line }, above = bool }.
function U.virt(buf)
  local out = {}
  for _, m in ipairs(api.nvim_buf_get_extmarks(buf, -1, 0, -1, { details = true })) do
    local d = m[4]
    if d.virt_lines then
      local rows = {}
      for _, row in ipairs(d.virt_lines) do
        local parts = {}
        for _, chunk in ipairs(row) do
          parts[#parts + 1] = chunk[1]
        end
        rows[#rows + 1] = table.concat(parts)
      end
      table.insert(out, { row = m[2] + 1, rows = rows, above = d.virt_lines_above })
    end
  end
  table.sort(out, function(x, y)
    return x.row < y.row
  end)
  return out
end

function U.virt_text(buf)
  local parts = {}
  for _, v in ipairs(U.virt(buf)) do
    parts[#parts + 1] = table.concat(v.rows, '\n')
  end
  return table.concat(parts, '\n')
end

function U.has_placeholders(buf)
  return U.virt_text(buf):find(PLACEHOLDER, 1, true) ~= nil
end

function U.settle(ms)
  vim.wait(ms or 50, function()
    return false
  end, 5)
end

-- Captures notify calls and new :messages lines while `fn` runs. Returns
-- the list of message strings with their levels, and the error (if any)
-- that `fn` raised.
function U.collect(fn)
  local log, restore = QA.capture_notify()
  local before = vim.fn.execute('messages')
  local ok, err = pcall(fn)
  U.settle(30)
  restore()
  local after = vim.fn.execute('messages')
  local out = {}
  for _, e in ipairs(log) do
    table.insert(out, { msg = tostring(e.msg), level = e.level })
  end
  if #after > #before and after:sub(1, #before) == before then
    for _, l in ipairs(vim.split(after:sub(#before + 1), '\n', { trimempty = true })) do
      table.insert(out, { msg = l, level = nil, from_messages = true })
    end
  end
  if not ok then
    table.insert(out, { msg = tostring(err), level = vim.log.levels.ERROR, raised = true })
  end
  return out, ok, err
end

function U.msgs(list)
  return vim.tbl_map(function(e)
    return e.msg
  end, list)
end

-- Counts calls of the public layout function. Returns a getter.
function U.count_layout()
  local km = require('kitty-markdown')
  local orig = km.layout
  local n = 0
  km.layout = function(...)
    n = n + 1
    return orig(...)
  end
  return function()
    return n
  end
end

-- Stubs the system opener. Returns the list of targets it received.
function U.stub_open()
  local got = {}
  if not ui_open_orig then
    ui_open_orig = vim.ui.open
  end
  vim.ui.open = function(target)
    table.insert(got, target)
    return {
      wait = function()
        return { code = 0 }
      end,
    }, nil
  end
  return got
end

-- The kitty graphics commands in a terminal capture: a list of key/value
-- tables, one per APC sequence, in order.
function U.graphics(cap)
  local out = {}
  local s = cap:text()
  for control in s:gmatch('\27_G([^;\27]*)') do
    local t = { raw = control }
    for k, v in control:gmatch('(%a)=([^,]*)') do
      t[k] = v
    end
    table.insert(out, t)
  end
  return out
end

-- Image ids transmitted (a=t or a=T, first chunk) in a capture.
function U.transmits(cap)
  local ids = {}
  for _, g in ipairs(U.graphics(cap)) do
    if (g.a == 't' or g.a == 'T') and g.i then
      table.insert(ids, tonumber(g.i))
    end
  end
  return ids
end

-- Image ids deleted (a=d) in a capture.
function U.deletes(cap)
  local ids = {}
  for _, g in ipairs(U.graphics(cap)) do
    if g.a == 'd' then
      table.insert(ids, g.i and tonumber(g.i) or g.raw)
    end
  end
  return ids
end

function U.queries(cap)
  local n = 0
  for _, g in ipairs(U.graphics(cap)) do
    if g.a == 'q' then
      n = n + 1
    end
  end
  return n
end

-- Puts the plugin in image mode: fakes a cell size, captures the terminal,
-- opens a throwaway preview so the plugin sends its probe, answers it as
-- kitty would, and waits until a picture shows. Returns the capture.
function U.image_mode()
  vim.o.termguicolors = true
  QA.fake_cell(10, 20, vim.o.lines, vim.o.columns)
  local cap = QA.capture_terminal()
  local dir = QA.tmpdir()
  QA.make_png(dir .. '/warm.png', 40, 20)
  local cur = api.nvim_get_current_buf()
  local buf = U.src(
    { '# warm', '', 'text', '', '![warm](warm.png)' },
    { dir = dir, name = 'warm.md' }
  )
  vim.cmd('KittyMarkdown open split')
  QA.answer_probe(true)
  local p = U.preview()
  local ok = p
    and QA.wait(8000, function()
      return api.nvim_buf_is_valid(p.buf) and U.has_placeholders(p.buf)
    end)
  QA.assert.truthy(ok, 'image mode set up: a picture shows after a kitty probe answer')
  api.nvim_set_current_buf(buf)
  vim.cmd('KittyMarkdown close')
  pcall(vim.cmd, 'silent! only!')
  if api.nvim_buf_is_valid(cur) then
    api.nvim_set_current_buf(cur)
  end
  pcall(api.nvim_buf_delete, buf, { force = true })
  cap:clear()
  return cap
end

-- A tiny HTTP server on 127.0.0.1 that answers every request with `body`
-- after `delay_ms`. Returns { url = base url, hits = n, stop = fn }.
function U.http_server(body, delay_ms, content_type)
  local uv = vim.uv
  local server = uv.new_tcp()
  server:bind('127.0.0.1', 0)
  local srv = { hits = 0, clients = {} }
  server:listen(16, function(err)
    if err then
      return
    end
    local client = uv.new_tcp()
    server:accept(client)
    table.insert(srv.clients, client)
    local req = ''
    client:read_start(function(rerr, chunk)
      if rerr or not chunk then
        pcall(client.close, client)
        return
      end
      req = req .. chunk
      if req:find('\r\n\r\n', 1, true) then
        client:read_stop()
        srv.hits = srv.hits + 1
        local function answer()
          if client:is_closing() then
            return
          end
          local head = 'HTTP/1.1 200 OK\r\nContent-Type: '
            .. (content_type or 'image/png')
            .. '\r\nContent-Length: '
            .. #body
            .. '\r\nConnection: close\r\n\r\n'
          client:write(head .. body, function()
            pcall(client.shutdown, client)
            pcall(client.close, client)
          end)
        end
        if (delay_ms or 0) > 0 then
          local t = uv.new_timer()
          t:start(delay_ms, 0, function()
            t:close()
            answer()
          end)
        else
          answer()
        end
      end
    end)
  end)
  local port = server:getsockname().port
  srv.url = ('http://127.0.0.1:%d'):format(port)
  srv.stop = function()
    for _, c in ipairs(srv.clients) do
      if not c:is_closing() then
        pcall(c.close, c)
      end
    end
    if not server:is_closing() then
      server:close()
    end
  end
  table.insert(servers, srv)
  return srv
end

-- The top line of a window.
function U.topline(win)
  return api.nvim_win_call(win, function()
    return vim.fn.line('w0')
  end)
end

-- Writes the current buffer to disk quietly.
function U.write(buf)
  api.nvim_buf_call(buf, function()
    vim.cmd('silent write')
  end)
end

return U
