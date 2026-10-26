-- The cache: root, subdirectories, key strings, LRU garbage collection,
-- the keep set, and mtime updates. Helper-level and end to end.
local QA = require('qa')
local U = require('images_util')
local a = QA.assert

local F = U.fixtures
local MERMAID_VERSION = '12.2.0'

local function key(...)
  return vim.fn.sha256(table.concat({ ... }, '|'))
end

-- The file in `dir` whose name without extension is `k`, or nil.
local function entry(dir, k)
  for _, f in ipairs(U.files(dir)) do
    if U.stem(f) == k then
      return dir .. '/' .. f
    end
  end
end

local function names(dir)
  return vim.inspect(U.files(dir))
end

local function filler(path, n, mtime)
  QA.write(path, ('x'):rep(n))
  U.set_mtime(path, mtime)
  return path
end

local srv

QA.describe('cache: helper keys and gc', function()
  local sess
  QA.before_each(function()
    sess = QA.helper_session()
    sess:request('probe', { plugin_version = '0', cache_dir = sess.cache_dir }, 5000)
  end)
  QA.after_each(function()
    if sess then
      sess:close()
    end
    sess = nil
    if srv then
      srv.stop()
      srv = nil
    end
  end)

  QA.check(
    'R-149',
    'The helper keeps the cache in the subdirectories images, remote, and mermaid',
    function()
      local src = QA.make_png(QA.tmpdir() .. '/s.png', 10, 10)
      local ri = sess:request('image.prepare', { path = src, width_px = 100 }, 10000)
      a.is_nil(ri.error)
      a.eq(sess.cache_dir .. '/images', vim.fs.dirname(ri.result.path))
      local r = sess:request('mermaid.render', {
        source = 'flowchart LR\n  A --> B',
        width_px = 300,
        theme = 'default',
        background = 'transparent',
      }, 20000)
      a.is_nil(r and r.error, 'mermaid error')
      a.eq(sess.cache_dir .. '/mermaid', vim.fs.dirname(r.result.path))
    end
  )

  QA.check(
    { 'R-212', 'R-150' },
    'An image entry is named by the SHA-256 of "<content hash>|<width px>|0|0" when no limit is given',
    function()
      local src = F .. '/photo.jpg'
      local r = sess:request('image.prepare', { path = src, width_px = 300 }, 10000)
      a.is_nil(r.error)
      a.eq(key(U.sha_file(src), '300', '0', '0'), U.stem(r.result.path))
    end
  )

  QA.check(
    'R-212',
    'The height limit is the third field and the width limit the fourth field of the image key string',
    function()
      local src = F .. '/photo.jpg'
      local rw =
        sess:request('image.prepare', { path = src, width_px = 300, max_width_px = 120 }, 10000)
      local rh =
        sess:request('image.prepare', { path = src, width_px = 300, max_height_px = 70 }, 10000)
      local rb = sess:request(
        'image.prepare',
        { path = src, width_px = 300, max_height_px = 70, max_width_px = 120 },
        10000
      )
      local h = U.sha_file(src)
      a.eq(key(h, '300', '0', '120'), U.stem(rw.result.path), 'width limit only')
      a.eq(key(h, '300', '70', '0'), U.stem(rh.result.path), 'height limit only')
      a.eq(key(h, '300', '70', '120'), U.stem(rb.result.path), 'both limits')
    end
  )

  QA.check('R-150', 'The image key depends on the content, not on the file path', function()
    local d = QA.tmpdir()
    local p1 = QA.make_png(d .. '/one.png', 30, 30, { 1, 2, 3, 255 })
    local p2 = QA.write(d .. '/two.png', QA.read(p1))
    local p3 = QA.make_png(d .. '/three.png', 30, 30, { 9, 9, 9, 255 })
    local r1 = sess:request('image.prepare', { path = p1, width_px = 300 }, 10000)
    local r2 = sess:request('image.prepare', { path = p2, width_px = 300 }, 10000)
    local r3 = sess:request('image.prepare', { path = p3, width_px = 300 }, 10000)
    a.eq(r1.result.path, r2.result.path, 'same bytes, same entry')
    a.truthy(r2.result.cached, 'the second prepare is a cache hit')
    a.ne(r1.result.path, r3.result.path, 'other bytes, other entry')
  end)

  QA.check('R-150', 'The image key changes when the target width changes', function()
    local src = F .. '/photo.jpg'
    local r1 = sess:request('image.prepare', { path = src, width_px = 300 }, 10000)
    local r2 = sess:request('image.prepare', { path = src, width_px = 200 }, 10000)
    a.ne(r1.result.path, r2.result.path)
  end)

  QA.check(
    'R-212',
    'An SVG entry uses the same image key string over the SVG bytes',
    { severity = 'medium' },
    function()
      local src = F .. '/drawing.svg'
      local r = sess:request('svg.rasterize', { path = src, width_px = 80 }, 10000)
      a.is_nil(r.error)
      a.eq(key(U.sha_file(src), '80', '0', '0'), U.stem(r.result.path))
    end
  )

  QA.check(
    { 'R-213', 'R-150' },
    'A mermaid entry is named by the SHA-256 of "<source hash>|<width px>|<theme>|<mermaid version>"',
    function()
      local src = 'flowchart LR\n  A --> B'
      local r = sess:request(
        'mermaid.render',
        { source = src, width_px = 400, theme = 'default', background = 'transparent' },
        20000
      )
      a.is_nil(r.error)
      a.eq(key(vim.fn.sha256(src), '400', 'default', MERMAID_VERSION), U.stem(r.result.path))
      local rd = sess:request(
        'mermaid.render',
        { source = src, width_px = 400, theme = 'dark', background = 'transparent' },
        20000
      )
      a.eq(
        key(vim.fn.sha256(src), '400', 'dark', MERMAID_VERSION),
        U.stem(rd.result.path),
        'theme field'
      )
    end
  )

  QA.check({ 'R-150', 'R-080' }, 'A remote entry is named by the SHA-256 of its URL', function()
    local body = QA.read(QA.make_png(QA.tmpdir() .. '/r.png', 12, 12))
    srv = U.server({ ['/img.png'] = { body = body } })
    local url = srv.url('/img.png')
    local r = sess:request('fetch', { url = url, timeout_ms = 3000, max_bytes = 1000000 }, 10000)
    a.is_nil(r.error, 'fetch error')
    a.eq(sess.cache_dir .. '/remote', vim.fs.dirname(r.result.path), 'directory')
    a.eq(vim.fn.sha256(url), U.stem(r.result.path), 'name')
  end)

  QA.check(
    'R-081',
    'A second fetch of the same URL is a cache hit and does not reach the server',
    function()
      local body = QA.read(QA.make_png(QA.tmpdir() .. '/r.png', 12, 12))
      srv = U.server({ ['/again.png'] = { body = body } })
      local url = srv.url('/again.png')
      local p = { url = url, timeout_ms = 3000, max_bytes = 1000000 }
      local r1 = sess:request('fetch', p, 10000)
      local r2 = sess:request('fetch', p, 10000)
      a.is_nil(r1.error)
      a.truthy(r2.result and r2.result.cached, 'cached flag')
      a.eq(1, srv.count('/again.png'), 'server hits')
    end
  )

  QA.check(
    'R-151',
    'Garbage collection deletes the least recently used entries until the cache is under the limit',
    function()
      local c = sess.cache_dir
      local now = os.time()
      filler(c .. '/images/' .. key('a') .. '.png', 1000, now - 500)
      filler(c .. '/remote/' .. key('b'), 1000, now - 400)
      filler(c .. '/mermaid/' .. key('c') .. '.png', 1000, now - 300)
      local new1 = filler(c .. '/images/' .. key('d') .. '.png', 1000, now - 200)
      local new2 = filler(c .. '/remote/' .. key('e'), 1000, now - 100)
      local r = sess:request('cache.gc', { max_bytes = 2500, keep = {} }, 10000)
      a.is_nil(r.error, 'gc error')
      a.eq(
        { 'images/' .. key('d') .. '.png', 'remote/' .. key('e') },
        U.files(c),
        'remaining entries'
      )
      a.truthy(U.exists(new1) and U.exists(new2))
    end
  )

  QA.check(
    'R-151',
    'Garbage collection deletes nothing when the cache is under the limit',
    function()
      local c = sess.cache_dir
      local now = os.time()
      filler(c .. '/images/' .. key('a') .. '.png', 1000, now - 500)
      filler(c .. '/remote/' .. key('b'), 1000, now - 400)
      sess:request('cache.gc', { max_bytes = 5000, keep = {} }, 10000)
      a.eq(2, #U.files(c), 'entries: ' .. names(c))
    end
  )

  QA.check(
    'R-152',
    'Garbage collection does not delete an entry in the keep set, even the oldest one',
    function()
      local c = sess.cache_dir
      local now = os.time()
      local old = filler(c .. '/images/' .. key('a') .. '.png', 1000, now - 500)
      filler(c .. '/remote/' .. key('b'), 1000, now - 400)
      filler(c .. '/mermaid/' .. key('c') .. '.png', 1000, now - 300)
      filler(c .. '/images/' .. key('d') .. '.png', 1000, now - 200)
      local newest = filler(c .. '/remote/' .. key('e'), 1000, now - 100)
      sess:request('cache.gc', { max_bytes = 2500, keep = { old } }, 10000)
      a.truthy(U.exists(old), 'kept entry')
      a.truthy(U.exists(newest), 'newest entry')
      a.eq(2, #U.files(c), 'entries left: ' .. names(c))
    end
  )

  QA.check(
    'R-152',
    'Garbage collection keeps every entry of the keep set even when the limit cannot be met',
    function()
      local c = sess.cache_dir
      local now = os.time()
      local k1 = filler(c .. '/images/' .. key('a') .. '.png', 1000, now - 500)
      local k2 = filler(c .. '/mermaid/' .. key('b') .. '.png', 1000, now - 400)
      local gone = filler(c .. '/remote/' .. key('c'), 1000, now - 300)
      local r = sess:request('cache.gc', { max_bytes = 1, keep = { k1, k2 } }, 10000)
      a.is_nil(r.error)
      a.truthy(U.exists(k1) and U.exists(k2), 'kept entries')
      a.falsy(U.exists(gone), 'the entry outside the keep set')
    end
  )

  QA.check(
    'R-201',
    'A cache hit on an image entry sets its modification time to the current time',
    function()
      local src = F .. '/photo.jpg'
      local r1 = sess:request('image.prepare', { path = src, width_px = 300 }, 10000)
      local old = os.time() - 100000
      U.set_mtime(r1.result.path, old)
      local r2 = sess:request('image.prepare', { path = src, width_px = 300 }, 10000)
      a.truthy(r2.result.cached, 'a cache hit')
      a.ge(os.time() - 60, U.mtime(r1.result.path), 'mtime after the hit')
    end
  )

  QA.check(
    'R-201',
    'A cache hit on a mermaid entry sets its modification time to the current time',
    function()
      local p = {
        source = 'flowchart TD\n  X --> Y',
        width_px = 300,
        theme = 'default',
        background = 'transparent',
      }
      local r1 = sess:request('mermaid.render', p, 20000)
      a.is_nil(r1.error)
      U.set_mtime(r1.result.path, os.time() - 100000)
      local r2 = sess:request('mermaid.render', p, 20000)
      a.truthy(r2.result.cached, 'a cache hit')
      a.ge(os.time() - 60, U.mtime(r1.result.path))
    end
  )

  QA.check(
    'R-201',
    'A cache hit on a remote entry sets its modification time to the current time',
    function()
      srv =
        U.server({ ['/t.png'] = { body = QA.read(QA.make_png(QA.tmpdir() .. '/t.png', 8, 8)) } })
      local p = { url = srv.url('/t.png'), timeout_ms = 3000, max_bytes = 1000000 }
      local r1 = sess:request('fetch', p, 10000)
      a.is_nil(r1.error)
      U.set_mtime(r1.result.path, os.time() - 100000)
      sess:request('fetch', p, 10000)
      a.ge(os.time() - 60, U.mtime(r1.result.path))
    end
  )

  QA.check(
    { 'R-201', 'R-151' },
    'A recently used entry survives garbage collection over an entry written later but not used since',
    function()
      local c = sess.cache_dir
      local src = F .. '/photo.jpg'
      local r1 = sess:request('image.prepare', { path = src, width_px = 300 }, 10000)
      local now = os.time()
      U.set_mtime(r1.result.path, now - 1000)
      local other = filler(c .. '/remote/' .. key('later'), r1.result.bytes, now - 500)
      sess:request('image.prepare', { path = src, width_px = 300 }, 10000)
      sess:request('cache.gc', { max_bytes = r1.result.bytes + 10, keep = {} }, 10000)
      a.truthy(U.exists(r1.result.path), 'the used entry')
      a.falsy(U.exists(other), 'the stale entry')
    end
  )
end)

QA.describe('cache: plugin end to end', function()
  QA.before_each(QA.reset_plugin)
  QA.after_each(function()
    if srv then
      srv.stop()
      srv = nil
    end
  end)

  QA.check('R-148', 'The cache directory option names the cache root', function()
    local dir = QA.tmpdir()
    QA.make_png(dir .. '/p.png', 20, 20)
    local cache = QA.tmpdir() .. '/custom-root'
    local r = U.open({ '![p](p.png)' }, { dir = dir, cache_dir = cache })
    a.truthy(U.is_placed(r.boxes[1]))
    a.ge(1, #U.files(cache .. '/images'), 'entries under the configured root: ' .. names(cache))
  end)

  QA.check(
    'R-148',
    'Without the cache directory option, the cache is under stdpath("cache")/kitty-markdown',
    function()
      local xdg = QA.tmpdir()
      local script = QA.tmpdir() .. '/child.lua'
      QA.write(
        script,
        table.concat({
          ('package.path = %q .. package.path'):format(QA.held_out .. '/lib/?.lua;'),
          "local QA = require('qa')",
          "local U = require('images_util')",
          'local dir = QA.tmpdir()',
          "QA.make_png(dir .. '/p.png', 20, 20)",
          "local ok, r = pcall(U.open, { '![p](p.png)' }, { dir = dir, cache_dir = false })",
          "io.stdout:write(vim.fn.stdpath('cache') .. '\\n')",
          "io.stdout:write((ok and r.boxes[1] and U.is_placed(r.boxes[1])) and 'PLACED\\n' or ('NOT ' .. tostring(r) .. '\\n'))",
          "pcall(require('kitty-markdown.helper').reset)",
          'os.exit(0)',
        }, '\n')
      )
      local res = QA.sh(
        { 'nvim', '--clean', '-l', script },
        { env = { XDG_CACHE_HOME = xdg, QA_HELPER_BIN = QA.helper_bin() }, timeout = 20000 }
      )
      QA.need(
        res.stdout and res.stdout:find(xdg, 1, true),
        'a child nvim that honors XDG_CACHE_HOME: ' .. tostring(res.stdout) .. tostring(res.stderr)
      )
      a.contains('PLACED', res.stdout, 'child run')
      local root = vim.fs.normalize(xdg .. '/nvim/kitty-markdown')
      a.ge(1, #U.files(root .. '/images'), 'entries under the default root: ' .. names(xdg))
    end
  )

  QA.check(
    'R-149',
    'Local images, remote fetches, and mermaid renders land in images, remote, and mermaid under the root',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/p.png', 20, 20)
      srv =
        U.server({ ['/r.png'] = { body = QA.read(QA.make_png(QA.tmpdir() .. '/r.png', 16, 16)) } })
      local lines = {
        '![loc](p.png)',
        '',
        '![rem](' .. srv.url('/r.png') .. ')',
        '',
        '```mermaid',
        'flowchart LR',
        '  A --> B',
        '```',
      }
      local r = U.open(lines, { dir = dir, n = 3, ms = 15000 })
      for i, b in ipairs(r.boxes) do
        a.truthy(U.is_placed(b), 'block ' .. i .. ': ' .. b.text)
      end
      local files = U.files(r.cache)
      local by = { images = 0, remote = 0, mermaid = 0 }
      for _, f in ipairs(files) do
        local top = f:match('^([^/]+)/')
        a.truthy(top and by[top], 'entry outside the three subdirectories: ' .. f)
        by[top] = by[top] + 1
      end
      a.ge(1, by.remote, 'remote entries')
      a.ge(1, by.mermaid, 'mermaid entries')
      a.ge(2, by.images, 'image entries for the local and the fetched image')
    end
  )

  QA.check(
    'R-212',
    'The plugin names a prepared image by the key string with the target width in pixels',
    function()
      local dir = QA.tmpdir()
      local src = QA.make_png(dir .. '/p.png', 30, 30)
      local r = U.open({ '![p](p.png)' }, { dir = dir, cell = { 10, 20 } })
      a.truthy(U.is_placed(r.boxes[1]))
      local width_px = U.text_width(r.win) * 10
      local k = key(U.sha_file(src), tostring(width_px), '0', '0')
      a.not_nil(entry(r.cache .. '/images', k), 'entry ' .. k .. ' in ' .. names(r.cache))
    end
  )

  QA.check(
    'R-212',
    'The plugin puts an <img> width or height into the width-limit and height-limit fields of the key string',
    function()
      local dir = QA.tmpdir()
      local src = QA.make_png(dir .. '/p.png', 200, 100)
      local r = U.open(
        { '<img src="p.png" width="50">', '', '<img src="p.png" height="40">' },
        { dir = dir, cell = { 10, 20 }, n = 2 }
      )
      local width_px = tostring(U.text_width(r.win) * 10)
      local h = U.sha_file(src)
      a.not_nil(
        entry(r.cache .. '/images', key(h, width_px, '0', '50')),
        'width limit entry in ' .. names(r.cache)
      )
      a.not_nil(
        entry(r.cache .. '/images', key(h, width_px, '40', '0')),
        'height limit entry in ' .. names(r.cache)
      )
    end
  )

  QA.check(
    'R-213',
    'The plugin names a mermaid render by the key string with the target width, the mermaid theme, and the mermaid version',
    function()
      local body = { 'flowchart LR', '  Q --> R' }
      local lines =
        vim.list_extend({ '```mermaid' }, vim.list_extend(vim.deepcopy(body), { '```' }))
      local r = U.open(
        lines,
        { cell = { 10, 20 }, config = { mermaid = { theme = 'forest' } }, ms = 15000 }
      )
      a.truthy(U.is_placed(r.boxes[1]), r.boxes[1].text)
      local width_px = tostring(U.text_width(r.win) * 10)
      local src = table.concat(body, '\n')
      local k1 = key(vim.fn.sha256(src), width_px, 'forest', MERMAID_VERSION)
      local k2 = key(vim.fn.sha256(src .. '\n'), width_px, 'forest', MERMAID_VERSION)
      local found = entry(r.cache .. '/mermaid', k1) or entry(r.cache .. '/mermaid', k2)
      a.not_nil(found, 'mermaid entry in ' .. names(r.cache))
    end
  )

  QA.check(
    { 'R-151', 'R-152' },
    'When the cache is over its limit after a render, old entries are deleted and the entries the preview shows are kept',
    function()
      local dir = QA.tmpdir()
      local cache = QA.tmpdir()
      local now = os.time()
      local junk = {}
      for i = 1, 3 do
        junk[i] = filler(cache .. '/images/' .. key('junk' .. i) .. '.png', 20000, now - 1000 * i)
      end
      QA.make_png(dir .. '/p1.png', 20, 20, { 1, 1, 1, 255 })
      QA.make_png(dir .. '/p2.png', 20, 20, { 2, 2, 2, 255 })
      local r = U.open(
        { '![a](p1.png)', '', '![b](p2.png)' },
        { dir = dir, cache_dir = cache, n = 2, config = { cache = { max_bytes = 1 } } }
      )
      a.truthy(U.is_placed(r.boxes[1]) and U.is_placed(r.boxes[2]), 'both placed')
      QA.wait(3000, function()
        return not (U.exists(junk[1]) or U.exists(junk[2]) or U.exists(junk[3]))
      end)
      for i = 1, 3 do
        a.falsy(U.exists(junk[i]), 'old entry ' .. i)
      end
      local left = U.files(cache .. '/images')
      a.eq(2, #left, 'the two images in use stay: ' .. vim.inspect(left))
    end
  )

  QA.check(
    'R-202',
    'One render batch over the size limit sends exactly one garbage collection request',
    function()
      local log = QA.tmpdir() .. '/requests.log'
      local wrap = U.logging_helper(log)
      local dir = QA.tmpdir()
      local cache = QA.tmpdir()
      filler(cache .. '/images/' .. key('junk') .. '.png', 50000, os.time() - 1000)
      local lines = {}
      for i = 1, 3 do
        QA.make_png(dir .. '/p' .. i .. '.png', 20, 20, { i, i, i, 255 })
        vim.list_extend(lines, { '![p' .. i .. '](p' .. i .. '.png)', '' })
      end
      local r = U.open(lines, {
        dir = dir,
        cache_dir = cache,
        n = 3,
        helper = wrap,
        config = { cache = { max_bytes = 10000 } },
      })
      for _, b in ipairs(r.boxes) do
        a.truthy(U.is_placed(b), b.text)
      end
      vim.wait(800)
      a.eq(1, U.count_lines(log, '"cache.gc"'), 'cache.gc requests')
    end
  )

  QA.check(
    'R-202',
    'A render that leaves the cache under the size limit sends no garbage collection request',
    function()
      local log = QA.tmpdir() .. '/requests.log'
      local wrap = U.logging_helper(log)
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/p.png', 20, 20)
      local r = U.open({ '![p](p.png)' }, { dir = dir, helper = wrap })
      a.truthy(U.is_placed(r.boxes[1]))
      vim.wait(500)
      QA.need(U.count_lines(log, 'image.prepare') >= 1, 'the request log of the wrapped helper')
      a.eq(0, U.count_lines(log, '"cache.gc"'), 'cache.gc requests')
    end
  )

  QA.check(
    'R-152',
    'A garbage collection request names the cache entries that the open preview shows',
    function()
      local log = QA.tmpdir() .. '/requests.log'
      local wrap = U.logging_helper(log)
      local dir = QA.tmpdir()
      local cache = QA.tmpdir()
      filler(cache .. '/images/' .. key('junk') .. '.png', 50000, os.time() - 1000)
      local src = QA.make_png(dir .. '/p.png', 20, 20)
      local r = U.open(
        { '![p](p.png)' },
        { dir = dir, cache_dir = cache, helper = wrap, config = { cache = { max_bytes = 10000 } } }
      )
      a.truthy(U.is_placed(r.boxes[1]))
      vim.wait(800)
      local gc
      for line in (QA.read(log) or ''):gmatch('[^\n]+') do
        if line:find('"cache.gc"', 1, true) then
          gc = vim.json.decode(line)
        end
      end
      a.not_nil(gc, 'a cache.gc request')
      local keep = (gc.params or {}).keep or {}
      local k = key(U.sha_file(src), tostring(U.text_width(r.win) * 10), '0', '0')
      local found = false
      for _, p in ipairs(keep) do
        if U.stem(p) == k then
          found = true
        end
      end
      a.truthy(found, 'the shown image entry in keep: ' .. vim.inspect(keep))
    end
  )
end)
