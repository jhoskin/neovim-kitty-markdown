-- A stand-in for `kitty-markdown-helper serve`, run as
-- `nvim --clean -l tests/fixtures/fake_helper.lua serve --cache-dir <dir>`
-- through the `fake-helper` wrapper. It speaks the JSON-lines protocol
-- (R-138) and answers out of order on request (R-139) so the client tests
-- never need the Rust binary.
--
-- Protocol methods: `probe`, `shutdown`, `cache.gc`. Test methods:
-- `test.echo` (result = params), `test.sleep` (`ms`), `test.error`
-- (error = params), `test.crash` (exit `code` without an answer),
-- `test.reorder` (the first is held until the second arrives, then both
-- are answered in reverse), `test.stderr` (`text` goes to stderr).

local uv = vim.uv
local stdin = uv.new_pipe(false)
local stdout = uv.new_pipe(false)
local stderr = uv.new_pipe(false)
stdin:open(0)
stdout:open(1)
stderr:open(2)

local plugin_version = dofile(
  vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(debug.getinfo(1, 'S').source:sub(2))))
    .. '/lua/kitty-markdown/version.lua'
)

local exiting = false
local held = nil

local function write(msg, after)
  stdout:write(vim.json.encode(msg) .. '\n', after)
end

local function reply(id, result)
  write({ id = id, result = result })
end

local function reply_error(id, err)
  write({ id = id, error = err })
end

local function exit(code)
  exiting = true
  vim.schedule(function()
    os.exit(code)
  end)
end

local methods = {}

function methods.probe(id)
  reply(id, {
    helper_version = vim.env.KM_FAKE_VERSION or plugin_version,
    mermaid_version = '12.2.0',
    cache_bytes = tonumber(vim.env.KM_FAKE_CACHE_BYTES) or 0,
    fonts = {},
    diagram_types = {},
  })
end

function methods.shutdown(id)
  write({ id = id, result = vim.empty_dict() }, function()
    exit(0)
  end)
end

methods['cache.gc'] = function(id, params)
  reply(id, {
    deleted_bytes = 0,
    cache_bytes = 0,
    -- Echoed so the client test can check what it sent (R-152).
    received = { max_bytes = params.max_bytes, keep = params.keep },
  })
end

methods['test.echo'] = function(id, params)
  reply(id, params)
end

methods['test.sleep'] = function(id, params)
  vim.defer_fn(function()
    reply(id, { slept = params.ms })
  end, params.ms or 0)
end

methods['test.error'] = function(id, params)
  reply_error(id, params)
end

methods['test.crash'] = function(id, params)
  local _ = id
  exit(params.code or 1)
end

methods['test.reorder'] = function(id, params)
  if held == nil then
    held = id
    return
  end
  reply(id, { order = 'second', tag = params.tag })
  reply(held, { order = 'first' })
  held = nil
end

methods['test.stderr'] = function(id, params)
  stderr:write(params.text .. '\n')
  reply(id, vim.empty_dict())
end

local function handle(line)
  local ok, msg = pcall(vim.json.decode, line)
  if not ok or type(msg) ~= 'table' then
    return
  end
  local method = methods[msg.method]
  if not method then
    reply_error(
      msg.id,
      { code = 'unknown_method', message = 'unknown method ' .. tostring(msg.method) }
    )
    return
  end
  method(msg.id, msg.params or {})
end

local buffer = ''
stdin:read_start(function(err, chunk)
  if err or not chunk then
    exit(0)
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
        handle(line)
      end)
    end
  end
end)

while not exiting do
  vim.wait(1000, function()
    return exiting
  end, 10)
end
