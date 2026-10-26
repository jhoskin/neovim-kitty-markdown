--# selene: allow(global_usage, incorrect_standard_library_use)
-- QA run: builds the helper, runs every hidden spec under held_out/tests in
-- its own nvim process, runs the dev agent's gates, and writes REVIEW.md.
--
-- Usage: nvim --clean -l held_out/run.lua [options]
--   --filter S     run only checks whose path, hint, or requirement IDs contain S
--   --spec S       run only spec files whose path contains S
--   --no-build     use the helper already built under held_out/out/target
--   --no-gates     skip the dev agent's tests and lint
--   --jobs N       spec files to run at once (default 4)
--   --no-review    do not write REVIEW.md
--
-- REVIEW.md gets only the public hint of each check and a failure class.
-- The inputs and assertion details go to held_out/out/details.md, which the
-- dev agent cannot read.

local here = vim.fn.fnamemodify(debug.getinfo(1, 'S').source:sub(2), ':p:h')
local root = vim.fn.fnamemodify(here, ':h')
local out_dir = here .. '/out'
local target_dir = out_dir .. '/target'
local helper_bin = vim.env.QA_HELPER_BIN or (target_dir .. '/release/kitty-markdown-helper')

local opts = { jobs = 4 }
do
  local i = 1
  local a = _G.arg
  while a[i] do
    local v = a[i]
    if v == '--filter' then
      i = i + 1
      opts.filter = a[i]
    elseif v == '--spec' then
      i = i + 1
      opts.spec = a[i]
    elseif v == '--jobs' then
      i = i + 1
      opts.jobs = tonumber(a[i])
    elseif v == '--no-build' then
      opts.no_build = true
    elseif v == '--no-gates' then
      opts.no_gates = true
    elseif v == '--no-review' then
      opts.no_review = true
    else
      io.stderr:write('unknown option ' .. v .. '\n')
      os.exit(2)
    end
    i = i + 1
  end
end

vim.fn.mkdir(out_dir .. '/results', 'p')
-- Partial runs (--spec) can run in parallel, so only a full run clears the
-- shared temp folder, and a partial run writes its own details file.
if not opts.spec then
  vim.fn.delete(out_dir .. '/tmp', 'rf')
end
local details_path = out_dir
  .. '/details'
  .. (opts.spec and ('-' .. opts.spec:gsub('[^%w]+', '_')) or '')
  .. '.md'

local function log(s)
  io.stdout:write(s .. '\n')
  io.stdout:flush()
end

local env_path = table.concat({
  '/opt/homebrew/bin',
  '/usr/local/bin',
  vim.env.HOME .. '/.cargo/bin',
  vim.env.HOME .. '/bin',
  vim.env.PATH,
}, ':')

local function sh(cmd, o)
  o = o or {}
  local res = vim
    .system(cmd, {
      cwd = o.cwd or root,
      text = true,
      env = vim.tbl_extend('force', { PATH = env_path }, o.env or {}),
    })
    :wait(o.timeout or 1800000)
  return res
end

local function read(p)
  local f = io.open(p, 'rb')
  if not f then
    return nil
  end
  local s = f:read('*a')
  f:close()
  return s
end

-- Requirements in SPEC.md ------------------------------------------------------

local spec_text = read(root .. '/SPEC.md') or ''
local reqs, req_order, section = {}, {}, nil
for line in (spec_text .. '\n'):gmatch('(.-)\n') do
  local h = line:match('^###%s+(.+)$')
  if h then
    section = h
  end
  local id = line:match('^%-%s+(R%-%d%d%d):')
  if id and not reqs[id] then
    reqs[id] = { id = id, section = section, checks = {} }
    table.insert(req_order, id)
  end
