-- QA harness for the hidden checks. Each spec file runs in its own nvim
-- process (see run_file.lua), so a check may change global plugin state.
--
-- A check names the requirements it covers and a public hint. The hint is
-- the only text about a check that reaches REVIEW.md, so it must describe
-- the expected behavior in general terms and never the exact input.

local bit = require('bit')

local M = {}

local function dirname(p)
  return vim.fn.fnamemodify(p, ':h')
end

M.held_out = dirname(dirname(debug.getinfo(1, 'S').source:sub(2)))
M.root = dirname(M.held_out)
M.evals = M.held_out .. '/evals'
M.out = M.held_out .. '/out'

vim.opt.runtimepath:prepend(M.root)
package.path = M.held_out .. '/lib/?.lua;' .. package.path

-- Results -------------------------------------------------------------------

M.results = {}
local stack = {}
local hooks = { before = {}, after = {} }
local queue = {}

local Fail = {}
Fail.__index = Fail

local function fail(kind, detail)
  error(setmetatable({ kind = kind, detail = detail }, Fail), 0)
end

-- Raised by M.need: the check cannot run through the public interface.
function M.need(cond, what)
  if not cond then
    fail('blocked', what)
  end
  return cond
end

-- Assertions ----------------------------------------------------------------

local A = {}

local function show(v)
  return vim.inspect(v, { newline = ' ', indent = '' })
end

local function check_assert(ok, detail, msg)
  if not ok then
    fail('assertion', (msg and (msg .. ': ') or '') .. detail)
  end
end

function A.eq(expected, actual, msg)
  check_assert(
    vim.deep_equal(expected, actual),
    'expected ' .. show(expected) .. ', got ' .. show(actual),
    msg
  )
end

function A.ne(unexpected, actual, msg)
  check_assert(not vim.deep_equal(unexpected, actual), 'did not expect ' .. show(actual), msg)
end

function A.truthy(v, msg)
  check_assert(v, 'expected a truthy value, got ' .. show(v), msg)
end

function A.falsy(v, msg)
  check_assert(not v, 'expected a falsy value, got ' .. show(v), msg)
end

function A.is_nil(v, msg)
  check_assert(v == nil, 'expected nil, got ' .. show(v), msg)
end

function A.not_nil(v, msg)
  check_assert(v ~= nil, 'expected a value, got nil', msg)
end

function A.matches(pattern, s, msg)
  check_assert(
    type(s) == 'string' and s:find(pattern) ~= nil,
    'expected ' .. show(s) .. ' to match ' .. show(pattern),
    msg
  )
end

function A.no_match(pattern, s, msg)
  check_assert(
    type(s) ~= 'string' or s:find(pattern) == nil,
    'expected ' .. show(s) .. ' not to match ' .. show(pattern),
    msg
  )
end

-- Plain substring test.
function A.contains(needle, haystack, msg)
  if type(haystack) == 'table' then
    for _, v in ipairs(haystack) do
      if vim.deep_equal(v, needle) then
        return
      end
    end
    check_assert(
      false,
      'expected the list to contain ' .. show(needle) .. ', got ' .. show(haystack),
      msg
    )
  end
  check_assert(
    type(haystack) == 'string' and haystack:find(needle, 1, true) ~= nil,
    'expected ' .. show(haystack) .. ' to contain ' .. show(needle),
    msg
  )
end

function A.not_contains(needle, haystack, msg)
  if type(haystack) == 'table' then
    for _, v in ipairs(haystack) do
      check_assert(
        not vim.deep_equal(v, needle),
        'expected the list not to contain ' .. show(needle),
        msg
      )
    end
    return
  end
  check_assert(
    type(haystack) ~= 'string' or haystack:find(needle, 1, true) == nil,
    'expected ' .. show(haystack) .. ' not to contain ' .. show(needle),
    msg
  )
end

function A.near(expected, actual, tol, msg)
  check_assert(
    type(actual) == 'number' and math.abs(expected - actual) <= tol,
    ('expected %s within %s, got %s'):format(expected, tol, show(actual)),
    msg
  )
