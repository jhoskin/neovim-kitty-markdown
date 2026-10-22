--# selene: allow(global_usage)
-- Busted-compatible subset for `nvim --clean -l tests/run.lua [filter]`.
-- busted and luarocks are not installed on the dev machine and plenary is
-- out of scope, so the handful of primitives the specs use live here.
-- A spec file is any `tests/**/*_spec.lua`. The optional filter is a plain
-- substring matched against the file path and the full test name.

local here = debug.getinfo(1, 'S').source:sub(2)
local root = vim.fs.dirname(vim.fs.dirname(vim.fn.fnamemodify(here, ':p')))
vim.opt.runtimepath:prepend(root)

local filter = _G.arg[1]
if filter == '' then
  filter = nil
end

local results = { passed = 0, failed = 0, pending = 0, failures = {} }
local stack = {}
local current_file = ''

local function full_name(name)
  local parts = {}
  for _, frame in ipairs(stack) do
    parts[#parts + 1] = frame.name
  end
  parts[#parts + 1] = name
  return table.concat(parts, ' ')
end

-- Assertion failures carry a marker so the reporter can print them without
-- the Lua traceback that a genuine runtime error deserves.
local function fail(message)
  -- selene: allow(incorrect_standard_library_use)
  error({ busted = true, message = message }, 3)
end

local function show(value)
  return vim.inspect(value)
end

local function message_of(err)
  if type(err) == 'table' then
    if err.busted then
      return err.message
    end
    return show(err)
  end
  return tostring(err)
end

local function with_prefix(msg, detail)
  if msg then
    return msg .. '\n' .. detail
  end
  return detail
end

local assertions = { are = {}, are_not = {}, is = {} }

function assertions.are.same(expected, actual, msg)
  if not vim.deep_equal(expected, actual) then
    fail(with_prefix(msg, 'expected:\n' .. show(expected) .. '\nactual:\n' .. show(actual)))
  end
end

function assertions.are.equal(expected, actual, msg)
  if expected ~= actual then
    fail(with_prefix(msg, 'expected: ' .. show(expected) .. '\nactual:   ' .. show(actual)))
  end
end

function assertions.are_not.same(expected, actual, msg)
  if vim.deep_equal(expected, actual) then
    fail(with_prefix(msg, 'expected a different value, both are:\n' .. show(actual)))
  end
end

function assertions.are_not.equal(expected, actual, msg)
  if expected == actual then
    fail(with_prefix(msg, 'expected a different value, both are: ' .. show(actual)))
  end
end

function assertions.is_true(value, msg)
  if value ~= true then
    fail(with_prefix(msg, 'expected true, got ' .. show(value)))
  end
end

function assertions.is_false(value, msg)
  if value ~= false then
    fail(with_prefix(msg, 'expected false, got ' .. show(value)))
  end
end

function assertions.is_nil(value, msg)
  if value ~= nil then
    fail(with_prefix(msg, 'expected nil, got ' .. show(value)))
  end
end

function assertions.is_not_nil(value, msg)
  if value == nil then
    fail(with_prefix(msg, 'expected a value, got nil'))
  end
end

function assertions.truthy(value, msg)
  if not value then
    fail(with_prefix(msg, 'expected a truthy value, got ' .. show(value)))
  end
end

function assertions.falsy(value, msg)
  if value then
    fail(with_prefix(msg, 'expected a falsy value, got ' .. show(value)))
  end
end

function assertions.has_error(fn, pattern, msg)
  local ok, err = pcall(fn)
  if ok then
    fail(with_prefix(msg, 'expected an error, the function returned normally'))
  end
  if pattern then
    local text = message_of(err)
    if not text:find(pattern) then
      fail(with_prefix(msg, 'error does not match ' .. show(pattern) .. ':\n' .. text))
    end
  end
end

function assertions.matches(pattern, actual, msg)
  if type(actual) ~= 'string' or not actual:find(pattern) then
    fail(with_prefix(msg, show(actual) .. ' does not match ' .. show(pattern)))
  end
end

assertions.is.truthy = assertions.truthy
assertions.is.falsy = assertions.falsy

setmetatable(assertions, {
  __call = function(_, condition, msg)
    if not condition then
      fail(msg or 'assertion failed')
    end
    return condition
  end,
})

_G.assert = assertions

local function matches_filter(name)
  if not filter then
    return true
  end
  return name:find(filter, 1, true) ~= nil or current_file:find(filter, 1, true) ~= nil
end

local function trace_handler(err)
  return { err = err, trace = debug.traceback('', 2) }
end

local function spec_lines(trace)
  local lines = {}
  for line in trace:gmatch('[^\n]+') do
    if line:find('_spec%.lua') or line:find('/lua/kitty%-markdown/') then
      lines[#lines + 1] = vim.trim(line)
    end
  end
  return table.concat(lines, '\n')
end

local function record_failure(name, caught)
  results.failed = results.failed + 1
  local detail = message_of(caught.err)
  local where = spec_lines(caught.trace)
  if where ~= '' then
    detail = detail .. '\n' .. where
  end
  results.failures[#results.failures + 1] = { name = name, detail = detail }
  print('FAIL     ' .. name)
end

function _G.describe(name, body)
  stack[#stack + 1] = { name = name, before = {}, after = {} }
  local ok, caught = xpcall(body, trace_handler)
  if not ok then
    record_failure(full_name('(describe)'), caught)
  end
  table.remove(stack)
end

function _G.before_each(fn)
  local frame = stack[#stack]
  frame.before[#frame.before + 1] = fn
end

function _G.after_each(fn)
  local frame = stack[#stack]
  frame.after[#frame.after + 1] = fn
end

function _G.pending(name)
  local full = full_name(name)
  if not matches_filter(full) then
    return
  end
  results.pending = results.pending + 1
  print('pending  ' .. full)
end

function _G.it(name, body)
  local full = full_name(name)
  if not matches_filter(full) then
    return
  end
  local before, after = {}, {}
  for _, frame in ipairs(stack) do
    vim.list_extend(before, frame.before)
  end
  for i = #stack, 1, -1 do
    vim.list_extend(after, stack[i].after)
  end
  local ok, caught = xpcall(function()
    for _, fn in ipairs(before) do
      fn()
    end
    body()
  end, trace_handler)
  -- after_each hooks run even when the body failed, so state never leaks
  -- into the next test.
  local after_ok, after_caught = xpcall(function()
    for _, fn in ipairs(after) do
      fn()
    end
  end, trace_handler)
  if ok and after_ok then
    results.passed = results.passed + 1
    print('ok       ' .. full)
  else
    record_failure(full, ok and after_caught or caught)
  end
end

local files = vim.fs.find(function(name)
  return name:match('_spec%.lua$') ~= nil
end, { path = root .. '/tests', type = 'file', limit = math.huge })
table.sort(files)

for _, file in ipairs(files) do
  current_file = file:sub(#root + 2)
  local ok, caught = xpcall(function()
    dofile(file)
  end, trace_handler)
  if not ok then
    record_failure(current_file .. ' (load)', caught)
  end
end

print('')
for _, failure in ipairs(results.failures) do
  print('--- ' .. failure.name)
  print(failure.detail)
  print('')
end
print(
  string.format('%d passed, %d failed, %d pending', results.passed, results.failed, results.pending)
)

os.exit(results.failed > 0 and 1 or 0)
