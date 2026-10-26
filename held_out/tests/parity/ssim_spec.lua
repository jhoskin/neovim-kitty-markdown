-- qa: serial
-- The helper's `compare` against an independent SSIM that follows the
-- SPEC.md definition: composite over white, BT.601 grayscale, 11x11
-- Gaussian window with sigma 1.5, K1 0.01, K2 0.03, L 255, only the window
-- positions that fit inside the image (no padding), mean over those
-- positions, range -1 to 1, and whole-image statistics when a side is
-- shorter than 11 px. Every pair here has equal sizes; the output format
-- and the crop of `compare` (R-321 to R-323) are checked in tests/helper.
local QA = require('qa')
local P = require('parity_util')
local a = QA.assert

-- compare prints four decimals, so the reference may differ by rounding.
local TOL = 0.0006

local pairs_dir
local function test_pairs()
  if not pairs_dir then
    local dir = QA.tmpdir()
    P.py({ 'pairs', dir })
    pairs_dir = dir
  end
  return pairs_dir
end

-- Asserts that the helper's SSIM of two images matches the reference.
local function agrees(pa, pb, label)
  local ref = P.py({ 'ssim', pa, pb })
  a.is_nil(ref.error, label .. ': reference SSIM')
  local got, out = P.compare(pa, pb)
  a.not_nil(got, label .. ': compare printed no number: ' .. out)
  a.near(ref.ssim, got, TOL, label)
  return got, ref.ssim
end

local function pair(x, y)
  local d = test_pairs()
  return d .. '/' .. x .. '.png', d .. '/' .. y .. '.png'
end

local function agrees_pair(x, y, label)
  local pa, pb = pair(x, y)
  return agrees(pa, pb, label)
end

QA.describe('parity: SSIM definition', function()
  QA.check(
    { 'R-110' },
    'The compare command reports an SSIM of 1 for two identical images',
    function()
      local x = pair('base', 'base')
      local got = P.compare(x, x)
      a.not_nil(got, 'compare output')
      a.near(1, got, TOL, 'identical images')
    end
  )

  QA.check(
    { 'R-110' },
    'The compare command matches the SSIM definition, valid window positions only, for shifted and noisy images',
    function()
      for _, name in ipairs({ 'shifted', 'noise' }) do
        local got = agrees_pair('base', name, name)
        a.le(0.95, got, name .. ' must score below 1')
      end
    end
  )

  QA.check(
    { 'R-110' },
    'The compare command composites both images over white before it computes the SSIM',
    function()
      local got = P.compare(pair('clear', 'white'))
      a.not_nil(got, 'compare output')
      a.near(1, got, TOL, 'transparent image against white')
      agrees_pair('base', 'alpha', 'half transparent')
      agrees_pair('base', 'clear', 'fully transparent')
    end
  )

  QA.check(
    { 'R-110' },
    'The compare command converts color to BT.601 grayscale before it computes the SSIM',
    function()
      agrees_pair('base', 'gray', 'gray copy')
      agrees_pair('base', 'swapped', 'swapped channels')
    end
  )

  QA.check(
    { 'R-110' },
    'The compare command reports a negative SSIM for images with inverted structure (range -1 to 1)',
    function()
      local got = agrees_pair('checker', 'checker_inv', 'inverted pattern')
      a.le(-0.5, got, 'inverted structure scores below 0')
      a.ge(-1, got, 'SSIM is not below -1')
    end
  )

  QA.check(
    { 'R-110' },
    'The compare command uses whole-image statistics when a side is shorter than 11 pixels',
    function()
      for _, p in ipairs({
        { 'strip_a', 'strip_b' },
        { 'strip_a', 'strip_c' },
        { 'narrow_a', 'narrow_b' },
      }) do
        local x, y = pair(p[1], p[2])
        agrees(x, y, p[1] .. ' vs ' .. p[2])
      end
      local x = pair('strip_a', 'strip_a')
      local got = P.compare(x, x)
      a.not_nil(got, 'compare output for a thin image')
      a.near(1, got, TOL, 'thin identical images')
    end
  )

  QA.check(
    { 'R-110' },
    'The compare command uses the window when each side is at least 11 pixels',
    function()
      local x, y = pair('row11_a', 'row11_b')
      agrees(x, y, 'one row of window positions')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-237' },
    'The compare command matches the SSIM definition for a helper render against an oracle golden cropped to its size',
    function()
      P.need_macos()
      local m = P.manifest()
      local done = 0
      for _, key in ipairs({ 'flowchart/subgraphs_lr', 'sequence/multilingual', 'pie/budget' }) do
        local e = m[key]
        if e and e.helper and e.goldens and e.goldens[1] then
          local dtype = key:match('^(.-)/')
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
          if r.code == 0 and w == e.helper.width and h == e.helper.height then
            local cr = P.py({ 'crop', hp, P.dir .. '/' .. dtype .. '/' .. e.goldens[1].file, tmp })
            if not cr.mismatch then
              agrees(cr.helper_path, cr.golden_path, key)
              done = done + 1
            end
          end
        end
      end
      QA.need(done > 0, 'a helper render with the size of a stored golden')
    end
  )
end)
