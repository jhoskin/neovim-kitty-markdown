-- The helper CLI: render, compare, failures, no cache dir (R-144, R-145,
-- R-146, R-199, R-200).

local QA = require('qa')
local U = require('helper_util')
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

-- R-324: the reason of a failure line, or nil when stderr is not one line.
local function reason_of(stderr)
  return (stderr or ''):match('^kitty%-markdown%-helper: ([^\n]*)\n?$')
end

-- True when every backslash in `reason` starts a `\\` or `\n` escape.
local function escapes_ok(reason)
  local i = 1
  while i <= #reason do
    if reason:sub(i, i) == '\\' then
      local nxt = reason:sub(i + 1, i + 1)
      if nxt ~= '\\' and nxt ~= 'n' then
        return false
      end
      i = i + 2
    else
      i = i + 1
    end
  end
  return true
end

-- Undoes the R-324 escapes.
local function unescape(reason)
  return (reason:gsub('\\(.)', function(c)
    return c == 'n' and '\n' or c
  end))
end

-- R-324: status 1 and exactly one line `kitty-markdown-helper: <reason>`,
-- with line breaks and backslashes escaped.
local function cli_failure(res)
  a.eq(1, res.code, 'exit status')
  local reason = reason_of(res.stderr)
  a.not_nil(reason, 'exactly one prefixed line on stderr, got ' .. vim.inspect(res.stderr))
  a.matches('%S', reason, 'a reason')
  a.truthy(escapes_ok(reason), 'every backslash is an escape: ' .. reason)
  return reason
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
    { 'R-146', 'R-324' },
    'A render of a missing input file fails with one prefixed reason line on stderr and status 1',
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
      cli_failure(res)
    end
  )

  QA.check(
    { 'R-146', 'R-324' },
    'A render of a file that is not an image fails with one prefixed reason line on stderr and status 1',
    function()
      local dir = QA.tmpdir()
      local src = QA.write(dir .. '/fake.png', 'this is plain text, not pixels\n')
      local res =
        QA.helper_cli({ 'render', '--image', src, '--width', '100', '-o', dir .. '/o.png' })
      cli_failure(res)
    end
  )

  QA.check(
    { 'R-146', 'R-324' },
    'A render of invalid mermaid source fails with one prefixed reason line on stderr and status 1',
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
      cli_failure(res)
    end
  )

  QA.check(
    { 'R-146', 'R-324' },
    'A render of text that is no diagram type fails with one prefixed reason line on stderr and status 1',
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
      cli_failure(res)
    end
  )

  QA.check(
    { 'R-146', 'R-324' },
    'A render of a broken SVG fails with one prefixed reason line on stderr and status 1',
    function()
      local dir = QA.tmpdir()
      local src = QA.write(dir .. '/bad.svg', '<svg xmlns="http://www.w3.org/2000/svg"><rect')
      local res = QA.helper_cli({ 'render', '--svg', src, '--width', '200', '-o', dir .. '/o.png' })
      cli_failure(res)
    end
  )

  QA.check(
    { 'R-146', 'R-324' },
    'A render whose output path cannot be written fails with one prefixed reason line on stderr and status 1',
    function()
      local dir = QA.tmpdir()
      local src = QA.make_png(dir .. '/in.png', 50, 50)
      local blocker = QA.write(dir .. '/file', 'x')
      local res =
        QA.helper_cli({ 'render', '--image', src, '--width', '100', '-o', blocker .. '/sub/o.png' })
      cli_failure(res)
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

  QA.check(
    'R-199',
    'compare of different images prints an SSIM below 1 and not below -1',
    function()
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
      a.ge(-1, v)
    end
  )

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

local function solid(path, w, h, rgba)
  return QA.make_png(path, w, h, rgba or { 40, 90, 160, 255 })
end

-- Two bands: the color depends only on y.
local function hbands(path, w, h)
  return U.make_png_fn(path, w, h, function(_, y)
    if y < 24 then
      return 200, 30, 30, 255
    end
    return 30, 30, 200, 255
  end)
end

-- Stripes: the color depends only on x.
local function vstripes(path, w, h)
  return U.make_png_fn(path, w, h, function(x)
    if x % 8 < 4 then
      return 250, 250, 250, 255
    end
    return 20, 20, 20, 255
  end)
end

local SSIM_LINE = '^%-?%d%.%d%d%d%d\n$'

QA.describe('helper CLI: compare output', function()
  QA.check(
    { 'R-321', 'R-199' },
    'compare of equal images prints 1.0000 and a line break, nothing else, and exits 0',
    function()
      local dir = QA.tmpdir()
      local p1 = solid(dir .. '/a.png', 40, 30)
      local p2 = solid(dir .. '/b.png', 40, 30)
      local res = QA.helper_cli({ 'compare', p1, p2 })
      a.eq(0, res.code, tostring(res.stderr))
      a.eq('1.0000\n', res.stdout)
    end
  )

  QA.check(
    'R-321',
    'compare of different images prints one number with four decimals and a line break',
    function()
      local dir = QA.tmpdir()
      local p1 = hbands(dir .. '/a.png', 48, 48)
      local p2 = vstripes(dir .. '/b.png', 48, 48)
      local res = QA.helper_cli({ 'compare', p1, p2 })
      a.eq(0, res.code, tostring(res.stderr))
      a.matches(SSIM_LINE, res.stdout or '')
      a.ne('1.0000\n', res.stdout)
    end
  )

  QA.check(
    'R-321',
    'compare writes nothing to stderr when it succeeds',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local p1 = hbands(dir .. '/a.png', 32, 48)
      local res = QA.helper_cli({ 'compare', p1, p1 })
      a.eq(0, res.code)
      a.eq('', res.stderr or '')
    end
  )

  QA.check(
    'R-321',
    'compare of images smaller than the 11 pixel window still prints four decimals',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local p1 = solid(dir .. '/a.png', 6, 5)
      local p2 = vstripes(dir .. '/b.png', 6, 5)
      local res = QA.helper_cli({ 'compare', p1, p2 })
      a.eq(0, res.code, tostring(res.stderr))
      a.matches(SSIM_LINE, res.stdout or '')
    end
  )
end)

QA.describe('helper CLI: compare sizes', function()
  QA.check(
    'R-322',
    'compare of images 1 pixel apart in width crops to the common size and succeeds',
    function()
      local dir = QA.tmpdir()
      local p1 = hbands(dir .. '/a.png', 64, 48)
      local p2 = hbands(dir .. '/b.png', 65, 48)
      local res = QA.helper_cli({ 'compare', p1, p2 })
      a.eq(0, res.code, tostring(res.stderr))
      a.eq('1.0000\n', res.stdout)
    end
  )

  QA.check(
    'R-322',
    'compare of images 1 pixel apart in height crops to the common size and succeeds',
    function()
      local dir = QA.tmpdir()
      local p1 = vstripes(dir .. '/a.png', 64, 48)
      local p2 = vstripes(dir .. '/b.png', 64, 49)
      local res = QA.helper_cli({ 'compare', p1, p2 })
      a.eq(0, res.code, tostring(res.stderr))
      a.eq('1.0000\n', res.stdout)
    end
  )

  QA.check(
    'R-322',
    'compare of images 1 pixel apart on both axes, in opposite directions, succeeds',
    function()
      local dir = QA.tmpdir()
      local p1 = solid(dir .. '/a.png', 64, 49)
      local p2 = solid(dir .. '/b.png', 65, 48)
      local res = QA.helper_cli({ 'compare', p1, p2 })
      a.eq(0, res.code, tostring(res.stderr))
      a.eq('1.0000\n', res.stdout)
    end
  )

  QA.check(
    'R-322',
    'compare crops around the centers, so a 1 pixel margin on one side does not lower the SSIM much',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      -- A centered square; the larger image has one extra column.
      local function square(path, w)
        return U.make_png_fn(path, w, 48, function(x, y)
          local cx = (w - 1) / 2
          if math.abs(x - cx) < 12 and math.abs(y - 23.5) < 12 then
            return 0, 0, 0, 255
          end
          return 255, 255, 255, 255
        end)
      end
      local p1 = square(dir .. '/a.png', 64)
      local p2 = square(dir .. '/b.png', 65)
      local res = QA.helper_cli({ 'compare', p1, p2 })
      a.eq(0, res.code, tostring(res.stderr))
      a.matches(SSIM_LINE, res.stdout or '')
    end
  )

  QA.check(
    'R-323',
    'compare of images more than 1 pixel apart in width reports both sizes on stderr and exits 1',
    function()
      local dir = QA.tmpdir()
      local p1 = solid(dir .. '/a.png', 64, 48)
      local p2 = solid(dir .. '/b.png', 66, 48)
      local res = QA.helper_cli({ 'compare', p1, p2 })
      a.eq(1, res.code)
      a.eq('kitty-markdown-helper: image sizes differ: 64x48 vs 66x48\n', res.stderr)
      a.eq('', res.stdout or '', 'stdout')
    end
  )

  QA.check(
    'R-323',
    'compare of images more than 1 pixel apart in height reports both sizes in argument order',
    function()
      local dir = QA.tmpdir()
      local p1 = solid(dir .. '/a.png', 30, 52)
      local p2 = solid(dir .. '/b.png', 30, 40)
      local res = QA.helper_cli({ 'compare', p1, p2 })
      a.eq(1, res.code)
      a.eq('kitty-markdown-helper: image sizes differ: 30x52 vs 30x40\n', res.stderr)
    end
  )

  QA.check(
    'R-323',
    'compare fails on sizes when one axis is within 1 pixel but the other is not',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local p1 = solid(dir .. '/a.png', 64, 48)
      local p2 = solid(dir .. '/b.png', 65, 50)
      local res = QA.helper_cli({ 'compare', p1, p2 })
      a.eq(1, res.code)
      a.eq('kitty-markdown-helper: image sizes differ: 64x48 vs 65x50\n', res.stderr)
    end
  )
end)

QA.describe('helper CLI: failure lines and arguments', function()
  QA.check(
    'R-324',
    'compare of a missing file fails with one prefixed reason line and status 1',
    function()
      local dir = QA.tmpdir()
      local p = solid(dir .. '/a.png', 8, 8)
      local res = QA.helper_cli({ 'compare', p, dir .. '/missing.png' })
      cli_failure(res)
    end
  )

  QA.check(
    'R-324',
    'compare of a file that is not a PNG fails with one prefixed reason line and status 1',
    function()
      local dir = QA.tmpdir()
      local p = solid(dir .. '/a.png', 8, 8)
      local bad = QA.write(dir .. '/b.png', 'not a picture')
      local res = QA.helper_cli({ 'compare', p, bad })
      cli_failure(res)
    end
  )

  local bad_args = {
    { 'an unknown subcommand', { 'frobnicate' } },
    { 'render without a width', { 'render', '--image', 'x.png', '-o', 'o.png' } },
    {
      'render with a width that is not a number',
      { 'render', '--image', 'x.png', '--width', 'wide', '-o', 'o.png' },
    },
    { 'render without an output path', { 'render', '--image', 'x.png', '--width', '10' } },
    { 'render without an input', { 'render', '--width', '10', '-o', 'o.png' } },
    {
      'render with two inputs',
      { 'render', '--image', 'x.png', '--svg', 'y.svg', '--width', '10', '-o', 'o.png' },
    },
    { 'compare with one file', { 'compare', 'a.png' } },
    {
      'an unknown flag',
      { 'render', '--image', 'x.png', '--width', '10', '-o', 'o.png', '--sparkle' },
    },
  }
  for _, case in ipairs(bad_args) do
    QA.check('R-325', 'Invalid CLI arguments exit with status 2: ' .. case[1], function()
      local dir = QA.tmpdir()
      local res = QA.helper_cli(case[2], { cwd = dir })
      a.eq(2, res.code, 'exit status; stderr: ' .. tostring(res.stderr))
    end)
  end
end)

QA.describe('helper CLI: failure line escapes', function()
  QA.check(
    'R-324',
    'A failure reason that spans several lines is written on one line with each line break as \\n',
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
      local reason = cli_failure(res)
      a.contains('\\n', reason, 'an escaped line break in the reason')
      local lines = vim.split(unescape(reason), '\n', { plain = true })
      a.ge(2, #lines, 'the unescaped reason has several lines')
    end
  )

  QA.check(
    'R-324',
    'A failure reason that holds a backslash writes it as two backslashes',
    function()
      local dir = QA.tmpdir()
      local missing = dir .. '/back\\slash-missing.png'
      local res =
        QA.helper_cli({ 'render', '--image', missing, '--width', '100', '-o', dir .. '/o.png' })
      local reason = cli_failure(res)
      QA.need(reason:find('slash-missing', 1, true), 'a reason that names the input path')
      a.contains('back\\\\slash-missing', reason, 'the backslash is doubled')
      a.contains(missing, unescape(reason), 'the unescaped reason names the path')
    end
  )

  QA.check(
    'R-324',
    'A multi-line failure reason that quotes source with a backslash escapes both',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local src = QA.write(dir .. '/bad.mmd', 'flowchart TD\n  Q\\R --> ((( ]]\n')
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
      local reason = cli_failure(res)
      QA.need(reason:find('Q', 1, true), 'a reason that quotes the source')
      a.contains('\\n', reason, 'an escaped line break')
      a.contains('Q\\\\R', reason, 'the quoted backslash is doubled')
    end
  )

  QA.check(
    'R-324',
    'A failure reason with no line break or backslash is written unchanged on one line',
    { severity = 'medium' },
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
      local reason = cli_failure(res)
      a.not_contains('\\\\', reason, 'no doubled backslash without a backslash in the reason')
    end
  )
end)
