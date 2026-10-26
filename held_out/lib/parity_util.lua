-- Shared code for the hidden parity checks (R-110, R-181, R-182, R-236,
-- R-237). The goldens and manifest.json come from
-- held_out/evals/parity/make_goldens.mjs. The crop and the independent SSIM
-- are in held_out/evals/parity/tools/parity.py.
--
-- Per case: render the helper at 960 px (R-236), take the golden rendered
-- with the R-237 `--size`, crop the golden to the helper size from the top
-- left (R-181), and compare at equal pixel size with
-- `kitty-markdown-helper compare`. No image is resampled (R-237).

local QA = require('qa')

local M = {}

M.dir = QA.evals .. '/parity'
M.tool = M.dir .. '/tools/parity.py'
M.mmdc = QA.root .. '/helper/oracle/node_modules/.bin/mmdc'
M.oracle_config = QA.root .. '/helper/oracle/mermaid-config.json'
M.puppeteer_config = QA.root .. '/helper/oracle/puppeteer.json'
M.WIDTH = 960 -- R-236
M.THRESHOLD = 0.95 -- R-110, R-182
M.UNSEEDED_RUNS = 5 -- R-182

-- The mermaid phase of each corpus folder (Constraints table).
M.PHASE = {
  flowchart = 1,
  sequence = 1,
  class = 1,
  state = 1,
  er = 2,
  pie = 2,
  gantt = 2,
  xychart = 2,
  quadrant = 2,
  gitgraph = 3,
  mindmap = 3,
  timeline = 3,
  journey = 3,
  sankey = 4,
  block = 4,
  packet = 4,
  kanban = 4,
  architecture = 4,
  c4 = 4,
  requirement = 4,
  radar = 5,
  treemap = 5,
  venn = 5,
  ishikawa = 5,
  wardley = 5,
  cynefin = 5,
  treeview = 5,
  eventmodeling = 5,
  usecase = 5,
  swimlane = 5,
}

-- Constraints: the parity check and the text metrics calibration run on
-- macOS. Elsewhere the checks are blocked, not failed.
function M.need_macos()
  QA.need(
    vim.uv.os_uname().sysname == 'Darwin',
    'macOS (parity and text metrics run on macOS only)'
  )
end

-- The first python3 that has numpy and Pillow: $QA_PYTHON, python3 on
-- PATH (the runner puts Homebrew first), then the pyenv versions.
local python
function M.need_python()
  if python == nil then
    python = false
    local candidates = { vim.env.QA_PYTHON or 'python3', 'python3' }
    vim.list_extend(
      candidates,
      vim.fn.glob(vim.env.HOME .. '/.pyenv/versions/*/bin/python3', false, true)
    )
    table.insert(candidates, '/usr/bin/python3')
    for _, c in ipairs(candidates) do
      if c and c ~= '' and vim.fn.executable(c) == 1 then
        local r = QA.sh({ c, '-I', '-c', 'import numpy, PIL' })
        if r.code == 0 then
          python = c
          break
        end
      end
    end
  end
  QA.need(python, 'python3 with numpy and Pillow for the image crop and the reference SSIM')
end

function M.py(args)
  M.need_python()
  local r = QA.sh(vim.list_extend({ python, '-I', M.tool }, args))
  if r.code ~= 0 then
    error('parity.py ' .. args[1] .. ' failed: ' .. (r.stderr or ''))
  end
  return vim.json.decode(r.stdout)
end

local manifest
function M.manifest()
  if not manifest then
    local s = QA.read(M.dir .. '/manifest.json')
    QA.need(s ~= nil, 'held_out/evals/parity/manifest.json (run make_goldens.mjs)')
    manifest = vim.json.decode(s)
  end
  return manifest
end

function M.cases(dtype)
  local out = {}
  for name, kind in vim.fs.dir(M.dir .. '/' .. dtype) do
    if kind == 'file' and name:match('%.mmd$') then
      table.insert(out, (name:gsub('%.mmd$', '')))
    end
  end
  table.sort(out)
  return out
end

-- The SSIM that `kitty-markdown-helper compare` prints, or nil and the
-- output when it prints no number.
function M.compare(a, b)
  local r = QA.helper_cli({ 'compare', a, b })
  local n = r.code == 0 and tonumber((r.stdout or ''):match('[-%d%.]+')) or nil
  return n, (r.stdout or '') .. (r.stderr or '')
end

-- R-237: the oracle --size for a helper output of w x h. aspect is the
-- oracle viewBox width divided by its height, the ratio mermaid-cli uses.
function M.oracle_size(dtype, w, h, fixed, aspect)
  if dtype == 'gantt' then
    return w
  end
  if fixed then
    return math.max(w, h)
  end
  local target = w + 8
  if w >= h then
    return target
  end
  local n = math.max(1, math.ceil((target - 0.5) / aspect))
  while math.floor(n * aspect + 0.5) < target do
    n = n + 1
  end
  return n
end