end

function A.le(limit, actual, msg)
  check_assert(
    type(actual) == 'number' and actual <= limit,
    ('expected at most %s, got %s'):format(limit, show(actual)),
    msg
  )
end

function A.ge(limit, actual, msg)
  check_assert(
    type(actual) == 'number' and actual >= limit,
    ('expected at least %s, got %s'):format(limit, show(actual)),
    msg
  )
end

function A.errors(fn, pattern, msg)
  local ok, err = pcall(fn)
  check_assert(not ok, 'expected an error', msg)
  if pattern then
    local s = type(err) == 'string' and err or show(err)
    check_assert(
      s:find(pattern) ~= nil,
      'error ' .. show(s) .. ' does not match ' .. show(pattern),
      msg
    )
  end
end

function A.no_error(fn, msg)
  local ok, err = pcall(fn)
  check_assert(ok, 'unexpected error ' .. show(err), msg)
end

M.assert = A

-- Declaration API -------------------------------------------------------------

function M.describe(name, fn)
  table.insert(stack, name)
  local saved =
    { before = vim.deepcopy(hooks.before, true), after = vim.deepcopy(hooks.after, true) }
  local ok, err = pcall(fn)
  if not ok then
    table.insert(queue, {
      path = table.concat(stack, ' / '),
      reqs = {},
      hint = 'spec file structure',
      load_error = tostring(err),
    })
  end
  hooks.before, hooks.after = saved.before, saved.after
  table.remove(stack)
end

function M.before_each(fn)
  table.insert(hooks.before, fn)
end

function M.after_each(fn)
  table.insert(hooks.after, 1, fn)
end

-- check(reqs, hint, [opts], fn)
--   reqs: 'R-001' or { 'R-001', 'R-002' }
--   hint: public, generic description of the expected behavior
--   opts: { severity = 'high'|'medium'|'low', phase = 1..5, fix = 'public fix text', timeout_ms = n }
function M.check(reqs, hint, opts, fn)
  if type(opts) == 'function' then
    fn, opts = opts, {}
  end
  if type(reqs) == 'string' then
    reqs = { reqs }
  end
  table.insert(queue, {
    path = table.concat(stack, ' / '),
    reqs = reqs,
    hint = hint,
    opts = opts or {},
    fn = fn,
    before = vim.list_slice(hooks.before),
    after = vim.list_slice(hooks.after),
  })
end

-- A requirement that cannot be checked here at all (for example, it needs a
-- real kitty window). It appears in REVIEW.md as unchecked, with the reason.
function M.manual(reqs, reason)
  if type(reqs) == 'string' then
    reqs = { reqs }
  end
  table.insert(
    queue,
    { path = table.concat(stack, ' / '), reqs = reqs, hint = reason, manual = true }
  )
end

local function run_one(t)
  local r = { path = t.path, reqs = t.reqs, hint = t.hint, opts = t.opts or {} }
  if t.load_error then
    r.status, r.kind, r.detail = 'error', 'load', t.load_error
    return r
  end
  if t.manual then
    r.status = 'manual'
    return r
  end
  local start = vim.uv.hrtime()
  local function call(f)
    return xpcall(f, function(e)
      if getmetatable(e) == Fail then
        return e
      end
      return { kind = 'error', detail = tostring(e) .. '\n' .. debug.traceback('', 2) }
    end)
  end
  local ok, err = true, nil
  for _, h in ipairs(t.before) do
    ok, err = call(h)
    if not ok then
      break
    end
  end
  if ok then
    ok, err = call(t.fn)
  end
  for _, h in ipairs(t.after) do
    local aok, aerr = call(h)
    if ok and not aok then
      ok, err = aok, aerr
    end
  end
  r.ms = (vim.uv.hrtime() - start) / 1e6
  if ok then
    r.status = 'pass'
  elseif err.kind == 'blocked' then
    r.status, r.kind, r.detail = 'blocked', 'blocked', err.detail
  else
    r.status, r.kind, r.detail = 'fail', err.kind, err.detail
  end
  return r
