-- Helper process client: spawn, JSON lines, timeouts, restart (package L-E).
-- Signatures: AGENTS.md, Conventions, Lua module contracts.
--
-- One helper per Neovim instance (R-136). The client starts it on the
-- first request, probes it (R-142), and restarts it after a crash
-- (R-140). Every callback runs through `vim.schedule`, so callers may
-- touch buffers and windows from inside one.

local config = require('kitty-markdown.config')
local cache = require('kitty-markdown.cache')
local download = require('kitty-markdown.download')
local version = require('kitty-markdown.version')

local M = {}

local BINARY = 'kitty-markdown-helper'
local STDERR_LINES = 50
local MAX_EXITS = 3
local EXIT_WINDOW_MS = 60000
local NO_HELPER_MESSAGE = 'kitty-markdown: no helper binary found; run: '
  .. 'cargo build --release --manifest-path helper/Cargo.toml'

local state = 'absent'
local proc, stdin, stdout, stderr = nil, nil, nil, nil
local exited = true
local expected_exit = false
local on_expected_exit = nil
local next_id = 1
local pending = {}
local queue = {}
local waiters = {}
local stderr_ring = {}
local info = nil
local exits = {}
local binary_kind = nil
local redownloaded = false
local locator = nil
local notified_no_helper = false
local warned_version = false
local warned_user_path = false

local function plugin_root()
  local source = debug.getinfo(1, 'S').source:sub(2)
  local here = vim.fn.fnamemodify(source, ':p')
  return vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(here)))
end

local function executable(path)
  return type(path) == 'string' and vim.fn.executable(path) == 1
end

-- Locate order from AGENTS.md: user path, install dir, download, local
-- build, none. A user path turns the download off (R-135) even when the
-- file is missing, because the user asked for that binary and no other.
local function locate(cb)
  local cfg = config.get().helper
  if cfg.path then
    if executable(cfg.path) then
      return cb(cfg.path, 'user')
    end
    if not warned_user_path then
      warned_user_path = true
      vim.notify(
        'kitty-markdown: helper.path ' .. cfg.path .. ' is not executable',
        vim.log.levels.WARN
      )
    end
  end
  local installed = download.binary_path()
  if executable(installed) then
    return cb(installed, 'installed')
  end
  local local_build = plugin_root() .. '/helper/target/release/' .. BINARY
  local function fallback()
    if executable(local_build) then
      return cb(local_build, 'local')
    end
    cb(nil)
  end
  if cfg.download and not cfg.path then
    download.fetch(version, function(err, path)
      if not err and executable(path) then
        return cb(path, 'installed')
      end
      fallback()
    end)
    return
  end
  fallback()
end

local function close_handle(handle)
  if handle and not handle:is_closing() then
    handle:close()
  end
end

local function close_handles()
  close_handle(stdin)
  close_handle(stdout)
  close_handle(stderr)
  close_handle(proc)
  stdin, stdout, stderr, proc = nil, nil, nil, nil
end

local function send(msg)
  if not stdin or stdin:is_closing() then
    return false
  end
  stdin:write(vim.json.encode(msg) .. '\n')
  return true
end

-- JSON encodes an empty Lua table as an array, and the helper wants an
-- object for `params`.
local function as_object(params)
  if params == nil or next(params) == nil then
    return vim.empty_dict()
  end
  return params
end

local function drop_pending(id)
  local entry = pending[id]
  if not entry then
    return nil
  end
  pending[id] = nil
  entry.timer:stop()
  close_handle(entry.timer)
  return entry
end

local function fail_pending(err)
  local entries = pending
  pending = {}
  for _, entry in pairs(entries) do
    entry.timer:stop()
    close_handle(entry.timer)
    vim.schedule(function()
      entry.cb(err, nil)
    end)
  end
end

local function fail_waiting(err)
  local queued = queue
  queue = {}
  for _, req in ipairs(queued) do
    vim.schedule(function()
      req.cb(err, nil)
    end)
  end
  local waiting = waiters
  waiters = {}
  for _, cb in ipairs(waiting) do
    vim.schedule(function()
      cb(err)
    end)
  end
end

local function start_request(method, params, cb)
  local id = next_id
  next_id = next_id + 1
  local timeout = config.get().helper.request_timeout_ms
  local timer = vim.uv.new_timer()
  pending[id] = { cb = cb, timer = timer, method = method }
  timer:start(timeout, 0, function()
    -- A reply that arrives after this point finds no entry and is dropped,
    -- so the caller hears exactly once (R-141).
    local entry = drop_pending(id)
    if not entry then
      return
    end
    vim.schedule(function()
      entry.cb({
        code = 'timeout',
        message = string.format('%s had no answer within %d ms', method, timeout),
      }, nil)
    end)
  end)
  if not send({ id = id, method = method, params = as_object(params) }) then
    local entry = drop_pending(id)
    vim.schedule(function()
      entry.cb({ code = 'helper_exit', message = 'helper is not running' }, nil)
    end)
  end
  return id
