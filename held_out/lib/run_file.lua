--# selene: allow(global_usage, incorrect_standard_library_use)
-- Runs one hidden spec file and writes JSON lines to the output path:
-- first one {decl=...} line per check, then one {result=...} line per check
-- as it finishes. The orchestrator uses the declarations to report checks
-- that never finished (a hang or a crash).
--
-- Usage: nvim --clean -l held_out/lib/run_file.lua <spec.lua> <out.jsonl> [filter]

local spec, out_path, filter = _G.arg[1], _G.arg[2], _G.arg[3]

package.path = vim.fn.fnamemodify(debug.getinfo(1, 'S').source:sub(2), ':h')
  .. '/?.lua;'
  .. package.path
local QA = require('qa')

local out = assert(io.open(out_path, 'w'))
local function emit(t)
  out:write(vim.json.encode(t) .. '\n')
  out:flush()
end

local ok, err = pcall(dofile, spec)
if not ok then
  emit({
    result = {
      path = spec,
      reqs = {},
      hint = 'spec file loads',
      status = 'error',
      kind = 'load',
      detail = tostring(err),
    },
  })
  out:close()
  os.exit(0)
end

-- Run one check at a time so each result is flushed before the next starts.
local all = QA._take_queue()
for i, t in ipairs(all) do
  emit({ decl = { n = i, path = t.path, reqs = t.reqs, hint = t.hint, opts = t.opts } })
end
for i, t in ipairs(all) do
  local name = t.path .. ' / ' .. (t.hint or '')
  if
    not filter
    or name:find(filter, 1, true)
    or table.concat(t.reqs, ' '):find(filter, 1, true)
  then
    local r = QA._run_one(t)
    r.n = i
    emit({ result = r })
  else
    emit({
      result = {
        n = i,
        path = t.path,
        reqs = t.reqs,
        hint = t.hint,
        opts = t.opts,
        status = 'skipped',
      },
    })
  end
end
out:close()
os.exit(0)