local function oracle_png(mmd, out, size)
  local r = QA.sh({
    M.mmdc,
    '-i',
    mmd,
    '-o',
    out,
    '--size',
    tostring(size),
    '-b',
    'white',
    '-c',
    M.oracle_config,
    '-p',
    M.puppeteer_config,
    '-q',
  })
  return r.code == 0
end

-- The goldens for one case at the helper size w x h. The stored goldens
-- were rendered for the helper size at generation time. When the helper
-- size changed, render fresh goldens with the R-237 size for the new one.
local function goldens_for(dtype, name, entry, w, h, tmp)
  local stored = entry.helper
  if stored and stored.width == w and stored.height == h then
    local out = {}
    for _, g in ipairs(entry.goldens or {}) do
      table.insert(out, M.dir .. '/' .. dtype .. '/' .. g.file)
    end
    return out, false
  end
  QA.need(
    vim.fn.executable(M.mmdc) == 1,
    'the pinned oracle in helper/oracle (npm ci --prefix helper/oracle)'
  )
  local nat = entry.natural
  local size = M.oracle_size(dtype, w, h, nat.fixed, nat.width / nat.height)
  local runs = #(entry.goldens or {}) > 1 and M.UNSEEDED_RUNS or 1
  local out = {}
  local mmd = M.dir .. '/' .. dtype .. '/' .. name .. '.mmd'
  for i = 1, runs do
    local p = ('%s/golden.%d.png'):format(tmp, i)
    if oracle_png(mmd, p, size) then
      table.insert(out, p)
    end
  end
  return out, true
end

-- Scores one case. Returns { score = best SSIM or nil, note = text }.
function M.score_case(dtype, name)
  local entry = M.manifest()[dtype .. '/' .. name]
  if not entry or entry.oracle_error then
    return { skip = true, note = 'no oracle golden' }
  end
  local tmp = QA.tmpdir()
  local hp = tmp .. '/helper.png'
  local mmd = M.dir .. '/' .. dtype .. '/' .. name .. '.mmd'
  local r = QA.helper_cli({
    'render',
    '--mermaid',
    mmd,
    '--width',
    tostring(M.WIDTH),
    '--theme',
    'default',
    '-o',
    hp,
  })
  if r.code ~= 0 then
    return {
      note = 'helper render failed: ' .. vim.trim((r.stderr or '') .. (r.stdout or '')):sub(1, 200),
    }
  end
  local w, h = QA.png_size(hp)
  if not w then
    return { note = 'helper wrote no PNG' }
  end
  local goldens, fresh = goldens_for(dtype, name, entry, w, h, tmp)
  local best, notes = nil, {}
  for i, g in ipairs(goldens) do
    local cdir = ('%s/c%d'):format(tmp, i)
    vim.fn.mkdir(cdir, 'p')
    local cr = M.py({ 'crop', hp, g, cdir })
    if cr.mismatch then
      table.insert(
        notes,
        ('golden %d is %dx%d, helper %dx%d'):format(i, cr.golden[1], cr.golden[2], w, h)
      )
    else
      local s = M.compare(cr.helper_path, cr.golden_path)
      if s and (not best or s > best) then
        best = s
      end
    end
  end
  local note = ('helper %dx%d%s; %s'):format(
    w,
    h,
    fresh and ' (fresh goldens)' or '',
    table.concat(notes, '; ')
  )
  return { score = best, note = note }
end

local function record(dtype, rows)
  local dir = QA.out .. '/parity_scores'
  vim.fn.mkdir(dir, 'p')
  QA.write(dir .. '/' .. dtype .. '.json', vim.json.encode(rows) .. '\n')
end

-- The per-type check body: every case must reach the threshold against its
-- golden (any one of the five for mindmap and cynefin, R-182).
--
-- opts.size_only: assert only that each case reaches the comparison at
-- equal pixel size within 1 px (R-181), not the SSIM threshold. Used for
-- gantt, where the R-237 size makes mermaid-cli draw 8 px narrower than the
-- helper output (open question).
function M.check_type(dtype, opts)
  opts = opts or {}
  M.need_macos()
  M.need_python()
  M.manifest()
  local fails, rows = {}, {}
  local cases = M.cases(dtype)
  QA.need(#cases > 0, 'parity cases for ' .. dtype)
  for _, name in ipairs(cases) do
    local res = M.score_case(dtype, name)
    rows[name] = res
    local ok = res.score and (opts.size_only or res.score >= M.THRESHOLD)
    if not res.skip and not ok then
      table.insert(
        fails,
        ('%s: SSIM %s (%s)'):format(
          name,
          res.score and ('%.4f'):format(res.score) or 'none',
          res.note
        )
      )
    end
  end
  record(dtype, rows)
  QA.assert.truthy(
    #fails == 0,
    ('%d of %d cases below %.2f: %s'):format(
      #fails,
      #cases,
      M.THRESHOLD,
      table.concat(fails, ' | ')
    )
  )
end

return M
