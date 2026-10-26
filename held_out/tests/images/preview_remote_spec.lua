-- End to end: remote images served by a local HTTP server.
local QA = require('qa')
local U = require('images_util')
local a = QA.assert

local srv

local function png_bytes(w, h, rgba)
  return QA.read(QA.make_png(QA.tmpdir() .. '/src.png', w, h, rgba))
end

local function start(routes)
  srv = U.server(routes)
  return srv
end

local function only_box(r)
  a.eq(1, #r.boxes, 'one pixel block')
  return r.boxes[1]
end

local function find_remote(cache, url)
  local key = vim.fn.sha256(url)
  for _, f in ipairs(U.files(cache .. '/remote')) do
    if U.stem(f) == key then
      return cache .. '/remote/' .. f
    end
  end
end

QA.describe('images: remote fetch', function()
  QA.before_each(QA.reset_plugin)
  QA.after_each(function()
    if srv then
      srv.stop()
      srv = nil
    end
  end)

  QA.check(
    'R-079',
    'With remote images on, an http image is fetched on the first render and placed',
    function()
      start({ ['/pic.png'] = { body = png_bytes(40, 20), ctype = 'image/png' } })
      local r = U.open(
        { '![remote](' .. srv.url('/pic.png') .. ')' },
        { remote = { enabled = true } }
      )
      a.truthy(U.is_placed(only_box(r)), 'placed: ' .. r.boxes[1].text)
      a.eq(1, srv.count('/pic.png'), 'server hits')
    end
  )

  QA.check(
    'R-079',
    'A remote image is fetched even when the server sends no content type and no length',
    { severity = 'medium' },
    function()
      start({ ['/raw'] = { body = png_bytes(30, 30), no_length = true } })
      local r = U.open({ '![raw](' .. srv.url('/raw') .. ')' })
      a.truthy(U.is_placed(only_box(r)), 'placed: ' .. r.boxes[1].text)
    end
  )

  QA.check(
    { 'R-080', 'R-150' },
    'A fetched image is stored in the remote cache under the SHA-256 of its URL',
    function()
      local body = png_bytes(20, 20)
      start({ ['/k.png'] = { body = body } })
      local url = srv.url('/k.png')
      local r = U.open({ '![k](' .. url .. ')' })
      a.truthy(U.is_placed(only_box(r)))
      local p = find_remote(r.cache, url)
      a.not_nil(p, 'an entry named by the URL hash; files: ' .. vim.inspect(U.files(r.cache)))
      a.eq(body, QA.read(p), 'the stored bytes are the fetched bytes')
    end
  )

  QA.check('R-081', 'A cached remote image is not fetched again on a later render', function()
    start({ ['/c.png'] = { body = png_bytes(20, 20) } })
    local r = U.open({ '![c](' .. srv.url('/c.png') .. ')' })
    a.truthy(U.is_placed(only_box(r)))
    vim.api.nvim_buf_set_lines(r.src, -1, -1, false, { '', 'more text' })
    require('kitty-markdown').refresh()
    QA.answer_probe(true)
    vim.wait(500)
    U.settle(r.pbuf, 1)
    a.eq(1, srv.count('/c.png'), 'server hits after a second render')
  end)

  QA.check(
    'R-081',
    'A cached remote image is not fetched again after the helper restarts',
    function()
      start({ ['/c2.png'] = { body = png_bytes(20, 20) } })
      local url = srv.url('/c2.png')
      local cache = QA.tmpdir()
      local r = U.open({ '![c](' .. url .. ')' }, { cache_dir = cache })
      a.truthy(U.is_placed(only_box(r)))
      QA.reset_plugin()
      for _, b in ipairs(vim.api.nvim_list_bufs()) do
        pcall(vim.api.nvim_buf_delete, b, { force = true })
      end
      local r2 = U.open({ '![c](' .. url .. ')' }, { cache_dir = cache, name = 'again.md' })
      a.truthy(U.is_placed(only_box(r2)), 'placed again')
      a.eq(1, srv.count('/c2.png'), 'server hits')
    end
  )

  QA.check(
    'R-081',
    'A cached remote image is shown from the cache when the server is gone',
    function()
      start({ ['/off.png'] = { body = png_bytes(20, 20) } })
      local url = srv.url('/off.png')
      local cache = QA.tmpdir()
      a.truthy(U.is_placed(only_box(U.open({ '![off](' .. url .. ')' }, { cache_dir = cache }))))
      srv.stop()
      srv = nil
      QA.reset_plugin()
      for _, b in ipairs(vim.api.nvim_list_bufs()) do
        pcall(vim.api.nvim_buf_delete, b, { force = true })
      end
      local r2 = U.open({ '![off](' .. url .. ')' }, { cache_dir = cache, name = 'offline.md' })
      local b = only_box(r2)
      a.truthy(U.is_placed(b), 'placed from the cache: ' .. b.text)
    end
  )

  QA.check(
    'R-082',
    'A remote fetch that exceeds the configured timeout shows a failure box with the reason "timeout"',
    function()
      start({ ['/slow.png'] = { stall = true } })
      local t0 = vim.uv.hrtime()
      local r = U.open(
        { '![slow](' .. srv.url('/slow.png') .. ')' },
        { remote = { timeout_ms = 400 }, ms = 6000 }
      )
      local b = only_box(r)
      a.contains('timeout', b.text)
      a.no_match('fetch failed', b.text)
      a.le(5000, (vim.uv.hrtime() - t0) / 1e6, 'ms until the failure box, with a 400 ms timeout')
    end
  )

  QA.check(
    'R-083',
    'A remote image larger than the configured size cap shows a failure box with the reason "too large"',
    function()
      start({ ['/big.png'] = { body = png_bytes(64, 64) } })
      local r = U.open(
        { '![big](' .. srv.url('/big.png') .. ')' },
        { remote = { max_bytes = 1000 } }
      )
      local b = only_box(r)
      a.contains('too large', b.text)
    end
  )

  QA.check(
    'R-083',
    'A remote image larger than the size cap is "too large" also when the server sends no length',
    function()
      start({ ['/bignolen'] = { body = png_bytes(64, 64), no_length = true } })
      local r = U.open(
        { '![big](' .. srv.url('/bignolen') .. ')' },
        { remote = { max_bytes = 1000 } }
      )
      a.contains('too large', only_box(r).text)
    end
  )

  QA.check('R-083', 'A remote image under the size cap is placed', function()
    local body = png_bytes(16, 16)
    start({ ['/ok.png'] = { body = body } })
    local r = U.open(
      { '![ok](' .. srv.url('/ok.png') .. ')' },
      { remote = { max_bytes = #body + 10 } }
    )
    a.truthy(U.is_placed(only_box(r)))
  end)

  QA.check(
    { 'R-084', 'R-173' },
    'An HTTP error status shows a failure box with the alt text and the reason "fetch failed"',
    function()
      start({})
      local r = U.open({ '![lost remote pic](' .. srv.url('/missing.png') .. ')' })
      local b = only_box(r)
      a.contains('lost remote pic', b.text)
      a.contains('fetch failed', b.text)
    end
  )

  QA.check('R-173', 'A server error status shows "fetch failed"', function()
    start({ ['/err.png'] = { status = 500, body = 'boom' } })
    local r = U.open({ '![e](' .. srv.url('/err.png') .. ')' })
    a.contains('fetch failed', only_box(r).text)
  end)

  QA.check(
    { 'R-084', 'R-173' },
    'A refused connection shows a failure box with the alt text and "fetch failed"',
    function()
      local s = U.server({})
      local url = s.url('/x.png')
      s.stop()
      local r = U.open({ '![refused pic](' .. url .. ')' })
      local b = only_box(r)
      a.contains('refused pic', b.text)
      a.contains('fetch failed', b.text)
    end
  )

  QA.check(
    { 'R-089', 'R-173' },
    'A fetched body that is not an image shows "unsupported format", not a fetch failure',
    { severity = 'medium' },
    function()
      start({ ['/page.png'] = { body = '<html>not an image</html>', ctype = 'text/html' } })
      local r = U.open({ '![page](' .. srv.url('/page.png') .. ')' })
      local b = only_box(r)
      a.falsy(U.is_placed(b))
      a.contains('unsupported format', b.text)
    end
  )

  QA.check(
    'R-225',
    'An image URL with a scheme other than http or https shows a failure box with the reason "fetch failed"',
    function()
      local r = U.open({ '![ftp pic](ftp://127.0.0.1/a.png)' })
      local b = only_box(r)
      a.contains('fetch failed', b.text)
      a.no_match('cannot read file', b.text)
    end
  )

  QA.check(
    'R-225',
    'Every URL scheme other than http or https shows "fetch failed", not only the common ones',
    function()
      local r = U.open({ '![mail](gopher://example.invalid/1/a.png)' })
      a.contains('fetch failed', only_box(r).text)
    end
  )
end)

QA.describe('images: remote images off', function()
  QA.before_each(QA.reset_plugin)
  QA.after_each(function()
    if srv then
      srv.stop()
      srv = nil
    end
  end)

  QA.check(
    'R-085',
    'With remote images off, a remote image shows a failure box with the reason "remote images off" and is not fetched',
    function()
      start({ ['/p.png'] = { body = png_bytes(20, 20) } })
      local r = U.open(
        { '![nofetch](' .. srv.url('/p.png') .. ')' },
        { remote = { enabled = false } }
      )
      local b = only_box(r)
      a.contains('remote images off', b.text)
      a.eq(0, srv.count('/p.png'), 'server hits')
    end
  )

  QA.check(
    'R-085',
    'With remote images off, an https image also shows "remote images off"',
    function()
      local r = U.open({ '![sec](https://127.0.0.1:9/p.png)' }, { remote = { enabled = false } })
      a.contains('remote images off', only_box(r).text)
    end
  )

  QA.check('R-085', 'With remote images off, local images still render', function()
    local dir = QA.tmpdir()
    QA.make_png(dir .. '/l.png', 20, 20)
    local r = U.open(
      { '![local](l.png)', '', '![rem](http://127.0.0.1:9/p.png)' },
      { dir = dir, n = 2, remote = { enabled = false } }
    )
    a.truthy(U.is_placed(r.boxes[1]), 'local placed')
    a.contains('remote images off', r.boxes[2].text)
  end)

  QA.check(
    'R-228',
    'With remote images off, many remote images give no message per block',
    function()
      local lines = {}
      for i = 1, 5 do
        vim.list_extend(lines, { '![offpic' .. i .. '](http://127.0.0.1:9/p' .. i .. '.png)', '' })
      end
      local before = U.messages()
      local r = U.open(lines, { n = 5, remote = { enabled = false } })
      a.eq(5, #r.boxes)
      local new = U.new_messages(before)
      local n = 0
      for _ in new:gmatch('offpic%d') do
        n = n + 1
      end
      a.eq(0, n, 'messages that name a block; got: ' .. new)
      local remote_lines = 0
      for l in new:gmatch('[^\n]+') do
        if l:lower():find('remote', 1, true) then
          remote_lines = remote_lines + 1
        end
      end
      a.le(1, remote_lines, 'lines about remote images')
    end
  )

  QA.check(
    'R-096',
    'A failed remote fetch writes its full error, with the HTTP status, to :messages',
    { severity = 'medium' },
    function()
      start({})
      local url = srv.url('/nothere.png')
      local before = U.messages()
      local r = U.open({ '![gone remote](' .. url .. ')' })
      a.contains('fetch failed', only_box(r).text)
      local new = U.new_messages(before)
      a.matches('404', new, 'the HTTP status is part of the full error')
    end
  )
end)
