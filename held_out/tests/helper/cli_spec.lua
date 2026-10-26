-- The helper CLI: render, compare, failures, no cache dir (R-144, R-145,
-- R-146, R-199, R-200).

local QA = require('qa')
local a = QA.assert

local FLOW =
  'flowchart TD\n  Start --> Check{Ready?}\n  Check -->|yes| Done\n  Check -->|no| Start\n'
local SVG = '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100">'
  .. '<rect x="10" y="10" width="180" height="80" fill="#2a7"/><circle cx="100" cy="50" r="30" fill="#fff"/></svg>'

-- An environment where no cache directory can exist: HOME and the XDG
-- dirs point below a regular file.
local function no_cache_env()
  local dir = QA.tmpdir()
  local blocker = QA.write(dir .. '/not-a-dir', 'x')
  return {
    HOME = blocker .. '/home',
    XDG_CACHE_HOME = blocker .. '/cache',
    XDG_DATA_HOME = blocker .. '/data',
    XDG_CONFIG_HOME = blocker .. '/config',
    TMPDIR = vim.env.TMPDIR,
    PATH = vim.env.PATH,
  }
end

local function first_number(s)
  local n = (s or ''):match('(%-?%d+%.?%d*[eE]?[%-+]?%d*)')
  return n and tonumber(n)
end

QA.describe('helper CLI: render', function()
  QA.check('R-144', 'render --mermaid writes a PNG at the -o path and exits 0', function()
    local dir = QA.tmpdir()
    local src = QA.write(dir .. '/d.mmd', FLOW)
    local out = dir .. '/out.png'
    local res = QA.helper_cli({
      'render',
      '--mermaid',
      src,
      '--width',
      '640',
      '--theme',
      'default',
      '-o',
      out,
    })
    a.eq(0, res.code, 'exit status; stderr: ' .. tostring(res.stderr))
    local w, h = QA.png_size(out)
    a.not_nil(w, 'a PNG at the -o path')
    a.ge(1, w)
    a.ge(1, h)
    a.le(640, w, 'no wider than the requested width')
  end)

  QA.check(
    'R-144',
    'render --mermaid accepts each mermaid theme name',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local src = QA.write(dir .. '/d.mmd', FLOW)
      for _, theme in ipairs({ 'default', 'dark', 'neutral', 'forest', 'base' }) do
        local out = dir .. '/' .. theme .. '.png'
        local res = QA.helper_cli({
          'render',
          '--mermaid',
          src,
          '--width',
          '500',
          '--theme',
          theme,
          '-o',
          out,
        })
        a.eq(0, res.code, 'theme ' .. theme .. ': ' .. tostring(res.stderr))
        a.not_nil(QA.png_size(out), 'PNG for theme ' .. theme)
      end
    end
  )

  QA.check('R-145', 'render --image writes a PNG at the -o path and exits 0', function()
    local dir = QA.tmpdir()
    local src = QA.make_png(dir .. '/in.png', 300, 150)
    local out = dir .. '/out.png'
    local res = QA.helper_cli({ 'render', '--image', src, '--width', '120', '-o', out })
    a.eq(0, res.code, 'exit status; stderr: ' .. tostring(res.stderr))
    local w, h = QA.png_size(out)
    a.not_nil(w, 'a PNG at the -o path')
    a.le(120, w, 'no wider than the requested width')
    a.ge(1, h)
  end)

  QA.check('R-145', 'render --svg writes a PNG at the -o path and exits 0', function()
    local dir = QA.tmpdir()
    local src = QA.write(dir .. '/in.svg', SVG)
    local out = dir .. '/out.png'
    local res = QA.helper_cli({ 'render', '--svg', src, '--width', '300', '-o', out })
    a.eq(0, res.code, 'exit status; stderr: ' .. tostring(res.stderr))
    local w, h = QA.png_size(out)
    a.not_nil(w, 'a PNG at the -o path')
    a.le(300, w, 'no wider than the requested width')
    a.ge(1, h)
  end)

  QA.check('R-145', 'render accepts the arguments in any order', { severity = 'low' }, function()
    local dir = QA.tmpdir()
    local src = QA.make_png(dir .. '/in.png', 200, 100)
    local out = dir .. '/out.png'
    local res = QA.helper_cli({ 'render', '-o', out, '--width', '100', '--image', src })
    a.eq(0, res.code, tostring(res.stderr))
    a.not_nil(QA.png_size(out))
  end)

  QA.check(
    'R-145',
    'render --image keeps the aspect ratio when it scales down to the width',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local src = QA.make_png(dir .. '/in.png', 400, 200)
      local out = dir .. '/out.png'
      local res = QA.helper_cli({ 'render', '--image', src, '--width', '200', '-o', out })
      a.eq(0, res.code, tostring(res.stderr))
      local w, h = QA.png_size(out)
      a.eq(200, w)
      a.near(100, h, 1)
    end
  )
end)

