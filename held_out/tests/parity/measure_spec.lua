-- qa: serial
-- R-214, R-301, R-302: the text measurer against the measurement oracle
-- (helper/oracle/measure.mjs). Two fixtures, both in held_out/evals/parity:
-- - measure_oracle.json: the output of helper/oracle/measure.mjs itself.
-- - measure_fixture.json: measure_corpus.mjs, which repeats the same
--   getBBox (SVG label) and getBoundingClientRect (HTML label) measurement
--   on the same page for the hidden label corpus.
-- The measure command has no weight or style option, so only regular,
-- upright cases are compared. A label is fallback text when it has at least
-- one character outside ASCII (definition); the tolerance applies to the
-- width and to the height of the whole label (R-301).
local QA = require('qa')
local P = require('parity_util')
local a = QA.assert

local TOL = { latin = 0.5, fallback = 3 }

local function load(file)
  local s = QA.read(QA.evals .. '/parity/' .. file)
  QA.need(s ~= nil, 'held_out/evals/parity/' .. file)
  return vim.json.decode(s).cases
end

local function class_of(text)
  return text:find('[\128-\255]') and 'fallback' or 'latin'
end

local cases
local function all_cases()
  if not cases then
    cases = {}
    for _, file in ipairs({ 'measure_oracle.json', 'measure_fixture.json' }) do
      for _, c in ipairs(load(file)) do
        local regular = (c.weight == nil or c.weight == 400)
          and (c.style == nil or c.style == 'normal')
        if regular and c.text ~= '' then
          table.insert(cases, c)
        end
      end
    end
  end
  return cases
end

local function measure(c, mode)
  local r = QA.helper_cli({
    'measure',
    '--text',
    c.text,
    '--font',
    c.family,
    '--size',
    tostring(c.size),
    '--mode',
    mode,
  })
  QA.need(not (r.stderr or ''):find('unrecognized'), 'the helper measure command')
  if r.code ~= 0 then
    return nil, r.stderr
  end
  return vim.json.decode(r.stdout)
end

local function run(class, mode)
  P.need_macos()
  local fails, n = {}, 0
  for _, c in ipairs(all_cases()) do
    if class_of(c.text) == class then
      n = n + 1
      local got, err = measure(c, mode)
      local want = c[mode]
      if not got then
        table.insert(fails, ('%q: measure failed: %s'):format(c.text, err or ''))
      else
        local dw, dh = got.width - want.width, got.height - want.height
        if math.abs(dw) > TOL[class] or math.abs(dh) > TOL[class] then
          table.insert(
            fails,
            ('%q %s %dpx: dw %.2f dh %.2f'):format(c.text, c.family, c.size, dw, dh)
          )
        end
      end
    end
  end
  QA.need(n > 0, 'measurement cases')
  a.truthy(
    #fails == 0,
    ('%d of %d %s %s cases off: %s'):format(#fails, n, class, mode, table.concat(fails, ' | '))
  )
end

QA.describe('parity: text measurer against the measurement oracle', function()
  QA.check(
    { 'R-214', 'R-301' },
    'ASCII SVG labels match the measurement oracle within 0.5 px in width and height',
    function()
      run('latin', 'svg')
    end
  )

  QA.check(
    { 'R-214', 'R-301' },
    'ASCII HTML labels match the measurement oracle within 0.5 px in width and height',
    function()
      run('latin', 'html')
    end
  )

  QA.check(
    { 'R-214', 'R-301', 'R-302' },
    'SVG labels with a character outside ASCII match the measurement oracle within 3 px in width and height',
    function()
      run('fallback', 'svg')
    end
  )

  QA.check(
    { 'R-214', 'R-301', 'R-302' },
    'HTML labels with a character outside ASCII match the measurement oracle within 3 px in width and height',
    function()
      run('fallback', 'html')
    end
  )
end)
