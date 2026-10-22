-- A stand-in for `kitty-markdown-helper serve` that answers the real
-- protocol methods the preview sends, run through the `fake-helper`
-- wrapper next to it. Unlike tests/fixtures/fake_helper.lua it never
-- echoes: it reads the PNG header of the file it is given, so the
-- preview tests exercise the size and row arithmetic on real bytes.
--
-- Behavior is driven by the request itself, so no scenario file is needed:
--
-- - `image.prepare` with a path whose basename contains `err-<code>`
--   answers that error code; `slow` answers after 400 ms; `crash` exits
--   without an answer; any other path is read as a PNG and scaled to the
--   requested width without upscaling.
-- - `fetch` with a URL that contains `err-<code>` answers that error;
--   otherwise it answers with the file named by `$KM_PREVIEW_PNG`.
-- - `mermaid.render` with a `%% err <code> [line]` comment answers that
--   error; otherwise it answers with `$KM_PREVIEW_PNG`.
-- - `cache.gc` and `probe` answer like the real helper would; `probe`
--   reads `$KM_FAKE_CACHE_BYTES` and `$KM_FAKE_VERSION`.

local uv = vim.uv
local stdin = uv.new_pipe(false)
local stdout = uv.new_pipe(false)
stdin:open(0)
stdout:open(1)

local here = debug.getinfo(1, 'S').source:sub(2)
local root = vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(here))))
local plugin_version = dofile(root .. '/lua/kitty-markdown/version.lua')

local exiting = false

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

local function be32(s, i)
  local a, b, c, d = s:byte(i, i + 3)
  return ((a * 256 + b) * 256 + c) * 256 + d
end

-- Width, height, and byte count of a PNG from its IHDR chunk.
local function png_info(path)
  local f = io.open(path, 'rb')
  if not f then
    return nil
  end
  local head = f:read(24)
  local size = f:seek('end')
  f:close()
  if not head or #head < 24 or head:sub(1, 8) ~= '\137PNG\r\n\26\n' then
    return nil
  end
  return { width = be32(head, 17), height = be32(head, 21), bytes = size }
end

-- Scale like the helper: down to the target width, never up (R-090,
-- R-091), then under the `<img>` limits (R-092).
local function scaled(info, params)
  local w = math.min(info.width, params.width_px or info.width)
  if params.max_width_px then
    w = math.min(w, params.max_width_px)
  end
  local h = math.floor(info.height * w / info.width + 0.5)
  if params.max_height_px and h > params.max_height_px then
    h = params.max_height_px
    w = math.floor(info.width * h / info.height + 0.5)
  end
  return w, h
end

local ERROR_DATA = {
  too_large = { bytes = 30000000, max_bytes = 20971520 },
  unsupported_format = { format = 'tiff' },
  unsupported_feature = { feature = 'elk' },
  unsupported_diagram_type = { diagram_type = 'zenuml' },
  fetch_failed = { status = 404 },
}

local function error_for(code, extra)
  local data = vim.tbl_extend('force', ERROR_DATA[code] or {}, extra or {})
  if next(data) == nil then
    data = vim.empty_dict()
  end
  return { code = code, message = 'fake ' .. code, data = data }
end

local function cached()
  return vim.env.KM_FAKE_CACHED == '1'
end

local function image_result(path, params)
  local info = png_info(path)
  if not info then
    return nil, { code = 'io', message = 'cannot read ' .. tostring(path), data = { path = path } }
  end
  local w, h = scaled(info, params)
  return { path = path, width_px = w, height_px = h, bytes = info.bytes, cached = cached() }
end

local methods = {}

function methods.probe(id)
  reply(id, {
    helper_version = vim.env.KM_FAKE_VERSION or plugin_version,
    mermaid_version = '12.2.0',
    cache_bytes = tonumber(vim.env.KM_FAKE_CACHE_BYTES) or 0,
    fonts = {},
    diagram_types = { 'flowchart', 'sequence', 'class', 'state' },
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
    received = { max_bytes = params.max_bytes, keep = params.keep },
  })
end

methods['image.prepare'] = function(id, params)
  local base = vim.fs.basename(params.path or '')
  local code = base:match('err%-([%w_]+)')
  if code then
    reply_error(id, error_for(code, { path = params.path }))
    return
  end
  if base:find('crash', 1, true) then
    exit(2)
    return
  end
  local result, err = image_result(params.path, params)
  local function answer()
    if err then
      reply_error(id, err)
    else
      reply(id, result)
    end
  end
  if base:find('slow', 1, true) then
    vim.defer_fn(answer, 400)
  else
    answer()
  end
end

methods.fetch = function(id, params)
  local code = (params.url or ''):match('err%-([%w_]+)')
  if code then
    reply_error(id, error_for(code))
    return
  end
  local path = vim.env.KM_PREVIEW_PNG
  local info = path and png_info(path)
  if not info then
    reply_error(id, { code = 'fetch_failed', message = 'no KM_PREVIEW_PNG', data = { status = 0 } })
    return
  end
  reply(id, { path = path, bytes = info.bytes, cached = cached() })
end

methods['mermaid.render'] = function(id, params)
  local source = params.source or ''
  local code, line = source:match('%%%% err ([%w_]+) ?(%d*)')
  local diagram_type = source:match('^%s*%%%%[^\n]*\n%s*(%S+)') or source:match('^%s*(%S+)') or ''
  if code then
    local extra = { diagram_type = diagram_type }
    local message = 'fake ' .. code
    if code == 'parse_error' then
      extra.line = tonumber(line) or 1
      extra.column = 1
      message = string.format(
        "Parse error on line %d:\n%s\n^\nExpecting 'NEWLINE', got 'INVALID'",
        extra.line,
        source
      )
    end
    local err = error_for(code, extra)
    err.message = message
    reply_error(id, err)
    return
  end
  local result, err = image_result(vim.env.KM_PREVIEW_PNG or '', params)
  if err then
    reply_error(id, err)
    return
  end
  result.diagram_type = diagram_type
  reply(id, result)
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