QA.describe('helper CLI: failures', function()
  QA.check(
    'R-146',
    'A render of a missing input file fails with a reason on stderr and a non-zero status',
    function()
      local dir = QA.tmpdir()
      local res = QA.helper_cli({
        'render',
        '--image',
        dir .. '/missing.png',
        '--width',
        '100',
        '-o',
        dir .. '/o.png',
      })
      a.ne(0, res.code, 'exit status')
      a.matches('%S', res.stderr or '', 'a reason on stderr')
    end
  )

  QA.check(
    'R-146',
    'A render of a file that is not an image fails with a reason on stderr and a non-zero status',
    function()
      local dir = QA.tmpdir()
      local src = QA.write(dir .. '/fake.png', 'this is plain text, not pixels\n')
      local res =
        QA.helper_cli({ 'render', '--image', src, '--width', '100', '-o', dir .. '/o.png' })
      a.ne(0, res.code, 'exit status')
      a.matches('%S', res.stderr or '', 'a reason on stderr')
    end
  )

  QA.check(
    'R-146',
    'A render of invalid mermaid source fails with a reason on stderr and a non-zero status',
    function()
      local dir = QA.tmpdir()
      local src = QA.write(dir .. '/bad.mmd', 'flowchart TD\n  A --> \n  ((( ]]\n')
      local res = QA.helper_cli({
        'render',
        '--mermaid',
        src,
        '--width',
        '400',
        '--theme',
        'default',
        '-o',
        dir .. '/o.png',
      })
      a.ne(0, res.code, 'exit status')
      a.matches('%S', res.stderr or '', 'a reason on stderr')
    end
  )

  QA.check(
    'R-146',
    'A render of text that is no diagram type fails with a reason on stderr and a non-zero status',
    function()
      local dir = QA.tmpdir()
      local src = QA.write(dir .. '/none.mmd', 'just some words here\n')
      local res = QA.helper_cli({
        'render',
        '--mermaid',
        src,
        '--width',
        '400',
        '--theme',
        'default',
        '-o',
        dir .. '/o.png',
      })
      a.ne(0, res.code, 'exit status')
      a.matches('%S', res.stderr or '', 'a reason on stderr')
    end
  )

  QA.check(
    'R-146',
    'A render of a broken SVG fails with a reason on stderr and a non-zero status',
    function()
      local dir = QA.tmpdir()
      local src = QA.write(dir .. '/bad.svg', '<svg xmlns="http://www.w3.org/2000/svg"><rect')
      local res = QA.helper_cli({ 'render', '--svg', src, '--width', '200', '-o', dir .. '/o.png' })
      a.ne(0, res.code, 'exit status')
      a.matches('%S', res.stderr or '', 'a reason on stderr')
    end
  )

  QA.check(
    'R-146',
    'A render whose output path cannot be written fails with a reason on stderr and a non-zero status',
    function()
      local dir = QA.tmpdir()
      local src = QA.make_png(dir .. '/in.png', 50, 50)
      local blocker = QA.write(dir .. '/file', 'x')
      local res =
        QA.helper_cli({ 'render', '--image', src, '--width', '100', '-o', blocker .. '/sub/o.png' })
      a.ne(0, res.code, 'exit status')
      a.matches('%S', res.stderr or '', 'a reason on stderr')
    end
  )
end)

