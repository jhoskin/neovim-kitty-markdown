-- qa: serial
-- R-214: the text measurer against the measurement oracle. The fixture
-- held_out/evals/parity/measure_fixture.json comes from
-- held_out/evals/parity/measure_corpus.mjs, which repeats the getBBox (SVG
-- label) and getBoundingClientRect (HTML label) measurements of
-- helper/oracle/measure.mjs for the hidden label corpus.
local QA = require('qa')
local a = QA.assert

local TOL = { latin = 0.5, fallback = 3 }

local function fixture()
  local s = QA.read(QA.evals .. '/parity/measure_fixture.json')
  QA.need(s ~= nil, 'held_out/evals/parity/measure_fixture.json')
  return vim.json.decode(s).cases
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
  QA.need(r.code == 0 or not (r.stderr or ''):find('unrecognized'), 'the helper measure command')
  if r.code ~= 0 then
    return nil, r.stderr
  end
  return vim.json.decode(r.stdout)
end

local function run(class, mode)
  local fails, n = {}, 0
  for _, c in ipairs(fixture()) do
    if c.class == class then
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
            ('%q %s %dpx: dw %.2f dh %.2f'):format(c.text, c.family_id, c.size, dw, dh)
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
  QA.check('R-214', 'Latin SVG label sizes match the measurement oracle within 0.5 px', function()
    run('latin', 'svg')
  end)

  QA.check('R-214', 'Latin HTML label sizes match the measurement oracle within 0.5 px', function()
    run('latin', 'html')
  end)

  QA.check(
    'R-214',
    'SVG label sizes for text that needs a fallback font match the measurement oracle within 3 px',
    function()
      run('fallback', 'svg')
    end
  )

  QA.check(
    'R-214',
    'HTML label sizes for text that needs a fallback font match the measurement oracle within 3 px',
    function()
      run('fallback', 'html')
    end
  )
end)
