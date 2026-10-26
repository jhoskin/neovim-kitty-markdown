-- Helper-level image checks: the CLI `render --image/--svg` and the
-- JSON-lines methods `image.prepare` and `svg.rasterize`.
local QA = require('qa')
local U = require('images_util')
local a = QA.assert

local F = U.fixtures
local PNG_MAGIC = '\137PNG\r\n\26\n'

local function is_png(path)
  local s = QA.read(path)
  return s ~= nil and s:sub(1, 8) == PNG_MAGIC
end

local function render(kind, input, width, extra)
  local out = QA.tmpdir() .. '/out.png'
  local args = { 'render', '--' .. kind, input, '--width', tostring(width), '-o', out }
  vim.list_extend(args, extra or {})
  local r = QA.helper_cli(args, { timeout = 20000 })
  return r, out
end

local function svg(path, w, h, body)
  return QA.write(
    path,
    ('<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d" viewBox="0 0 %d %d">%s</svg>\n'):format(
      w,
      h,
      w,
      h,
      body or ''
    )
  )
end

QA.describe('images: helper decode (CLI)', function()
  local formats = {
    { 'PNG', nil, 64, 32 },
    { 'JPEG', 'photo.jpg', 400, 200 },
    { 'GIF', 'still.gif', 30, 15 },
    { 'WebP', 'pic.webp', 60, 30 },
    { 'BMP', 'pic.bmp', 50, 25 },
  }
  for _, f in ipairs(formats) do
    QA.check('R-086', 'The helper CLI decodes a ' .. f[1] .. ' image and writes a PNG', function()
      local input = f[2] and (F .. '/' .. f[2]) or QA.make_png(QA.tmpdir() .. '/in.png', f[3], f[4])
      local r, out = render('image', input, 1000)
      a.eq(0, r.code, 'exit status; stderr: ' .. tostring(r.stderr))
      a.truthy(is_png(out), 'output starts with the PNG signature')
      local w, h = QA.png_size(out)
      a.eq({ f[3], f[4] }, { w, h }, 'a narrow image keeps its native size')
    end)
  end

  QA.check('R-086', 'A JPEG decodes to the same colors it holds (encoded as PNG)', function()
    local r, out = render('image', F .. '/photo.jpg', 1000)
    a.eq(0, r.code)
    local px = U.pixel(out, 10, 10)
    a.near(40, px[1], 6, 'red')
    a.near(160, px[2], 6, 'green')
    a.near(90, px[3], 6, 'blue')
  end)

  QA.check('R-087', 'An animated GIF renders its first frame, not a later one', function()
    local r, out = render('image', F .. '/anim.gif', 1000)
    a.eq(0, r.code, tostring(r.stderr))
    local px = U.pixel(out, 5, 5)
    a.ge(200, px[1], 'red channel of frame 1')
    a.le(50, px[2], 'green channel')
    a.le(50, px[3], 'blue channel')
  end)

  QA.check(
    'R-090',
    'The CLI scales a raster image wider than the target width down to it and keeps the aspect ratio',
    function()
      local r, out = render('image', F .. '/photo.jpg', 300)
      a.eq(0, r.code)
      local w, h = QA.png_size(out)
      a.eq(300, w, 'width')
      a.near(150, h, 1, 'height keeps the aspect ratio')
    end
  )

  QA.check('R-091', 'The CLI does not scale a raster image up past its native size', function()
    local r, out = render('image', F .. '/pic.bmp', 900)
    a.eq(0, r.code)
    local w, h = QA.png_size(out)
    a.eq({ 50, 25 }, { w, h })
  end)

  QA.check(
    'R-089',
    'The CLI rejects a raster format that the spec does not list, with a non-zero status',
    { severity = 'medium' },
    function()
      local r, out = render('image', F .. '/pic.tiff', 300)
      a.ne(0, r.code, 'exit status')
      a.falsy(is_png(out), 'no output PNG')
    end
  )

  QA.check(
    'R-088',
    'The CLI rasterizes an SVG wider than the target width at the target width',
    function()
      local p = svg(
        QA.tmpdir() .. '/wide.svg',
        400,
        200,
        '<rect x="0" y="0" width="400" height="200" fill="#2050c0"/>'
      )
      local r, out = render('svg', p, 200)
      a.eq(0, r.code, tostring(r.stderr))
      local w, h = QA.png_size(out)
      a.eq(200, w, 'width')
      a.near(100, h, 1, 'height')
    end
  )

  QA.check(
    'R-226',
    'The CLI rasterizes an SVG with no background onto transparent pixels',
    function()
      local r, out = render('svg', F .. '/drawing.svg', 100)
      a.eq(0, r.code, tostring(r.stderr))
      local corner = U.pixel(out, 0, 0)
      a.eq(0, corner[4], 'alpha of an empty corner')
      local w, h = QA.png_size(out)
      local mid = U.pixel(out, math.floor(w / 2), math.floor(h / 2))
      a.eq(255, mid[4], 'alpha of the drawn shape')
    end
  )
end)

