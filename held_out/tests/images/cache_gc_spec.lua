-- Cache garbage collection from the plugin: the keep set of R-328, one
-- request per render batch while over the limit (R-329), and the default
-- size limit in MiB.
local QA = require('qa')
local U = require('images_util')
local a = QA.assert

local srv

local function key(...)
  return vim.fn.sha256(table.concat({ ... }, '|'))
end

local function filler(path, n, mtime)
  QA.write(path, ('x'):rep(n))
  U.set_mtime(path, mtime)
  return path
end

-- A file of `n` bytes that uses almost no disk (sparse).
local function sparse(path, n, mtime)
  vim.fn.mkdir(vim.fs.dirname(path), 'p')
  local fd = assert(vim.uv.fs_open(path, 'w', 420))
  assert(vim.uv.fs_ftruncate(fd, n))
  vim.uv.fs_close(fd)
  U.set_mtime(path, mtime)
  return path
end

local function gc_requests(log)
  local out = {}
  for line in (QA.read(log) or ''):gmatch('[^\n]+') do
    if line:find('"cache.gc"', 1, true) then
      table.insert(out, vim.json.decode(line))
    end
  end
  return out
end

local function has_stem(paths, stem)
  for _, p in ipairs(paths) do
    if U.stem(p) == stem then
      return true
    end
  end
  return false
end

