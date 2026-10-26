-- Shared code for the hidden parity checks (R-110, R-181, R-182, R-236,
-- R-237). The goldens and manifest.json come from
-- held_out/evals/parity/make_goldens.mjs. Image work (framing, the
-- independent SSIM) is in held_out/evals/parity/tools/parity.py.

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
  QA.need(python, 'python3 with numpy and Pillow for the image framing')
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
-- were rendered at --size = the longer side of the helper output at
-- generation time (R-181). When the helper size changed by more than the
-- 1 px tolerance, render fresh goldens at the new longer side.
local function goldens_for(dtype, name, entry, w, h, tmp)
  local stored = entry.helper
  if stored and math.abs(stored.width - w) <= 1 and math.abs(stored.height - h) <= 1 then
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
  local runs = #(entry.goldens or {}) > 1 and M.UNSEEDED_RUNS or 1
  local out = {}
  local mmd = M.dir .. '/' .. dtype .. '/' .. name .. '.mmd'
  for i = 1, runs do
    local p = ('%s/golden.%d.png'):format(tmp, i)
    if oracle_png(mmd, p, math.max(w, h)) then
      table.insert(out, p)
    end
  end
  return out, true
end

-- The --size that makes mermaid-cli draw a `width="100%"` diagram exactly
-- `width` wide (its clip is 8 px wider than the drawing).
local function ideal_size(width, aspect)
  local target = width + 8
  if aspect >= 1 then
    return target
  end
  local n = math.ceil((target - 0.5) / aspect)
  while math.floor(n * aspect + 0.5) < target do
    n = n + 1
  end
  return n
end

-- The best SSIM that the R-237 framing allows for this case, and the SSIM
-- of the helper output against the oracle drawn at the helper's own pixel
-- size (no resampling). The ideal image is mermaid's own drawing at the
-- helper's scale; framed like a helper output and compared with the
-- goldens, it gives the ceiling. Returns nil when the case has no ideal.
local function ceiling(dtype, name, entry, goldens, hp, w, h, fresh, tmp)
  if not entry.ideal then
    return nil
  end
  local src = M.dir .. '/' .. dtype .. '/' .. entry.ideal.file
  if fresh then
    local nat = entry.natural[1]
    src = tmp .. '/ideal.png'
    if
      not oracle_png(
        M.dir .. '/' .. dtype .. '/' .. name .. '.mmd',
        src,
        ideal_size(w, nat.width / nat.height)
      )
    then
      return nil
    end
  end
  local fit = tmp .. '/ideal-fit.png'
  M.py({ 'fit', src, tostring(w), tostring(h), fit })
  local best
  for i, g in ipairs(goldens) do
    local fdir = ('%s/c%d'):format(tmp, i)
    vim.fn.mkdir(fdir, 'p')
    local fr = M.py({ 'frame', fit, g, fdir })
    if not fr.size_mismatch then
      for _, f in pairs(fr.framed) do
        local s = M.compare(f.path, fr.oracle_path)
        if s and (not best or s > best) then
          best = s
        end
      end
    end
  end
  local hfit = tmp .. '/helper-fit.png'
  M.py({ 'fit', hp, tostring(w), tostring(h), hfit })
  local direct = M.compare(hfit, fit)
  return best, direct
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
    local fdir = ('%s/f%d'):format(tmp, i)
    vim.fn.mkdir(fdir, 'p')
    local fr = M.py({ 'frame', hp, g, fdir })
    if fr.size_mismatch then
      table.insert(
        notes,
        ('golden %d is %dx%d, helper %dx%d'):format(i, fr.oracle[1], fr.oracle[2], w, h)
      )
    else
      for reading, f in pairs(fr.framed) do
        local s = M.compare(f.path, fr.oracle_path)
        if s and (not best or s > best) then
          best = s
          notes.best = ('golden %d, %s framing'):format(i, reading)
        end
      end
    end
  end
  local note = ('helper %dx%d%s; %s'):format(
    w,
    h,
    fresh and ' (fresh goldens)' or '',
    table.concat(notes, '; ')
  )
  if notes.best then
    note = note .. ' best: ' .. notes.best
  end
  local cap, direct
  if best and best < M.THRESHOLD then
    cap, direct = ceiling(dtype, name, entry, goldens, hp, w, h, fresh, tmp)
    if cap then
      note = note
        .. (' ceiling %.4f, same-size %s'):format(cap, direct and ('%.4f'):format(direct) or 'none')
    end
  end
  return { score = best, ceiling = cap, direct = direct, note = note }
end

local function record(dtype, rows)
  local dir = QA.out .. '/parity_scores'
  vim.fn.mkdir(dir, 'p')
  QA.write(dir .. '/' .. dtype .. '.json', vim.json.encode(rows) .. '\n')
end

-- A case passes when the framed SSIM reaches the threshold. When even
-- mermaid's own drawing cannot reach the threshold through the R-237
-- framing (the ceiling is below it), the framed score cannot decide the
-- case; it then passes when the helper output reaches the threshold
-- against the oracle drawn at the same pixel size (R-110 wording). This is
-- an open question in open_questions.md.
function M.passes(res)
  if not res.score then
    return false
  end
  if res.score >= M.THRESHOLD then
    return true
  end
  return res.ceiling ~= nil
    and res.ceiling < M.THRESHOLD
    and res.direct ~= nil
    and res.direct >= M.THRESHOLD
end

-- The per-type check body: every case must reach the threshold.
function M.check_type(dtype)
  M.need_python()
  M.manifest()
  local fails, rows = {}, {}
  local cases = M.cases(dtype)
  QA.need(#cases > 0, 'parity cases for ' .. dtype)
  for _, name in ipairs(cases) do
    local res = M.score_case(dtype, name)
    rows[name] = res
    if not res.skip and not M.passes(res) then
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