QA.describe('helper CLI: no cache directory', function()
  QA.check('R-200', 'render --image works when no cache directory can exist', function()
    local dir = QA.tmpdir()
    local src = QA.make_png(dir .. '/in.png', 120, 60)
    local out = dir .. '/out.png'
    local res = QA.helper_cli(
      { 'render', '--image', src, '--width', '100', '-o', out },
      { env = no_cache_env(), clear_env = true }
    )
    a.eq(0, res.code, 'exit status; stderr: ' .. tostring(res.stderr))
    a.not_nil(QA.png_size(out), 'a PNG at the -o path')
  end)

  QA.check('R-200', 'render --mermaid works when no cache directory can exist', function()
    local dir = QA.tmpdir()
    local src = QA.write(dir .. '/d.mmd', FLOW)
    local out = dir .. '/out.png'
    local res = QA.helper_cli(
      { 'render', '--mermaid', src, '--width', '400', '--theme', 'default', '-o', out },
      { env = no_cache_env(), clear_env = true }
    )
    a.eq(0, res.code, 'exit status; stderr: ' .. tostring(res.stderr))
    a.not_nil(QA.png_size(out), 'a PNG at the -o path')
  end)

  QA.check('R-200', 'render --svg works when no cache directory can exist', function()
    local dir = QA.tmpdir()
    local src = QA.write(dir .. '/in.svg', SVG)
    local out = dir .. '/out.png'
    local res = QA.helper_cli(
      { 'render', '--svg', src, '--width', '200', '-o', out },
      { env = no_cache_env(), clear_env = true }
    )
    a.eq(0, res.code, 'exit status; stderr: ' .. tostring(res.stderr))
    a.not_nil(QA.png_size(out), 'a PNG at the -o path')
  end)
end)

QA.describe('helper CLI: compare', function()
  QA.check('R-199', 'compare of a PNG with itself prints an SSIM of 1', function()
    local dir = QA.tmpdir()
    local p = QA.make_png(dir .. '/a.png', 64, 48)
    local res = QA.helper_cli({ 'compare', p, p })
    a.eq(0, res.code, tostring(res.stderr))
    a.near(1, first_number(res.stdout), 1e-6)
  end)

  QA.check('R-199', 'compare of two equal files prints an SSIM of 1', function()
    local dir = QA.tmpdir()
    local p1 = QA.make_png(dir .. '/a.png', 64, 48, { 10, 120, 200, 255 })
    local p2 = QA.make_png(dir .. '/b.png', 64, 48, { 10, 120, 200, 255 })
    local res = QA.helper_cli({ 'compare', p1, p2 })
    a.eq(0, res.code, tostring(res.stderr))
    a.near(1, first_number(res.stdout), 1e-6)
  end)

  QA.check('R-199', 'compare of different images prints an SSIM below 1 and not below 0', function()
    local dir = QA.tmpdir()
    local p1 = QA.write(dir .. '/a.svg', SVG)
    local pa, pb = dir .. '/a.png', dir .. '/b.png'
    a.eq(0, QA.helper_cli({ 'render', '--svg', p1, '--width', '200', '-o', pa }).code)
    QA.make_png(pb, 200, 100, { 255, 255, 255, 255 })
    QA.need(select(2, QA.png_size(pa)) == 100, 'the SVG render size')
    local res = QA.helper_cli({ 'compare', pa, pb })
    a.eq(0, res.code, tostring(res.stderr))
    local v = first_number(res.stdout)
    a.not_nil(v, 'a number on stdout')
    a.le(0.99, v)
    a.ge(0, v)
  end)

  QA.check(
    'R-199',
    'compare composites transparency over white before it compares',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local clear = QA.make_png(dir .. '/clear.png', 32, 32, { 0, 0, 0, 0 })
      local white = QA.make_png(dir .. '/white.png', 32, 32, { 255, 255, 255, 255 })
      local res = QA.helper_cli({ 'compare', clear, white })
      a.eq(0, res.code, tostring(res.stderr))
      a.near(1, first_number(res.stdout), 1e-6)
    end
  )
end)