QA.describe('images: helper protocol', function()
  local sess
  QA.before_each(function()
    sess = QA.helper_session()
  end)
  QA.after_each(function()
    if sess then
      sess:close()
    end
    sess = nil
  end)

  local function prepare(params)
    local r = sess:request('image.prepare', params, 10000)
    QA.need(r ~= nil, 'an answer to image.prepare')
    return r
  end

  local cases = {
    { 'PNG', nil, 64, 32 },
    { 'JPEG', 'photo.jpg', 400, 200 },
    { 'GIF', 'still.gif', 30, 15 },
    { 'WebP', 'pic.webp', 60, 30 },
    { 'BMP', 'pic.bmp', 50, 25 },
  }
  for _, c in ipairs(cases) do
    QA.check(
      'R-086',
      'Preparing a ' .. c[1] .. ' image gives a PNG file of its native size',
      function()
        local input = c[2] and (F .. '/' .. c[2])
          or QA.make_png(QA.tmpdir() .. '/in.png', c[3], c[4])
        local r = prepare({ path = input, width_px = 1000 })
        a.is_nil(r.error, 'error')
        a.truthy(r.result and is_png(r.result.path), 'result path is a PNG file')
        a.eq({ c[3], c[4] }, { QA.png_size(r.result.path) }, 'file size')
        a.eq({ c[3], c[4] }, { r.result.width_px, r.result.height_px }, 'reported size')
      end
    )
  end

  QA.check('R-087', 'Preparing an animated GIF keeps only the first frame', function()
    local r = prepare({ path = F .. '/anim.gif', width_px = 1000 })
    a.is_nil(r.error)
    local px = U.pixel(r.result.path, 20, 10)
    a.eq({ 255, 0, 0 }, { px[1], px[2], px[3] }, 'color of frame 1')
  end)

  QA.check(
    'R-090',
    'Preparing an image wider than the target width scales it down to that width and keeps the aspect ratio',
    function()
      local src = QA.make_png(QA.tmpdir() .. '/w.png', 600, 240)
      local r = prepare({ path = src, width_px = 300 })
      a.is_nil(r.error)
      a.eq(300, r.result.width_px, 'reported width')
      a.near(120, r.result.height_px, 1, 'reported height')
      a.eq(
        { r.result.width_px, r.result.height_px },
        { QA.png_size(r.result.path) },
        'file matches the reported size'
      )
    end
  )

  QA.check(
    'R-091',
    'Preparing an image narrower than the target width does not scale it up',
    function()
      local src = QA.make_png(QA.tmpdir() .. '/n.png', 120, 80)
      local r = prepare({ path = src, width_px = 800 })
      a.is_nil(r.error)
      a.eq({ 120, 80 }, { r.result.width_px, r.result.height_px })
    end
  )

  QA.check('R-092', 'A width limit smaller than the computed width wins', function()
    local r = prepare({ path = F .. '/photo.jpg', width_px = 300, max_width_px = 100 })
    a.is_nil(r.error)
    a.eq(100, r.result.width_px)
  end)

  QA.check('R-092', 'A height limit smaller than the computed height wins', function()
    local r = prepare({ path = F .. '/photo.jpg', width_px = 300, max_height_px = 40 })
    a.is_nil(r.error)
    a.eq(40, r.result.height_px)
  end)

  QA.check(
    'R-092',
    'A width limit larger than the computed width does not enlarge the image',
    function()
      local r = prepare({ path = F .. '/photo.jpg', width_px = 300, max_width_px = 5000 })
      a.is_nil(r.error)
      a.eq(300, r.result.width_px)
      local r2 = prepare({ path = F .. '/pic.bmp', width_px = 300, max_width_px = 200 })
      a.eq(50, r2.result.width_px, 'a limit above the native width does not scale up')
    end
  )

  QA.check(
    'R-088',
    'Rasterizing an SVG wider than the target width gives an image of the target width',
    function()
      local p =
        svg(QA.tmpdir() .. '/wide.svg', 500, 100, '<rect width="500" height="100" fill="#20a040"/>')
      local r = sess:request('svg.rasterize', { path = p, width_px = 250 }, 10000)
      a.is_nil(r.error, 'error')
      a.eq(250, r.result.width_px, 'reported width')
      a.eq(250, (QA.png_size(r.result.path)), 'file width')
      a.near(50, r.result.height_px, 1, 'height')
    end
  )

  QA.check('R-226', 'Rasterizing an SVG leaves its empty area transparent', function()
    local r = sess:request('svg.rasterize', { path = F .. '/drawing.svg', width_px = 100 }, 10000)
    a.is_nil(r.error)
    a.eq(0, U.pixel(r.result.path, 0, 0)[4], 'corner alpha')
    a.eq(
      0,
      U.pixel(r.result.path, r.result.width_px - 1, r.result.height_px - 1)[4],
      'opposite corner alpha'
    )
  end)

  QA.check(
    'R-089',
    'Preparing a raster format that the spec does not list fails with an unsupported-format error',
    function()
      local r = prepare({ path = F .. '/pic.tiff', width_px = 300 })
      a.not_nil(r.error, 'an error')
      a.matches('unsupported', vim.inspect(r.error))
    end
  )

  QA.check(
    'R-224',
    'Preparing a truncated PNG fails with an unsupported-format error, not a read error',
    function()
      local dir = QA.tmpdir()
      local full = QA.read(QA.make_png(dir .. '/full.png', 64, 64))
      local cut = QA.write(dir .. '/cut.png', full:sub(1, 70))
      local r = prepare({ path = cut, width_px = 300 })
      a.not_nil(r.error, 'an error')
      a.matches('unsupported', vim.inspect(r.error))
    end
  )

  QA.check(
    'R-172',
    'Preparing a missing file fails with a read error, not a format error',
    function()
      local r = prepare({ path = QA.tmpdir() .. '/absent.png', width_px = 300 })
      a.not_nil(r.error, 'an error')
      a.no_match('unsupported', vim.inspect(r.error))
    end
  )
end)