end

function M._take_queue()
  local q = queue
  queue = {}
  return q
end

M._run_one = run_one

-- Helpers for checks ----------------------------------------------------------

local tmp_n = 0
function M.tmpdir()
  tmp_n = tmp_n + 1
  local dir = ('%s/tmp/%d-%d'):format(M.out, vim.uv.os_getpid(), tmp_n)
  vim.fn.delete(dir, 'rf')
  vim.fn.mkdir(dir, 'p')
  return dir
end

function M.write(path, content)
  vim.fn.mkdir(vim.fn.fnamemodify(path, ':h'), 'p')
  local f = assert(io.open(path, 'wb'))
  f:write(content)
  f:close()
  return path
end

function M.read(path)
  local f = io.open(path, 'rb')
  if not f then
    return nil
  end
  local s = f:read('*a')
  f:close()
  return s
end

local function to_lines(text)
  if type(text) == 'table' then
    return text
  end
  return vim.split(text, '\n', { plain = true })
end

-- A markdown buffer with the given text. opts.name gives it a file name (the
-- file is written to disk when opts.write is true). opts.ft overrides the
-- filetype.
function M.buf(text, opts)
  opts = opts or {}
  local buf = vim.api.nvim_create_buf(true, false)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, to_lines(text))
  if opts.name then
    if opts.write then
      M.write(opts.name, table.concat(to_lines(text), '\n') .. '\n')
    end
    vim.api.nvim_buf_set_name(buf, opts.name)
    vim.bo[buf].modified = false
  end
  vim.bo[buf].filetype = opts.ft or 'markdown'
  return buf
end

-- The layout model of a document given as text.
function M.layout(text, opts, bufopts)
  local buf = M.buf(text, bufopts)
  local model = require('kitty-markdown').layout(buf, opts or {})
  return model, buf
end

-- Lines of a layout model with trailing spaces removed.
function M.lines(model)
  return vim.tbl_map(function(l)
    return (l:gsub('%s+$', ''))
  end, model.lines)
end

-- All highlight spans that cover the first occurrence of `needle` in the
-- model's lines. Returns a list of group names (may be empty) and the line.
function M.groups_at(model, needle)
  for i, l in ipairs(model.lines) do
    local s = l:find(needle, 1, true)
    if s then
      local col = s - 1
      local groups = {}
      for _, h in ipairs(model.highlights) do
        local line = h.line or h[1]
        local cs = h.col_start or h.start_col or h[2]
        local ce = h.col_end or h.end_col or h[3]
        local g = h.group or h[4]
        if line == i and cs <= col and col < ce then
          table.insert(groups, g)
        end
      end
      return groups, i
    end
  end
  return nil
end

function M.wait(ms, pred, interval)
  return vim.wait(ms, pred, interval or 10)
end

-- Records vim.notify calls until the returned restore() is called.
function M.capture_notify()
  local log = {}
  local orig = vim.notify
  vim.notify = function(msg, level, o)
    table.insert(log, { msg = msg, level = level, opts = o })
  end
  return log, function()
    vim.notify = orig
  end
end

-- Messages in a notify log at or above a level, as strings.
function M.messages(log, min_level)
  local out = {}
  for _, e in ipairs(log) do
    if (e.level or vim.log.levels.INFO) >= (min_level or 0) then
      table.insert(out, e.msg)
    end
  end
  return out
end

function M.sh(cmd, opts)
  local res = vim
    .system(cmd, vim.tbl_extend('force', { text = true }, opts or {}))
    :wait(opts and opts.timeout or 120000)
  return res
end

-- The helper binary that run.lua built for this QA run.
function M.helper_bin()
  local p = vim.env.QA_HELPER_BIN or (M.out .. '/target/release/kitty-markdown-helper')
  M.need(vim.fn.executable(p) == 1, 'helper binary built by held_out/run.lua')
  return p
