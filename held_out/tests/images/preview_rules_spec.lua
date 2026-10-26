-- End to end: path resolution, <img> aspect ratio, file:// URLs, the
-- :messages format, fetch retry, the remote cache on failure, the size
-- cap default, and the image side of the block key.
local QA = require('qa')
local U = require('images_util')
local a = QA.assert

local srv

local function png_bytes(w, h, rgba)
  return QA.read(QA.make_png(QA.tmpdir() .. '/src.png', w, h, rgba))
end

local function only_box(r)
  a.eq(1, #r.boxes, 'one pixel block')
  return r.boxes[1]
end

local function remote_files(cache)
  return U.files(cache .. '/remote')
end

local function stop_server()
  if srv then
    srv.stop()
    srv = nil
  end
end

-- Lines of :messages that start with the plugin prefix and name `kind`.
local function failure_lines(text)
  local out = {}
  for l in text:gmatch('[^\n]+') do
    if l:find('^kitty%-markdown: ') and not l:find('graphics on', 1, true) then
      table.insert(out, l)
    end
  end
  return out
end

QA.describe('images: path resolution', function()
  QA.before_each(QA.reset_plugin)

  QA.check(
    { 'R-284', 'R-077' },
    'A relative image path resolves against the source file directory when the working directory is elsewhere',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/img/r.png', 30, 30)
      vim.fn.chdir(QA.tmpdir())
      local r = U.open({ '![r](img/r.png)' }, { dir = dir })
      a.truthy(U.is_placed(only_box(r)), r.boxes[1].text)
    end
  )

  QA.check(
    'R-284',
    'A relative image path with a leading ./ resolves against the source file directory',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/dot.png', 30, 30)
      vim.fn.chdir(QA.tmpdir())
      local r = U.open({ '![d](./dot.png)' }, { dir = dir })
      a.truthy(U.is_placed(only_box(r)), r.boxes[1].text)
    end
  )

  QA.check(
    'R-285',
    'In a buffer with no file name, a relative image path resolves against the current directory of Neovim',
    function()
      local cwd = QA.tmpdir()
      QA.make_png(cwd .. '/here.png', 30, 30)
      vim.fn.chdir(cwd)
      local r = U.open({ '![here](here.png)' }, { noname = true })
      a.truthy(U.is_placed(only_box(r)), r.boxes[1].text)
    end
  )

  QA.check(
    'R-285',
    'In a buffer with no file name, a relative image that is not in the current directory cannot be read',
    function()
      local cwd = QA.tmpdir()
      vim.fn.chdir(cwd)
      local r = U.open({ '![absent](absent.png)' }, { noname = true })
      a.contains('cannot read file', only_box(r).text)
    end
  )

  QA.check(
    'R-285',
    'In a buffer with no file name, a relative path in a subdirectory of the current directory resolves',
    function()
      local cwd = QA.tmpdir()
      QA.make_png(cwd .. '/sub/deep.png', 30, 30)
      vim.fn.chdir(cwd)
      local r = U.open({ '![deep](sub/deep.png)' }, { noname = true })
      a.truthy(U.is_placed(only_box(r)), r.boxes[1].text)
    end
  )
end)

QA.describe('images: <img> aspect ratio', function()
  QA.before_each(QA.reset_plugin)

  QA.check(
    { 'R-286', 'R-092' },
    'An <img> with only a width gets the height from the aspect ratio',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/p.png', 200, 100)
      local r = U.open({ '<img src="p.png" width="100">' }, { dir = dir, cell = { 10, 20 } })
      local b = only_box(r)
      a.truthy(U.is_placed(b), b.text)
      local rows, cols = U.grid(b)
      a.eq(10, cols, 'columns of the given width')
      a.eq(3, rows, 'rows of the height from the aspect ratio')
    end
  )

  QA.check(
    { 'R-286', 'R-092' },
    'An <img> with only a height gets the width from the aspect ratio',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/p.png', 200, 100)
      local r = U.open({ '<img src="p.png" height="40">' }, { dir = dir, cell = { 10, 20 } })
      local b = only_box(r)
      a.truthy(U.is_placed(b), b.text)
      local rows, cols = U.grid(b)
      a.eq(2, rows, 'rows of the given height')
      a.eq(8, cols, 'columns of the width from the aspect ratio')
    end
  )

  QA.check(
    'R-286',
    'An <img> width limit on an image wider than the target width keeps the aspect ratio',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/w.png', 1600, 400)
      local r = U.open({ '<img src="w.png" width="400">' }, { dir = dir, cell = { 10, 20 } })
      local rows, cols = U.grid(only_box(r))
      a.eq(40, cols, 'columns')
      a.eq(5, rows, 'rows')
    end
  )
end)

QA.describe('images: file URLs and failure messages', function()
  QA.before_each(QA.reset_plugin)
  QA.after_each(stop_server)

  QA.check(
    { 'R-287', 'R-225' },
    'A file:// image URL shows a failure box with the reason "fetch failed", even when the file exists',
    function()
      local dir = QA.tmpdir()
      local p = QA.make_png(dir .. '/f.png', 20, 20)
      local r = U.open({ '![fileurl](file://' .. p .. ')' }, { dir = dir })
      local b = only_box(r)
      a.falsy(U.is_placed(b), 'a file URL is not read as a local path')
      a.contains('fetch failed', b.text)
    end
  )

  QA.check(
    'R-287',
    'A file:// image URL is not fetched with a remote request and writes no remote cache entry',
    { severity = 'medium' },
    function()
      local r = U.open({ '![fileurl](file:///nonexistent/q.png)' })
      a.contains('fetch failed', only_box(r).text)
      a.eq({}, remote_files(r.cache), 'remote entries')
    end
  )

  QA.check(
    'R-288',
    'A failed image writes "kitty-markdown: image <alt>: <code>: <message>" to :messages',
    function()
      local before = U.messages()
      U.open({ '![lost photo](not-there.png)' })
      local lines = failure_lines(U.new_messages(before))
      a.eq(1, #lines, 'one failure line; got ' .. vim.inspect(lines))
      a.matches('^kitty%-markdown: image lost photo: [^:%s]+: .+', lines[1])
    end
  )

  QA.check(
    'R-288',
    'A failed image without alt text uses the kind as the title in :messages',
    function()
      local before = U.messages()
      U.open({ '![](no-alt-here.png)' })
      local lines = failure_lines(U.new_messages(before))
      a.eq(1, #lines, 'one failure line; got ' .. vim.inspect(lines))
      a.matches('^kitty%-markdown: image image: [^:%s]+: .+', lines[1])
    end
  )

  QA.check(
    'R-288',
    'The :messages line of a failed local image names its path and the cause',
    function()
      local dir = QA.tmpdir()
      local before = U.messages()
      U.open({ '![p](missing-q7.png)' }, { dir = dir })
      local lines = failure_lines(U.new_messages(before))
      a.eq(1, #lines)
      local msg = lines[1]:match('^kitty%-markdown: image p: [^:%s]+: (.+)$')
      a.not_nil(msg, 'message part of ' .. lines[1])
      a.contains(dir .. '/missing-q7.png', msg, 'the resolved path')
    end
  )

  QA.check(
    'R-288',
    'The :messages line of a failed fetch names the URL and the HTTP status',
    function()
      srv = U.server({})
      local url = srv.url('/absent-z.png')
      local before = U.messages()
      U.open({ '![net](' .. url .. ')' })
      local lines = failure_lines(U.new_messages(before))
      a.eq(1, #lines)
      local msg = lines[1]:match('^kitty%-markdown: image net: [^:%s]+: (.+)$')
      a.not_nil(msg, 'message part of ' .. lines[1])
      a.contains(url, msg, 'the URL')
      a.contains('404', msg, 'the HTTP status')
    end
  )

  QA.check(
    'R-288',
    'The :messages line of an undecodable image names its path and the cause',
    function()
      local dir = QA.tmpdir()
      QA.write(dir .. '/t.tiff', QA.read(U.fixtures .. '/pic.tiff'))
      local before = U.messages()
      U.open({ '![tif](t.tiff)' }, { dir = dir })
      local lines = failure_lines(U.new_messages(before))
      a.eq(1, #lines)
      a.matches('^kitty%-markdown: image tif: [^:%s]+: .+', lines[1])
      a.contains('t.tiff', lines[1], 'the path')
    end
  )
end)

QA.describe('images: fetch retry and remote cache on failure', function()
  QA.before_each(QA.reset_plugin)
  QA.after_each(stop_server)

  local function rerender(r)
    require('kitty-markdown').refresh()
    QA.answer_probe(true)
    vim.wait(200)
    return U.settle(r.pbuf, 1)
  end

  QA.check('R-291', 'A later render fetches a failed remote image again', function()
    srv = U.server({})
    local r = U.open({ '![later](' .. srv.url('/later.png') .. ')' })
    a.contains('fetch failed', only_box(r).text)
    a.eq(1, srv.count('/later.png'), 'hits after the first render')
    srv.routes['/later.png'] = { body = png_bytes(20, 20) }
    QA.wait(4000, function()
      local bs = rerender(r)
      return bs[1] and U.is_placed(bs[1])
    end)
    a.ge(2, srv.count('/later.png'), 'hits after a second render')
    a.truthy(U.is_placed(U.boxes(r.pbuf)[1]), 'placed once the server has the image')
  end)

  QA.check('R-291', 'A later render fetches a timed-out remote image again', function()
    srv = U.server({ ['/slow.png'] = { stall = true } })
    local r = U.open(
      { '![slow](' .. srv.url('/slow.png') .. ')' },
      { remote = { timeout_ms = 300 }, ms = 4000 }
    )
    a.contains('timeout', only_box(r).text)
    srv.routes['/slow.png'] = { body = png_bytes(20, 20) }
    rerender(r)
    QA.wait(3000, function()
      local bs = U.boxes(r.pbuf)
      return bs[1] and U.is_placed(bs[1])
    end)
    a.ge(2, srv.count('/slow.png'), 'hits')
    a.truthy(U.is_placed(U.boxes(r.pbuf)[1]), 'placed after the retry')
  end)

  QA.check('R-292', 'A fetch that gets an HTTP error stores nothing in the remote cache', function()
    srv = U.server({ ['/e.png'] = { status = 500, body = 'server error body' } })
    local r = U.open({ '![e](' .. srv.url('/e.png') .. ')' })
    a.contains('fetch failed', only_box(r).text)
    a.eq({}, remote_files(r.cache), 'remote entries')
  end)

  QA.check(
    'R-292',
    'A fetch over the size cap stores nothing in the remote cache, with or without a length header',
    function()
      srv = U.server({
        ['/a.png'] = { body = png_bytes(64, 64) },
        ['/b.png'] = { body = png_bytes(64, 64, { 1, 2, 3, 255 }), no_length = true },
      })
      local r = U.open(
        { '![a](' .. srv.url('/a.png') .. ')', '', '![b](' .. srv.url('/b.png') .. ')' },
        { n = 2, remote = { max_bytes = 1000 } }
      )
      a.contains('too large', r.boxes[1].text)
      a.contains('too large', r.boxes[2].text)
      a.eq({}, remote_files(r.cache), 'remote entries')
    end
  )

  QA.check('R-292', 'A fetch that times out stores nothing in the remote cache', function()
    srv = U.server({ ['/t.png'] = { stall = true } })
    local r = U.open(
      { '![t](' .. srv.url('/t.png') .. ')' },
      { remote = { timeout_ms = 300 }, ms = 4000 }
    )
    a.contains('timeout', only_box(r).text)
    a.eq({}, remote_files(r.cache), 'remote entries')
  end)

  QA.check(
    'R-292',
    'A failed fetch through the helper protocol leaves no remote entry for the URL',
    function()
      srv = U.server({ ['/big'] = { body = ('z'):rep(5000), no_length = true } })
      local sess = QA.helper_session()
      local ok, err = pcall(function()
        local r404 = sess:request(
          'fetch',
          { url = srv.url('/nope'), timeout_ms = 2000, max_bytes = 100000 },
          5000
        )
        a.not_nil(r404 and r404.error, '404 is an error')
        local rbig = sess:request(
          'fetch',
          { url = srv.url('/big'), timeout_ms = 2000, max_bytes = 1000 },
          5000
        )
        a.not_nil(rbig and rbig.error, 'oversize is an error')
        a.eq({}, U.files(sess.cache_dir .. '/remote'), 'remote entries')
      end)
      sess:close()
      if not ok then
        error(err, 0)
      end
    end
  )
end)

QA.describe('images: remote size cap default', function()
  QA.before_each(QA.reset_plugin)
  QA.after_each(stop_server)

  -- A body that is not an image: "unsupported format" means it passed
  -- the size cap, "too large" means it did not.
  QA.check(
    'R-290',
    'With no size cap option, a remote body of exactly 20 MiB is under the cap',
    function()
      srv = U.server({ ['/edge'] = { body = ('a'):rep(20971520) } })
      local r = U.open({ '![edge](' .. srv.url('/edge') .. ')' }, { ms = 15000 })
      local b = only_box(r)
      a.no_match('too large', b.text)
    end
  )

  QA.check(
    'R-290',
    'With no size cap option, a remote body one byte over 20 MiB is "too large"',
    function()
      srv = U.server({ ['/over'] = { body = ('a'):rep(20971521) } })
      local r = U.open({ '![over](' .. srv.url('/over') .. ')' }, { ms = 15000 })
      a.contains('too large', only_box(r).text)
    end
  )

  QA.check({ 'R-290', 'R-083' }, 'The remote_images.max_bytes option sets the size cap', function()
    local body = png_bytes(40, 40)
    srv = U.server({ ['/c.png'] = { body = body } })
    local r = U.open(
      { '![c](' .. srv.url('/c.png') .. ')' },
      { remote = { max_bytes = #body - 1 } }
    )
    a.contains('too large', only_box(r).text)
  end)
end)

QA.describe('images: block key (image side)', function()
  local function key_of(text, dir)
    local m = QA.layout(text, { width = 80, dir = dir })
    QA.need(m.blocks[1] and m.blocks[1].key, 'a key field on the block descriptor')
    return m.blocks[1].key
  end

  QA.check(
    'R-204',
    'Two images with the same source and different <img> attributes have different block keys',
    function()
      local k1 = key_of({ '<img src="p.png" width="50">' }, '/d')
      local k2 = key_of({ '<img src="p.png" width="60">' }, '/d')
      local k3 = key_of({ '<img src="p.png">' }, '/d')
      a.ne(k1, k2, 'width 50 versus 60')
      a.ne(k1, k3, 'width versus none')
    end
  )

  QA.check(
    'R-204',
    'The same relative image path under two base directories gives two block keys',
    function()
      a.ne(key_of({ '![x](p.png)' }, '/one'), key_of({ '![x](p.png)' }, '/two'))
    end
  )

  QA.check(
    'R-204',
    'A relative and an absolute path to the same file give the same block key',
    { severity = 'medium' },
    function()
      a.eq(key_of({ '![x](p.png)' }, '/one'), key_of({ '![x](/one/p.png)' }, '/two'))
    end
  )
end)

QA.describe('images: box glyphs', function()
  local LOADING = vim.fn.nr2char(0xF051F)
  local FAILURE = vim.fn.nr2char(0xF0156)

  QA.before_each(QA.reset_plugin)
  QA.after_each(stop_server)

  QA.check(
    'R-274',
    'A pixel block that is still loading shows the loading box glyph from the Icons definition',
    function()
      srv = U.server({ ['/wait.png'] = { stall = true } })
      local r = U.open(
        { '![wait](' .. srv.url('/wait.png') .. ')' },
        { wait = false, remote = { timeout_ms = 20000 } }
      )
      local box
      QA.wait(3000, function()
        local bs = U.boxes(r.pbuf)
        if bs[1] and bs[1].text:find('loading', 1, true) then
          box = bs[1]
          return true
        end
        return false
      end)
      a.not_nil(box, 'a loading box')
      a.contains(LOADING, box.text, 'loading glyph')
      a.not_contains(FAILURE, box.text, 'no failure glyph in a loading box')
    end
  )

  QA.check(
    'R-274',
    'A failed pixel block shows the failure box glyph from the Icons definition',
    function()
      local dir = QA.tmpdir()
      QA.write(dir .. '/g.tiff', QA.read(U.fixtures .. '/pic.tiff'))
      local r = U.open({ '![glyph](g.tiff)' }, { dir = dir })
      local b = only_box(r)
      a.contains('unsupported format', b.text)
      a.contains(FAILURE, b.text, 'failure glyph')
      a.not_contains(LOADING, b.text, 'no loading glyph in a failure box')
    end
  )
end)
