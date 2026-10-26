-- qa: serial
-- The helper's `compare` against an independent SSIM that follows the
-- SPEC.md definition (composite over white, BT.601 grayscale, 11x11
-- Gaussian window with sigma 1.5, K1 0.01, K2 0.03, mean over the image).
-- The spec does not say how the window treats the image border, so a score
-- passes when it matches either border reading within 0.001.
local QA = require('qa')
local P = require('parity_util')
local a = QA.assert

local TOL = 0.001

local pairs_dir
local function test_pairs()
  if not pairs_dir then
    local dir = QA.tmpdir()
    P.py({ 'pairs', dir })
    pairs_dir = dir
  end
  return pairs_dir
end

-- Asserts that the helper's SSIM of a and b matches the reference.
local function agrees(pa, pb, label)
  local ref = P.py({ 'ssim', pa, pb })
  a.is_nil(ref.error, label .. ': reference SSIM')
  local got, out = P.compare(pa, pb)
  a.not_nil(got, label .. ': compare printed no number: ' .. out)
  local ok = math.abs(got - ref.same) <= TOL
    or (ref.valid ~= nil and math.abs(got - ref.valid) <= TOL)
  a.truthy(
    ok,
    ('%s: compare %.4f, reference same %.4f valid %s'):format(
      label,
      got,
      ref.same,
      tostring(ref.valid)
    )
  )
  return got
end

QA.describe('parity: SSIM definition', function()
  QA.check(
    { 'R-110' },
    'The compare command reports an SSIM of 1 for two identical images',
    function()
      local d = test_pairs()
      local got = P.compare(d .. '/base.png', d .. '/base.png')
      a.not_nil(got, 'compare output')
      a.near(1, got, TOL, 'identical images')
    end
  )

  QA.check(
    { 'R-110' },
    'The compare command matches the SSIM definition for shifted and noisy images',
    function()
      local d = test_pairs()
      for _, name in ipairs({ 'shifted', 'noise' }) do
        local got = agrees(d .. '/base.png', d .. '/' .. name .. '.png', name)
        a.le(0.95, got, name .. ' must score below 1')
      end
    end
  )

  QA.check(
    { 'R-110' },
    'The compare command composites both images over white before it computes the SSIM',
    function()
      local d = test_pairs()
      local got = P.compare(d .. '/clear.png', d .. '/white.png')
      a.not_nil(got, 'compare output')
      a.near(1, got, TOL, 'transparent image against white')
      agrees(d .. '/base.png', d .. '/alpha.png', 'half transparent')
      agrees(d .. '/base.png', d .. '/clear.png', 'fully transparent')
    end
  )

  QA.check(
    { 'R-110' },
    'The compare command converts color to BT.601 grayscale before it computes the SSIM',
    function()
      local d = test_pairs()
      agrees(d .. '/base.png', d .. '/gray.png', 'gray copy')
      agrees(d .. '/base.png', d .. '/swapped.png', 'swapped channels')
    end
  )

  QA.check(
    { 'R-110', 'R-237' },
    'The compare command matches the SSIM definition for a framed helper render against an oracle golden',
    function()
      local m = P.manifest()
      local done = 0
      for _, key in ipairs({ 'flowchart/subgraphs_lr', 'sequence/multilingual', 'pie/budget' }) do
        local e = m[key]
        if e and e.helper and e.goldens and e.goldens[1] then
          local dtype, name = key:match('^(.-)/(.*)$')
          local tmp = QA.tmpdir()
          local hp = tmp .. '/helper.png'
          local r = QA.helper_cli({
            'render',
            '--mermaid',
            P.dir .. '/' .. key .. '.mmd',
            '--width',
            tostring(P.WIDTH),
            '--theme',
            'default',
            '-o',
            hp,
          })
          local w, h = QA.png_size(hp)
          if
            r.code == 0
            and w
            and math.abs(w - e.helper.width) <= 1
            and math.abs(h - e.helper.height) <= 1
          then
            local fr = P.py({ 'frame', hp, P.dir .. '/' .. dtype .. '/' .. e.goldens[1].file, tmp })
            if not fr.size_mismatch then
              agrees(fr.framed.spec.path, fr.oracle_path, name)
              done = done + 1
            end
          end
        end
      end
      QA.need(done > 0, 'a helper render with the size of a stored golden')
    end
  )
end)