end

-- Capture of every byte the plugin writes to the terminal. Returns the
-- capture object: cap.data (string), cap:clear().
function M.capture_terminal()
  local transport = require('kitty-markdown.transport')
  M.need(type(transport.set_writer) == 'function', 'a terminal writer hook in the transport module')
  local cap = { chunks = {} }
  function cap:text()
    return table.concat(self.chunks)
  end
  function cap:clear()
    self.chunks = {}
  end
  transport.set_writer(function(s)
    table.insert(cap.chunks, s)
  end)
  return cap
end

-- Fakes the terminal cell size in pixels.
function M.fake_cell(width_px, height_px, rows, cols)
  local transport = require('kitty-markdown.transport')
  M.need(
    type(transport.set_winsize_reader) == 'function',
    'a window size hook in the transport module'
  )
  rows, cols = rows or vim.o.lines, cols or vim.o.columns
  transport.set_winsize_reader(function()
    return rows, cols, cols * width_px, rows * height_px
  end)
end

-- Answers the next graphics probe as kitty would (yes = true) or as a
-- terminal without graphics would (yes = false).
function M.answer_probe(yes)
  vim.schedule(function()
    local seq = yes and '\27_Gi=31;OK\27\\' or '\27[?62;c'
    vim.api.nvim_exec_autocmds('TermResponse', { data = { sequence = seq }, modeline = false })
    if yes then
      vim.api.nvim_exec_autocmds(
        'TermResponse',
        { data = { sequence = '\27[?62;c' }, modeline = false }
      )
    end
  end)
end

-- Points the plugin at a helper binary (the real one by default).
function M.use_helper(path)
  local helper = require('kitty-markdown.helper')
  path = path or M.helper_bin()
  if type(helper.set_locator) == 'function' then
    helper.set_locator(function(cb)
      cb(path, 'user')
    end)
  end
  return path
end

-- A JSON-lines session with a helper process. Returns an object with
-- :request(method, params, timeout_ms) -> response table, and :close().
function M.helper_session(args, bin)
  bin = bin or M.helper_bin()
  local cache = M.tmpdir()
  args = args or { 'serve', '--cache-dir', cache }
  local buf, responses, next_id = '', {}, 0
  local sess = { cache_dir = cache, stderr = {} }
  local proc = vim.system(vim.list_extend({ bin }, args), {
    stdin = true,
    stdout = function(_, data)
      if not data then
        return
      end
      buf = buf .. data
      while true do
        local nl = buf:find('\n', 1, true)
        if not nl then
          break
        end
        local line = buf:sub(1, nl - 1)
        buf = buf:sub(nl + 1)
        local ok, msg = pcall(vim.json.decode, line, { luanil = { object = true, array = true } })
        if ok and type(msg) == 'table' then
          responses[msg.id or 'noid'] = msg
          table.insert(sess, msg)
        else
          table.insert(sess, { raw = line })
        end
      end
    end,
    stderr = function(_, data)
      if data then
        table.insert(sess.stderr, data)
      end
    end,
  })
  sess.proc = proc
  function sess:send_raw(line)
    proc:write(line .. '\n')
  end
  function sess:send(method, params)
    next_id = next_id + 1
    local msg = { id = next_id, method = method }
    if params ~= nil then
      msg.params = params
    end
    proc:write(vim.json.encode(msg) .. '\n')
    return next_id
  end
  function sess:wait_for(id, timeout_ms)
    vim.wait(timeout_ms or 30000, function()
      return responses[id] ~= nil
    end, 5)
    return responses[id]
  end
  function sess:request(method, params, timeout_ms)
    return self:wait_for(self:send(method, params), timeout_ms)
  end
  function sess:close()
    pcall(function()
      proc:write(vim.json.encode({ id = 999999, method = 'shutdown' }) .. '\n')
    end)
    local done = proc:wait(3000)
    if not done then
      proc:kill(9)
    end
    return done
  end
  return sess