end
log(('SPEC.md: %d requirements'):format(#req_order))

-- Build ---------------------------------------------------------------------------

local build_error
if not opts.no_build then
  log('building the helper (release) into held_out/out/target ...')
  local r = sh({
    'cargo',
    'build',
    '--release',
    '--manifest-path',
    root .. '/helper/Cargo.toml',
    '--target-dir',
    target_dir,
  })
  if r.code ~= 0 then
    build_error = r.stderr
    log('helper build FAILED')
  end
end

-- Hidden specs ------------------------------------------------------------------

local function phase()
  local jf = read(root .. '/justfile') or ''
  local n = 1
  for p in jf:gmatch('phase(%d)') do
    n = math.max(n, tonumber(p))
  end
  return tonumber(vim.env.QA_PHASE) or n
end
local delivered = phase()
log(('delivered mermaid phase: %d'):format(delivered))

local specs = vim.fs.find(function(name)
  return name:match('_spec%.lua$') ~= nil
end, { path = here .. '/tests', type = 'file', limit = math.huge })
table.sort(specs)
specs = vim.tbl_filter(function(p)
  return not opts.spec or p:find(opts.spec, 1, true)
end, specs)

local function is_serial(path)
  local head = (read(path) or ''):sub(1, 400)
  return head:find('%-%- qa: serial') ~= nil
end

local function result_path(spec)
  return out_dir .. '/results/' .. spec:sub(#here + 2):gsub('[/%.]', '_') .. '.jsonl'
end

local FILE_TIMEOUT_MS = tonumber(vim.env.QA_FILE_TIMEOUT_MS) or 900000

local function start(spec)
  local rp = result_path(spec)
  os.remove(rp)
  local cmd = { vim.v.progpath, '--clean', '-l', here .. '/lib/run_file.lua', spec, rp }
  if opts.filter then
    table.insert(cmd, opts.filter)
  end
  local job = { spec = spec, rp = rp, t0 = vim.uv.hrtime() }
  job.proc = vim.system(cmd, {
    cwd = root,
    text = true,
    -- The QA shell may run inside tmux or zmx. Checks that need $TMUX set it
    -- themselves, so every check starts outside a multiplexer.
    env = { QA_HELPER_BIN = helper_bin, QA_PHASE = tostring(delivered), PATH = env_path, TMUX = '' },
  }, function(res)
    job.res = res
  end)
  return job
end

-- vim.system's own timeout sends SIGTERM, which nvim defers while Lua runs,
-- so a check stuck in a loop would never end. Kill with SIGKILL instead.
local function reap_if_late(job)
  if not job.res and (vim.uv.hrtime() - job.t0) / 1e6 > FILE_TIMEOUT_MS then
    job.timed_out = true
    pcall(job.proc.kill, job.proc, 9)
    vim.wait(5000, function()
      return job.res ~= nil
    end, 20)
    job.res = job.res or { code = 124, stdout = '', stderr = '' }
  end
end

local all_results = {}

local function collect(job)
  local decls, results = {}, {}
  for line in ((read(job.rp) or '') .. '\n'):gmatch('(.-)\n') do
    if line ~= '' then
      local ok, msg = pcall(vim.json.decode, line)
      if ok and msg.decl then
        decls[msg.decl.n] = msg.decl
      elseif ok and msg.result then
        if msg.result.n then
          results[msg.result.n] = msg.result
        else
          table.insert(all_results, vim.tbl_extend('force', msg.result, { spec = job.spec }))
        end
      end
    end
  end
  local rel = job.spec:sub(#here + 2)
  local tail = job.res and ((job.res.stderr or '') .. (job.res.stdout or '')):sub(-2000) or ''
  local why = job.timed_out and 'timeout' or 'crash'
  -- The first unfinished check is the one that hung or crashed; the ones
  -- after it never started, so they are not findings.
  local first_missing
  for n in pairs(decls) do
    if not results[n] and (not first_missing or n < first_missing) then
      first_missing = n
    end
  end
  for n, d in pairs(decls) do
    local r = results[n]
    if not r and n == first_missing then
      r = vim.tbl_extend('force', d, {
        status = 'fail',
        kind = why,
        detail = 'the spec process ended before this check finished\n' .. tail,
      })
    elseif not r then
      r = vim.tbl_extend('force', d, {
        status = 'notrun',
        kind = 'notrun',
        detail = 'an earlier check in the same file did not finish',
      })
    end
    r.spec = rel
    table.insert(all_results, r)
  end
  if vim.tbl_isempty(decls) and not next(results) then
    table.insert(all_results, {
      spec = rel,
      path = rel,
      reqs = {},
      hint = 'spec file runs',
      status = 'error',
      kind = 'load',
      detail = 'no output\n' .. tail,
    })
  end
  local secs = (vim.uv.hrtime() - job.t0) / 1e9
  log(('  %-50s %5.1fs'):format(rel, secs))
end

local parallel, serial = {}, {}
for _, s in ipairs(specs) do
  table.insert(is_serial(s) and serial or parallel, s)
end

log(('running %d spec files (%d serial) ...'):format(#specs, #serial))
local running, idx = {}, 1
while idx <= #parallel or #running > 0 do
  while idx <= #parallel and #running < opts.jobs do
    table.insert(running, start(parallel[idx]))
    idx = idx + 1
  end
  vim.wait(100, function()
    for _, j in ipairs(running) do
      if j.res then
        return true
      end
    end
    return false
  end, 20)
  for _, j in ipairs(running) do
    reap_if_late(j)
  end
  for i = #running, 1, -1 do
    if running[i].res then
      collect(table.remove(running, i))
    end
  end
end
for _, s in ipairs(serial) do
  local j = start(s)
  while not j.res do
    vim.wait(200, function()
      return j.res ~= nil
    end, 20)
    reap_if_late(j)
  end
  collect(j)
end

-- Dev gates ---------------------------------------------------------------------

local gates = {}
local function gate(name, req, severity, hint, cmd, o)
  log('gate: ' .. name)
  local r = sh(cmd, o)
  local ok = r.code == 0
  table.insert(gates, {
    name = name,
    req = req,
    severity = severity,
    hint = hint,
    ok = ok,
    out = ((r.stdout or '') .. (r.stderr or '')),
  })
end

if not opts.no_gates then
  gate(
    'lua unit tests',
    'R-210',
    'high',
    'The dev unit tests (`nvim -l tests/run.lua`) pass.',
    { vim.v.progpath, '--clean', '-l', 'tests/run.lua' }
  )
  gate(
    'rust tests',
    'CONV',
    'high',
    'The helper tests (`cargo test` with the phase features of the justfile) pass.',
    {
      'cargo',
      'test',
      '--manifest-path',
      'helper/Cargo.toml',
      '--features',
      'phase' .. math.max(2, delivered),
      '--target-dir',
      target_dir,
    }
  )
  gate(
    'stylua',
    'CONV',
    'low',
    '`stylua --check lua plugin tests` passes.',
    { 'stylua', '--check', 'lua', 'plugin', 'tests' }
  )
  gate(
    'selene',
    'CONV',
    'low',
    '`selene lua plugin tests` passes.',
    { 'selene', 'lua', 'plugin', 'tests' }
  )
  gate(
    'rustfmt',
    'CONV',
    'low',
    '`cargo fmt --check` passes in helper/.',
    { 'cargo', 'fmt', '--manifest-path', 'helper/Cargo.toml', '--', '--check' }
  )
  gate('clippy', 'CONV', 'low', '`cargo clippy --all-targets -- -D warnings` passes in helper/.', {
    'cargo',
    'clippy',
    '--manifest-path',
    'helper/Cargo.toml',
    '--all-targets',
    '--features',
    'phase' .. math.max(2, delivered),
    '--target-dir',
    target_dir,
    '--',
    '-D',
    'warnings',
  })
  gate('markdownlint', 'CONV', 'low', 'Markdown files pass `markdownlint-cli2` (MD013 off).', {
    'markdownlint-cli2',
    'README.md',
    'SPEC.md',
    'AGENTS.md',
    'doc/**/*.md',
    'helper/**/*.md',
    '#helper/target',
    '#helper/oracle/node_modules',
  })
end

-- Aggregate -----------------------------------------------------------------------

local SEV = { high = 3, medium = 2, low = 1 }
local KIND = {
  assertion = 'wrong result',
  error = 'Lua error or crash during the check',
  timeout = 'no result before the time limit',
  crash = 'the test process ended during the check',
}

local counts =
  { pass = 0, fail = 0, error = 0, blocked = 0, manual = 0, notdue = 0, skipped = 0, notrun = 0 }
local harness_errors = {}
for _, r in ipairs(all_results) do
  local o = r.opts or {}
  if r.status == 'skipped' then
    counts.skipped = counts.skipped + 1
  elseif o.phase and o.phase > delivered then
    r.status = 'notdue'
    counts.notdue = counts.notdue + 1
  elseif r.kind == 'load' then
    table.insert(harness_errors, r)
    counts.error = counts.error + 1
  else
    counts[r.status] = (counts[r.status] or 0) + 1
  end
  for _, id in ipairs(r.reqs or {}) do
    if reqs[id] then
      table.insert(reqs[id].checks, r)
    else
      table.insert(
        harness_errors,
        vim.tbl_extend('force', r, { detail = 'unknown requirement ID ' .. id })
      )
    end
  end
end

local findings, passed, unchecked, uncovered = {}, {}, {}, {}
for _, id in ipairs(req_order) do
  local rq = reqs[id]
  local failed, ok, other = {}, {}, {}
  for _, c in ipairs(rq.checks) do
    if c.status == 'fail' or (c.status == 'error' and c.kind ~= 'load') then
      table.insert(failed, c)
    elseif c.status == 'pass' then
      table.insert(ok, c)
    elseif c.status == 'blocked' or c.status == 'manual' then
      table.insert(other, c)
    end
  end
  if #failed > 0 then
    table.insert(findings, { req = id, failed = failed, ok = ok, total = #failed + #ok })
  elseif #ok > 0 then
    table.insert(passed, { req = id, ok = ok })
  elseif #other > 0 then
    table.insert(unchecked, { req = id, other = other })
  elseif #rq.checks == 0 then
    table.insert(uncovered, id)
  end
end

-- A check tagged with several requirements fails all of them. Findings that
-- share a failing check are one defect, so join them (connected components)
-- into one finding that names every requirement involved.
do
  local function key(c)
    return (c.spec or '') .. '#' .. tostring(c.n or c.hint)
  end
  local parent = {}
  local function find(i)
    while parent[i] ~= i do
      parent[i] = parent[parent[i]]
      i = parent[i]
    end
    return i
  end
  local owner = {}
  for i, f in ipairs(findings) do
    parent[i] = i
    for _, c in ipairs(f.failed) do
      local k = key(c)
      if owner[k] then
        parent[find(i)] = find(owner[k])
      else
        owner[k] = i
      end
    end
  end
  local merged, by_root = {}, {}
  for i, f in ipairs(findings) do
    local r = find(i)
    local m = by_root[r]
    if not m then
      m = { req = f.req, failed = {}, ok = {}, seen = {} }
      by_root[r] = m
      table.insert(merged, m)
    else
      m.req = m.req .. ', ' .. f.req
    end
    for _, list in ipairs({ { f.failed, m.failed }, { f.ok or {}, m.ok } }) do
      for _, c in ipairs(list[1]) do
        local k = key(c)
        if not m.seen[k] then
          m.seen[k] = true
          table.insert(list[2], c)
        end
      end
    end
  end
  for _, m in ipairs(merged) do
    m.total = #m.failed + #m.ok
    m.seen = nil
  end
  findings = merged
end

for _, g in ipairs(gates) do
  if not g.ok then
    table.insert(findings, { req = g.req, gate = g, total = 1 })
  end
end
if build_error then
  table.insert(findings, {
    req = 'CONV',
    gate = {
      name = 'build',
      severity = 'high',
      hint = 'The helper builds with `cargo build --release`.',
      out = build_error,
    },
    total = 1,
  })
end

-- Hints and questions are plain text that may hold `<name>` placeholders or
-- a bare `*`. Escape them outside code spans so REVIEW.md passes markdownlint.
local function md(s)
  local out, i = {}, 1
  s = tostring(s)
  while i <= #s do
    local a, b = s:find('`[^`]*`', i)
    local plain = s:sub(i, (a or #s + 1) - 1)
    table.insert(out, (plain:gsub('([<>*_])', '\\%1')))
    if not a then
      break
    end
    table.insert(out, s:sub(a, b))
    i = b + 1
  end
  return table.concat(out)
end

local function cell(s)
  return (md(s):gsub('\r?\n', ' '):gsub('|', '\\|'))
end

local function uniq_hints(list, max)
  local seen, out = {}, {}
  for _, c in ipairs(list) do
    if not seen[c.hint] then
      seen[c.hint] = true
      table.insert(out, c.hint)
    end
  end
  if #out > max then
    local n = #out - max
    out = vim.list_slice(out, 1, max)
    table.insert(out, ('and %d more behaviors in the same requirement'):format(n))
  end
  return out
end

for _, f in ipairs(findings) do
  if f.gate then
    f.severity = f.gate.severity
    f.evidence = f.gate.hint
      .. ' It does not: the command exits with an error. Run it to see the output.'
    f.fix = 'Fix the failures that the command reports.'
  else
    local sev, kinds = 0, {}
    for _, c in ipairs(f.failed) do
      sev = math.max(sev, SEV[(c.opts or {}).severity or 'high'] or 3)
      kinds[KIND[c.kind] or c.kind] = true
    end
    f.severity = ({ 'low', 'medium', 'high' })[sev]
    f.evidence = ('%d of %d hidden checks fail (%s). Expected: %s.'):format(
      #f.failed,
      f.total,
      table.concat(vim.tbl_keys(kinds), '; '),
      table.concat(uniq_hints(f.failed, 6), '; ')
    )
    local fixes = {}
    for _, c in ipairs(f.failed) do
      if c.opts and c.opts.fix then
        fixes[c.opts.fix] = true
      end
    end
    f.fix = next(fixes) and table.concat(vim.tbl_keys(fixes), ' ')
      or (
        'Make the behavior match '
        .. f.req
        .. ' in SPEC.md for every input of this kind, not only the obvious case.'
      )
  end
end
table.sort(findings, function(a, b)
  if SEV[a.severity] ~= SEV[b.severity] then
    return SEV[a.severity] > SEV[b.severity]
  end
  return a.req < b.req
end)

-- REVIEW.md -------------------------------------------------------------------------

local function sha()
  local r = sh({ 'git', 'rev-parse', '--short', 'HEAD' })
  local s = vim.trim(r.stdout or '')
  local d = sh({ 'git', 'status', '--porcelain', '--', '.', ':!held_out', ':!REVIEW.md' })
  if vim.trim(d.stdout or '') ~= '' then
    s = s .. ' (with uncommitted changes)'
  end
  return s
end

local blocking = 0
for _, f in ipairs(findings) do
  if f.severity ~= 'low' then
    blocking = blocking + 1
  end
end

local L = {}
local function w(s)
  table.insert(L, s or '')
end
w('# QA review')
w()
w('- Date: ' .. os.date('%Y-%m-%d'))
w('- Commit: ' .. sha())
w('- Verdict: ' .. (blocking == 0 and 'pass' or 'fail'))
w(('- Delivered mermaid phase: %d (from the `phase` features in the justfile)'):format(delivered))
w(
  ('- Hidden checks: %d passed, %d failed, %d not due for the delivered phase'):format(
    counts.pass,
    counts.fail + counts.error - #harness_errors,
    counts.notdue
  )
)
w()
w('The evidence describes the expected behavior in general terms. The hidden')
w('checks use inputs that are not shown here. Fix the behavior for the whole')
w('class of input that the requirement covers, then ask for a new QA run.')
w()
w('## Findings')
w()
if #findings == 0 then
  w('No findings.')
else
  w('| ID | Requirement | Severity | Evidence | Suggested fix |')
  w('| --- | --- | --- | --- | --- |')
  for i, f in ipairs(findings) do
    w(
      ('| F-%03d | %s | %s | %s | %s |'):format(i, f.req, f.severity, cell(f.evidence), cell(f.fix))
    )
  end
end
w()
w('## Passed')
w()
if #passed == 0 then
  w('- None.')
end
for _, p in ipairs(passed) do
  w(('- %s: %s.'):format(p.req, md(table.concat(uniq_hints(p.ok, 3), '; '))))
end
w()
w('## Open questions for the spec')
w()
local oq_files =
  vim.fs.find('open_questions.md', { path = here .. '/evals', type = 'file', limit = math.huge })
table.sort(oq_files)
local oq = table.concat(
  vim.tbl_map(function(p)
    return read(p) or ''
  end, oq_files),
  '\n'
)
local any = false
for line in (oq .. '\n'):gmatch('(.-)\n') do
  if line:match('^%- ') then
    w('- ' .. md(line:sub(3)))
    any = true
  end
end
for _, u in ipairs(unchecked) do
  w(
    ('- %s is not checked automatically: %s.'):format(
      u.req,
      md(table.concat(uniq_hints(u.other, 2), '; '))
    )
  )
  any = true
end
if not any then
  w('- None.')
end

if not opts.no_review then
  local f = assert(io.open(root .. '/REVIEW.md', 'w'))
  f:write(table.concat(L, '\n') .. '\n')
  f:close()
end

-- Private details for the QA agent ----------------------------------------------

local D = { '# QA details (private)', '' }
local function d(s)
  table.insert(D, s or '')
end
d(
  ('Checks: %d pass, %d fail, %d error, %d blocked, %d manual, %d not due, %d skipped, %d not run (after a hang)'):format(
    counts.pass,
    counts.fail,
    counts.error,
    counts.blocked,
    counts.manual,
    counts.notdue,
    counts.skipped,
    counts.notrun
  )
)
d()
d('## Harness errors')
d()
for _, r in ipairs(harness_errors) do
  d(('- %s: %s'):format(r.spec or r.path, cell(r.detail or '')))
end
d()
d('## Uncovered requirements')
d()
d(#uncovered == 0 and '- None.' or ('- ' .. table.concat(uncovered, ', ')))
d()
d('## Blocked checks')
d()
for _, r in ipairs(all_results) do
  if r.status == 'blocked' then
    d(('- [%s] %s / %s: needs %s'):format(table.concat(r.reqs, ','), r.spec, r.hint, r.detail))
  end
end
d()
d('## Failures')
d()
for _, r in ipairs(all_results) do
  if r.status == 'fail' or r.status == 'error' then
    d(('### [%s] %s'):format(table.concat(r.reqs or {}, ','), r.hint or ''))
    d()
    d('- Spec: ' .. (r.spec or '') .. ' / ' .. (r.path or ''))
    d('- Kind: ' .. tostring(r.kind))
    d()
    d('```text')
    d((r.detail or ''):sub(1, 4000))
    d('```')
    d()
  end
end
d('## Gates')
d()
for _, g in ipairs(gates) do
  d(('### %s: %s'):format(g.name, g.ok and 'ok' or 'FAILED'))
  if not g.ok then
    d()
    d('```text')
    d(g.out:sub(-6000))
    d('```')
  end
  d()
end
local df = assert(io.open(details_path, 'w'))
df:write(table.concat(D, '\n') .. '\n')
df:close()

log('')
log(
  ('checks: %d pass, %d fail, %d error, %d blocked, %d manual, %d not due'):format(
    counts.pass,
    counts.fail,
    counts.error,
    counts.blocked,
    counts.manual,
    counts.notdue
  )
)
log(
  ('requirements: %d with findings, %d passed, %d unchecked, %d uncovered'):format(
    #findings,
    #passed,
    #unchecked,
    #uncovered
  )
)
log(('harness errors: %d'):format(#harness_errors))
log('details: ' .. details_path)
if not opts.no_review then
  log('review:  ' .. root .. '/REVIEW.md')
end
os.exit(0)