QA.describe('cache: keep set and gc batches', function()
  QA.before_each(QA.reset_plugin)
  QA.after_each(function()
    if srv then
      srv.stop()
      srv = nil
    end
  end)

  QA.check(
    'R-328',
    'The keep set of a garbage collection request holds the prepared PNG and the fetched remote file of a shown remote image',
    function()
      local body = QA.read(QA.make_png(QA.tmpdir() .. '/r.png', 24, 24))
      srv = U.server({ ['/k.png'] = { body = body } })
      local url = srv.url('/k.png')
      local log = QA.tmpdir() .. '/requests.log'
      local cache = QA.tmpdir()
      filler(cache .. '/images/' .. key('junk') .. '.png', 50000, os.time() - 1000)
      local r = U.open({ '![k](' .. url .. ')' }, {
        cache_dir = cache,
        helper = U.logging_helper(log),
        config = { cache = { max_bytes = 10000 } },
      })
      a.truthy(U.is_placed(r.boxes[1]), r.boxes[1].text)
      vim.wait(800)
      local gcs = gc_requests(log)
      a.ge(1, #gcs, 'cache.gc requests')
      local keep = (gcs[#gcs].params or {}).keep or {}
      local width_px = tostring(U.text_width(r.win) * 10)
      a.truthy(has_stem(keep, vim.fn.sha256(url)), 'the remote file in keep: ' .. vim.inspect(keep))
      a.truthy(
        has_stem(keep, key(vim.fn.sha256(body), width_px, '0', '0')),
        'the prepared PNG in keep: ' .. vim.inspect(keep)
      )
    end
  )

  QA.check(
    'R-328',
    'Garbage collection after a render keeps the remote file and the prepared PNG of a shown remote image',
    function()
      local body = QA.read(QA.make_png(QA.tmpdir() .. '/r.png', 24, 24))
      srv = U.server({ ['/s.png'] = { body = body } })
      local url = srv.url('/s.png')
      local cache = QA.tmpdir()
      local junk = filler(cache .. '/mermaid/' .. key('junk') .. '.png', 30000, os.time() - 1000)
      local r = U.open({ '![s](' .. url .. ')' }, {
        cache_dir = cache,
        config = { cache = { max_bytes = 1 } },
      })
      a.truthy(U.is_placed(r.boxes[1]))
      QA.wait(3000, function()
        return not U.exists(junk)
      end)
      a.falsy(U.exists(junk), 'the unused entry is deleted')
      a.truthy(U.exists(cache .. '/remote/' .. vim.fn.sha256(url)), 'remote file kept')
      a.eq(1, #U.files(cache .. '/images'), 'prepared PNG kept')
    end
  )

  QA.check('R-328', 'The keep set holds one prepared PNG per shown local image', function()
    local dir = QA.tmpdir()
    local log = QA.tmpdir() .. '/requests.log'
    local cache = QA.tmpdir()
    filler(cache .. '/images/' .. key('junk') .. '.png', 50000, os.time() - 1000)
    local a1 = QA.make_png(dir .. '/a.png', 20, 20, { 5, 5, 5, 255 })
    local b1 = QA.make_png(dir .. '/b.png', 20, 20, { 6, 6, 6, 255 })
    local r = U.open({ '![a](a.png)', '', '![b](b.png)' }, {
      dir = dir,
      n = 2,
      cache_dir = cache,
      helper = U.logging_helper(log),
      config = { cache = { max_bytes = 10000 } },
    })
    vim.wait(800)
    local gcs = gc_requests(log)
    a.ge(1, #gcs, 'cache.gc requests')
    local keep = (gcs[#gcs].params or {}).keep or {}
    local w = tostring(U.text_width(r.win) * 10)
    a.truthy(has_stem(keep, key(U.sha_file(a1), w, '0', '0')), 'first image')
    a.truthy(has_stem(keep, key(U.sha_file(b1), w, '0', '0')), 'second image')
  end)

  QA.check(
    'R-329',
    'While the cache stays over its limit, each render batch sends one more garbage collection request',
    function()
      local dir = QA.tmpdir()
      local log = QA.tmpdir() .. '/requests.log'
      local cache = QA.tmpdir()
      for i = 1, 3 do
        QA.make_png(dir .. '/p' .. i .. '.png', 40, 40, { i, 9, 9, 255 })
      end
      local r = U.open({ '![p1](p1.png)', '', '![p2](p2.png)' }, {
        dir = dir,
        n = 2,
        cache_dir = cache,
        helper = U.logging_helper(log),
        -- The images that the preview shows cannot be deleted, so the
        -- cache stays over this limit.
        config = { cache = { max_bytes = 1 } },
      })
      a.truthy(U.is_placed(r.boxes[1]) and U.is_placed(r.boxes[2]))
      vim.wait(600)
      a.eq(1, #gc_requests(log), 'requests after the first render')
      vim.api.nvim_buf_set_lines(r.src, -1, -1, false, { '', '![p3](p3.png)' })
      require('kitty-markdown').refresh()
      QA.answer_probe(true)
      vim.wait(300)
      U.settle(r.pbuf, 3)
      vim.wait(600)
      a.eq(2, #gc_requests(log), 'requests after the second render')
    end
  )

  QA.check(
    'R-151',
    'With no size limit option, a cache just under 500 MiB gets no garbage collection request',
    function()
      local dir = QA.tmpdir()
      local log = QA.tmpdir() .. '/requests.log'
      local cache = QA.tmpdir()
      -- Over 500 MB but under 500 MiB, including the new entry.
      sparse(cache .. '/remote/' .. key('big'), 524288000 - 200000, os.time() - 1000)
      QA.make_png(dir .. '/p.png', 20, 20)
      local r = U.open(
        { '![p](p.png)' },
        { dir = dir, cache_dir = cache, helper = U.logging_helper(log) }
      )
      a.truthy(U.is_placed(r.boxes[1]))
      vim.wait(600)
      QA.need(U.count_lines(log, 'image.prepare') >= 1, 'the request log of the wrapped helper')
      a.eq(0, #gc_requests(log), 'cache.gc requests')
    end
  )

  QA.check(
    'R-151',
    'With no size limit option, a cache over 500 MiB gets one garbage collection request that brings it under the limit',
    function()
      local dir = QA.tmpdir()
      local log = QA.tmpdir() .. '/requests.log'
      local cache = QA.tmpdir()
      local big = sparse(cache .. '/remote/' .. key('big'), 524288001, os.time() - 1000)
      QA.make_png(dir .. '/p.png', 20, 20)
      local r = U.open(
        { '![p](p.png)' },
        { dir = dir, cache_dir = cache, helper = U.logging_helper(log) }
      )
      a.truthy(U.is_placed(r.boxes[1]))
      vim.wait(800)
      local gcs = gc_requests(log)
      a.eq(1, #gcs, 'cache.gc requests')
      a.eq(524288000, (gcs[1].params or {}).max_bytes, 'the limit in the request')
      QA.wait(2000, function()
        return not U.exists(big)
      end)
      a.falsy(U.exists(big), 'the oldest entry is deleted')
    end
  )
end)