end

local function dispatch(line)
  local ok, msg = pcall(vim.json.decode, line)
  if not ok or type(msg) ~= 'table' or msg.id == nil then
    vim.notify('kitty-markdown: helper sent a malformed line: ' .. line, vim.log.levels.DEBUG)
    return
  end
  local entry = drop_pending(msg.id)
  if not entry then
    return
  end
  if msg.error ~= nil and msg.error ~= vim.NIL then
    local err = msg.error
    if type(err) ~= 'table' then
      err = { code = 'internal', message = tostring(err) }
    end
    err.code = err.code or 'internal'
    entry.cb(err, nil)
  else
    entry.cb(nil, msg.result)
  end
end

local function push_stderr(line)
  stderr_ring[#stderr_ring + 1] = line
  if #stderr_ring > STDERR_LINES then
    table.remove(stderr_ring, 1)
  end
end

local function flush_ready()
  state = 'ready'
  local queued = queue
  queue = {}
  for _, req in ipairs(queued) do
    start_request(req.method, req.params, req.cb)
  end
  local waiting = waiters
  waiters = {}
  for _, cb in ipairs(waiting) do
    vim.schedule(function()
      cb(nil)
    end)
  end
end

local function describe_exit(code, signal)
  if signal and signal ~= 0 then
    return 'signal ' .. tostring(signal)
  end
  return 'code ' .. tostring(code)
end

local function handle_exit(code, signal, reason)
  exited = true
  close_handles()
  local what = reason or ('helper exited with ' .. describe_exit(code, signal))
  fail_pending({ code = 'helper_exit', message = what })
  if expected_exit then
    expected_exit = false
    local continue = on_expected_exit
    on_expected_exit = nil
    if continue then
      continue()
    else
      state = 'absent'
    end
    return
  end
  fail_waiting({ code = 'helper_exit', message = what })
  -- The one message of an exit (R-313). Every vim.notify level reaches
  -- :messages, so the stderr tail stays behind M.stderr_tail().
  vim.notify('kitty-markdown: ' .. what .. '; it restarts on the next request', vim.log.levels.WARN)
  state = 'absent'
  local now = vim.uv.now()
  local recent = { now }
  for _, at in ipairs(exits) do
    if now - at < EXIT_WINDOW_MS then
      recent[#recent + 1] = at
    end
  end
  exits = recent
  if #exits >= MAX_EXITS then
    state = 'failed'
    vim.notify(
      'kitty-markdown: helper exited '
        .. MAX_EXITS
        .. ' times within 60 seconds; run :KittyMarkdown refresh to start it again',
      vim.log.levels.ERROR
    )
  end
end

local start

local function probe(path, kind)
  start_request(
    'probe',
    { plugin_version = version, cache_dir = cache.root() },
    function(err, result)
      if err then
        if err.code ~= 'helper_exit' then
          -- The process is alive but does not speak the protocol: treat it
          -- like a crash so the exit counter can stop the restarts.
          fail_waiting(err)
          if proc and not exited then
            proc:kill('sigkill')
          end
        end
        return
      end
      info = result
      cache.note_probe(result.cache_bytes)
      if result.helper_version ~= version then
        if kind == 'installed' and not redownloaded then
          -- R-143: a stale download is replaced once; a second mismatch
          -- means the release is wrong and the warning below says so.
          redownloaded = true
          expected_exit = true
          on_expected_exit = function()
            os.remove(path)
            start()
          end
          proc:kill('sigterm')
          return
        end
        if not warned_version then
          warned_version = true
          vim.notify(
            string.format(
              'kitty-markdown: helper version %s differs from plugin version %s',
              tostring(result.helper_version),
              version
            ),
            vim.log.levels.WARN
          )
        end
      end
      flush_ready()
    end
  )
end

local function spawn(path, kind)
  binary_kind = kind
  stdin, stdout, stderr = vim.uv.new_pipe(false), vim.uv.new_pipe(false), vim.uv.new_pipe(false)
  exited = false
  expected_exit = false
  local handle, err = vim.uv.spawn(path, {
    args = { 'serve', '--cache-dir', cache.root() },
    stdio = { stdin, stdout, stderr },
  }, function(code, signal)
    vim.schedule(function()
      handle_exit(code, signal)
    end)
  end)
  if not handle then
    handle_exit(nil, nil, 'helper failed to start: ' .. tostring(err))
    return
  end
  proc = handle
  local buffer = ''
  stdout:read_start(function(_, chunk)
    if not chunk then
      return
    end
    buffer = buffer .. chunk
    while true do
      local nl = buffer:find('\n', 1, true)
      if not nl then
        break
      end
      local line = buffer:sub(1, nl - 1)
      buffer = buffer:sub(nl + 1)
      if line ~= '' then
        vim.schedule(function()
          dispatch(line)
        end)
      end
    end
  end)
  stderr:read_start(function(_, chunk)
    if not chunk then
      return
    end
    for line in chunk:gmatch('[^\n]+') do
      push_stderr(line)
    end
  end)
  probe(path, kind)
end

start = function()
  state = 'starting'
  local find = locator or locate
  find(function(path, kind)
    if not path then
      state = 'failed'
      if not notified_no_helper then
        notified_no_helper = true
        vim.notify(NO_HELPER_MESSAGE, vim.log.levels.ERROR)
      end
      fail_waiting({ code = 'no_helper', message = 'no helper binary found' })
      return
    end
    spawn(path, kind or 'user')
  end)
end

local function unavailable()
  return { code = 'no_helper', message = 'helper unavailable until :KittyMarkdown refresh' }
end

--- Send `method` with `params`; `cb(err, result)` runs once, scheduled.
--- A request made before the helper is ready waits for the probe.
function M.request(method, params, cb)
  cb = cb or function() end
  if state == 'ready' then
    start_request(method, params, cb)
    return
  end
  if state == 'failed' then
    vim.schedule(function()
      cb(unavailable(), nil)
    end)
    return
  end
  queue[#queue + 1] = { method = method, params = params, cb = cb }
  if state == 'absent' then
    start()
  end
end

--- Start the helper when needed; `cb(err)` runs once it is ready.
function M.ensure(cb)
  cb = cb or function() end
  if state == 'ready' then
    vim.schedule(function()
      cb(nil)
    end)
    return
  end
  if state == 'failed' then
    vim.schedule(function()
      cb(unavailable())
    end)
    return
  end
  waiters[#waiters + 1] = cb
  if state == 'absent' then
    start()
  end
end

--- Ask the helper to exit and wait at most `timeout_ms` (default 2000),
--- then kill it (R-137, R-197).
function M.shutdown(timeout_ms)
  if not proc or exited then
    return
  end
  expected_exit = true
  on_expected_exit = nil
  send({ id = next_id, method = 'shutdown', params = vim.empty_dict() })
  next_id = next_id + 1
  vim.wait(timeout_ms or 2000, function()
    return exited
  end, 10)
  if not exited and proc then
    proc:kill('sigkill')
    vim.wait(1000, function()
      return exited
    end, 10)
  end
  if state ~= 'failed' then
    state = 'absent'
  end
end

function M.is_running()
  return proc ~= nil and not exited
end

function M.state()
  return state
end

--- The last `probe` answer: `helper_version`, `mermaid_version`,
--- `cache_bytes`, `fonts`, `diagram_types`. Nil before the first probe.
function M.info()
  return info
end

--- Which route found the binary: `user`, `installed`, or `local`.
function M.binary_kind()
  return binary_kind
end

--- The last 50 stderr lines of the current or last helper process.
function M.stderr_tail()
  return vim.deepcopy(stderr_ring)
end

--- Forget the exit counter so the next request may start the helper
--- again, and the restart stop needs three new exits (R-196, R-244). The
--- preview calls this on every `:KittyMarkdown refresh` (R-310).
function M.reset_failures()
  exits = {}
  if state == 'failed' then
    state = 'absent'
  end
end

--- Test hook: `fn(cb)` calls `cb(path, kind)` or `cb(nil)` in place of
--- the real locate order.
function M.set_locator(fn)
  locator = fn
end

--- Test hook: the real locate order, exposed so tests can drive it.
M.locate = locate

--- Test hook: kill any process and forget every piece of state.
function M.reset()
  if proc and not exited then
    expected_exit = true
    on_expected_exit = nil
    proc:kill('sigkill')
    vim.wait(2000, function()
      return exited
    end, 10)
  end
  close_handles()
  for id in pairs(pending) do
    drop_pending(id)
  end
  state = 'absent'
  exited = true
  expected_exit = false
  on_expected_exit = nil
  next_id = 1
  pending, queue, waiters = {}, {}, {}
  stderr_ring = {}
  info = nil
  exits = {}
  binary_kind = nil
  redownloaded = false
  locator = nil
  notified_no_helper = false
  warned_version = false
  warned_user_path = false
end

vim.api.nvim_create_autocmd('VimLeavePre', {
  group = vim.api.nvim_create_augroup('kitty-markdown-helper', { clear = true }),
  callback = function()
    M.shutdown(2000)
  end,
})

return M