end

-- Runs the helper CLI. Returns the vim.system result.
function M.helper_cli(args, opts)
  return M.sh(vim.list_extend({ M.helper_bin() }, args), opts)
end

-- PNG width and height from the IHDR chunk.
function M.png_size(path)
  local s = M.read(path)
  if not s or #s < 24 or s:sub(1, 8) ~= '\137PNG\r\n\26\n' then
    return nil
  end
  local function u32(i)
    local a, b, c, d = s:byte(i, i + 3)
    return ((a * 256 + b) * 256 + c) * 256 + d
  end
  return u32(17), u32(21)
end

-- Writes a minimal valid RGBA PNG of the given size (solid color) with the
-- helper itself unavailable: uses a tiny encoder with stored zlib blocks.
function M.make_png(path, w, h, rgba)
  rgba = rgba or { 200, 30, 30, 255 }
  local function be32(n)
    return string.char(
      bit.band(bit.rshift(n, 24), 255),
      bit.band(bit.rshift(n, 16), 255),
      bit.band(bit.rshift(n, 8), 255),
      bit.band(n, 255)
    )
  end
  local crc_table = {}
  for i = 0, 255 do
    local c = i
    for _ = 1, 8 do
      if bit.band(c, 1) == 1 then
        c = bit.bxor(0xEDB88320, bit.rshift(c, 1))
      else
        c = bit.rshift(c, 1)
      end
    end
    crc_table[i] = c
  end
  local function crc(s)
    local c = 0xFFFFFFFF
    for i = 1, #s do
      c = bit.bxor(crc_table[bit.band(bit.bxor(c, s:byte(i)), 255)], bit.rshift(c, 8))
    end
    return bit.bxor(c, 0xFFFFFFFF) % 2 ^ 32
  end
  local function chunk(t, data)
    return be32(#data) .. t .. data .. be32(crc(t .. data))
  end
  local px = string.char(unpack(rgba))
  local row = '\0' .. px:rep(w)
  local raw = row:rep(h)
  -- zlib stream of stored blocks
  local parts = { '\120\1' }
  local i = 1
  while i <= #raw do
    local blk = raw:sub(i, i + 65534)
    i = i + #blk
    local final = i > #raw and 1 or 0
    local len = #blk
    local nlen = bit.band(bit.bnot(len), 0xFFFF)
    table.insert(
      parts,
      string.char(
        final,
        bit.band(len, 255),
        bit.rshift(len, 8),
        bit.band(nlen, 255),
        bit.rshift(nlen, 8)
      ) .. blk
    )
  end
  local a, b = 1, 0
  for k = 1, #raw do
    a = (a + raw:byte(k)) % 65521
    b = (b + a) % 65521
  end
  table.insert(parts, be32(b * 65536 + a))
  local ihdr = be32(w) .. be32(h) .. '\8\6\0\0\0'
  M.write(
    path,
    '\137PNG\r\n\26\n'
      .. chunk('IHDR', ihdr)
      .. chunk('IDAT', table.concat(parts))
      .. chunk('IEND', '')
  )
  return path
end

-- The delivered mermaid phase: the highest phaseN feature the dev agent's
-- justfile enables (phaseN implies the earlier ones). Phase 1 is always on.
function M.delivered_phase()
  local jf = M.read(M.root .. '/justfile') or ''
  local n = 1
  for p in jf:gmatch('phase(%d)') do
    n = math.max(n, tonumber(p))
  end
  return tonumber(vim.env.QA_PHASE) or n
end

-- Resets the plugin modules between checks. Modules without a reset are
-- reloaded on the next require.
function M.reset_plugin()
  for _, name in ipairs({ 'preview', 'helper', 'transport', 'cache', 'download', 'theme' }) do
    local mod = package.loaded['kitty-markdown.' .. name]
    if mod and type(mod.reset) == 'function' then
      pcall(mod.reset)
    end
  end
  pcall(vim.cmd, 'silent! only | silent! tabonly')
end

return M
