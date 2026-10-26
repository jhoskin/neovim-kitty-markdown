-- QA stand-in for `kitty-markdown-helper serve`, run as
-- `nvim --clean -l qa_fake_helper.lua serve --cache-dir <dir>` through a
-- wrapper that helper_util.lua writes. The wrapper bakes the environment in:
--
--   QA_FAKE_MODE     comma list of: exit_at_start, crash, hang, garbage,
--                    ignore_shutdown, silent_probe
--   QA_FAKE_LOG      file that gets one line per event (start, method, exit)
--   QA_FAKE_VERSION  the helper version that probe reports
--   QA_FAKE_DELAY    ms before a pixel answer
--   QA_FAKE_PNG, QA_FAKE_W, QA_FAKE_H  the pixel result
--   QA_FAKE_SHUTDOWN_DELAY  ms between the shutdown answer and the exit

local uv = vim.uv
local mode = {}
for m in (vim.env.QA_FAKE_MODE or ''):gmatch('[^,]+') do
  mode[m] = true
end

local function log(s)
  local path = vim.env.QA_FAKE_LOG
  if not path or path == '' then
    return
  end
  local f = io.open(path, 'a')
  if f then
    f:write(s .. '\n')
    f:close()
  end
end

log('start ' .. uv.os_getpid())

if mode.exit_at_start then
  log('exit 3')
  os.exit(3)
end

local stdin = uv.new_pipe(false)
local stdout = uv.new_pipe(false)
stdin:open(0)
stdout:open(1)

local done = false

local function quit(code)
  log('exit ' .. code)
  done = true
  vim.schedule(function()
    os.exit(code)
  end)
end

local function write_raw(s, after)
  stdout:write(s .. '\n', after)
end

local function reply(id, result, after)
  write_raw(vim.json.encode({ id = id, result = result }), after)
end

local function pixel(id)
  if mode.crash then
    log('crash')
    quit(5)
    return
  end
  if mode.hang then
    return
  end
  if mode.garbage then
    write_raw('this is not json')
    write_raw('{"id":')
    write_raw(vim.json.encode({ id = id + 100000, result = vim.empty_dict() }))
    return
  end
  local function answer()
    reply(id, {
      path = vim.env.QA_FAKE_PNG,
      width_px = tonumber(vim.env.QA_FAKE_W) or 40,
      height_px = tonumber(vim.env.QA_FAKE_H) or 20,
      cached = true,
    })
    log('answered ' .. id)
  end
  local delay = tonumber(vim.env.QA_FAKE_DELAY) or 0
  if delay > 0 then
    vim.defer_fn(answer, delay)
  else
    answer()
  end
end

local function handle(line)
  local ok, msg = pcall(vim.json.decode, line)
  if not ok or type(msg) ~= 'table' then
    log('bad line')
    return
  end
  log('method ' .. tostring(msg.method))
  if msg.method == 'probe' then
    if mode.silent_probe then
      return
    end
    reply(msg.id, {
      helper_version = vim.env.QA_FAKE_VERSION,
      mermaid_version = '12.2.0',
      cache_bytes = 0,
      fonts = {},
      diagram_types = {},
    })
  elseif msg.method == 'shutdown' then
    if mode.ignore_shutdown then
      return
    end
    reply(msg.id, vim.empty_dict())
    local delay = tonumber(vim.env.QA_FAKE_SHUTDOWN_DELAY) or 0
    vim.defer_fn(function()
      log('clean_exit')
      quit(0)
    end, math.max(1, delay))
  elseif msg.method == 'cache.gc' then
    reply(msg.id, { deleted_bytes = 0, cache_bytes = 0 })
  else
    pixel(msg.id)
  end
end

local buffer = ''
stdin:read_start(function(_, chunk)
  if not chunk then
    log('eof')
    if not mode.ignore_shutdown then
      quit(0)
    end
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

-- An ignore_shutdown fake outlives its parent for at most 20 s.
local started = uv.now()
while not done and uv.now() - started < 20000 do
  vim.wait(100, function()
    return done
  end, 10)
end
os.exit(0)
