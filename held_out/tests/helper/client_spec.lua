-- The helper client in a running editor: one process, crash reports,
-- restarts, the restart limit, timeouts, and the boxes the preview shows
-- for those failures (R-134, R-136, R-140, R-141, R-194, R-195, R-196,
-- R-198, R-244, R-245).

local QA = require('qa')
local U = require('helper_util')
local a = QA.assert

local IMG_DOC = { '# Doc', '', '![Harbor view at dusk](pic.png)', '', 'After.' }
local TWO_DOC = {
  '# Two',
  '',
  '![First picture alt](one.png)',
  '',
  '```mermaid',
  'flowchart LR',
  '  Alpha --> Beta',
  '```',
  '',
}

local log, restore

local function before()
  U.reset()
  log, restore = QA.capture_notify()
end

local function after()
  restore()
  U.cleanup()
end

-- Drives requests until the fake has started `n` processes.
local function exits(n)
  for _ = 1, n do
    U.request('image.prepare', { path = '/nope.png', width_px = 10 }, 5000)
  end
end

local function warn_count()
  return #U.at_level(log, vim.log.levels.WARN)
end

local function refresh_msgs()
  local out = {}
  for _, m in ipairs(U.at_level(log, vim.log.levels.ERROR)) do
    if m:find(':KittyMarkdown refresh', 1, true) then
      table.insert(out, m)
    end
  end
  return out
end

QA.describe('helper client: one process', function()
  QA.before_each(before)
  QA.after_each(after)

  QA.check('R-136', 'No helper starts for a preview of a document without pixel blocks', function()
    local fake, logp = U.make_fake()
    U.setup_with_path(fake)
    U.open_images_preview({ '# Only text', '', 'A paragraph with *words*.', '', '- a list' })
    QA.wait(800, function()
      return false
    end)
    a.eq(0, U.count(logp, 'start'), 'helper starts')
  end)

  QA.check(
    'R-136',
    'The setup call alone does not start a helper',
    { severity = 'medium' },
    function()
      local fake, logp = U.make_fake()
      U.setup_with_path(fake)
      QA.wait(400, function()
        return false
      end)
      a.eq(0, U.count(logp, 'start'), 'helper starts')
    end
  )

  QA.check('R-136', 'A preview with pixel blocks starts the helper', function()
    local fake, logp = U.make_fake()
    U.setup_with_path(fake)
    U.open_images_preview(IMG_DOC)
    QA.wait(3000, function()
      return U.count(logp, 'start') > 0
    end)
    a.eq(1, U.count(logp, 'start'), 'helper starts')
  end)

  QA.check('R-136', 'Two previews with pixel blocks share one helper process', function()
    local fake, logp = U.make_fake()
    U.setup_with_path(fake)
    U.open_images_preview(IMG_DOC)
    local d2 = QA.tmpdir()
    U.open_doc(TWO_DOC, d2)
    require('kitty-markdown').open('split')
    QA.wait(3000, function()
      return U.count(logp, 'method image.prepare') + U.count(logp, 'method mermaid.render') >= 3
    end)
    a.eq(2, #U.preview_bufs(), 'two previews')
    a.eq(1, U.count(logp, 'start'), 'helper processes')
  end)

  QA.check(
    'R-136',
    'Many requests sent before the helper is ready start only one process',
    function()
      local fake, logp = U.make_fake()
      U.setup_with_path(fake)
      local helper = U.client()
      local n = 0
      for _ = 1, 6 do
        helper.request('cache.gc', { max_bytes = 1 }, function()
          n = n + 1
        end)
      end
      QA.wait(5000, function()
        return n == 6
      end)
      a.eq(6, n, 'answers')
      a.eq(1, U.count(logp, 'start'), 'helper processes')
    end
  )
end)

QA.describe('helper client: exit and restart', function()
  QA.before_each(before)
  QA.after_each(after)

  QA.check(
    { 'R-140', 'R-195' },
    'A helper exit without shutdown is reported with a WARN notice',
    function()
      local fake = U.make_fake({ mode = 'crash' })
      U.setup_with_path(fake)
      exits(1)
      QA.wait(1000, function()
        return warn_count() > 0
      end)
      a.ge(1, warn_count(), 'WARN notices')
    end
  )

  QA.check(
    'R-140',
    'A helper exit without shutdown appears in :messages',
    { severity = 'medium' },
    function()
      restore()
      local fake = U.make_fake({ mode = 'crash' })
      U.setup_with_path(fake)
      vim.cmd('messages clear')
      exits(1)
      QA.wait(500, function()
        return false
      end)
      local out = vim.api.nvim_exec2('messages', { output = true }).output
      log, restore = QA.capture_notify()
      a.matches('[Hh]elper', out, ':messages output')
    end
  )

  QA.check('R-140', 'After a helper exit the next request starts a new helper', function()
    local fake, logp = U.make_fake({ mode = 'crash' })
    U.setup_with_path(fake)
    exits(1)
    a.eq(1, U.count(logp, 'start'))
    exits(1)
    a.eq(2, U.count(logp, 'start'), 'a second process')
    local pids = U.pids(logp)
    a.ne(pids[1], pids[2], 'a new process id')
  end)

  QA.check(
    'R-140',
    'A helper exit with no request in flight is reported too',
    { severity = 'medium' },
    function()
      local fake, logp = U.make_fake()
      U.setup_with_path(fake)
      U.request('cache.gc', { max_bytes = 1 }, 5000)
      local pid = U.pids(logp)[1]
      QA.need(pid, 'the fake helper pid')
      vim.uv.kill(pid, 'sigkill')
      QA.wait(2000, function()
        return warn_count() > 0
      end)
      a.ge(1, warn_count(), 'WARN notices')
    end
  )

  QA.check('R-195', 'A helper exit opens no popup window', function()
    local fake = U.make_fake({ mode = 'crash' })
    U.setup_with_path(fake)
    U.open_images_preview(IMG_DOC)
    QA.wait(1500, function()
      return warn_count() > 0
    end)
    local snap = U.win_snapshot()
    exits(1)
    QA.wait(300, function()
      return false
    end)
    a.eq(snap, U.win_snapshot(), 'windows, floats, tabs')
    a.eq(0, U.win_snapshot().floats, 'floating windows')
  end)

  QA.check(
    'R-195',
    'A helper exit gives one WARN notice, not one per waiting request',
    { severity = 'medium' },
    function()
      local fake = U.make_fake({ mode = 'crash' })
      U.setup_with_path(fake)
      local helper = U.client()
      local n = 0
      for _ = 1, 4 do
        helper.request('image.prepare', { path = '/x.png', width_px = 10 }, function()
          n = n + 1
        end)
      end
      QA.wait(5000, function()
        return n == 4
      end)
      a.eq(1, warn_count(), 'WARN notices for one exit')
    end
  )
end)

QA.describe('helper client: restart limit', function()
  QA.before_each(before)
  QA.after_each(after)

  QA.check(
    'R-196',
    'After three helper exits within 60 seconds the plugin stops starting it',
    function()
      local fake, logp = U.make_fake({ mode = 'crash' })
      U.setup_with_path(fake)
      exits(3)
      a.eq(3, U.count(logp, 'start'))
      exits(2)
      a.eq(3, U.count(logp, 'start'), 'no start after the third exit')
    end
  )

  QA.check('R-196', 'Three exits at start-up also stop the restarts', function()
    local fake, logp = U.make_fake({ mode = 'exit_at_start' })
    U.setup_with_path(fake)
    exits(3)
    a.eq(3, U.count(logp, 'start'))
    exits(1)
    a.eq(3, U.count(logp, 'start'), 'no start after the third exit')
  end)

  QA.check('R-196', 'Two exits do not stop the restarts', { severity = 'medium' }, function()
    local fake, logp = U.make_fake({ mode = 'crash' })
    U.setup_with_path(fake)
    exits(2)
    exits(1)
    a.eq(3, U.count(logp, 'start'))
  end)

  QA.check(
    'R-196',
    'Requests made while restarts are stopped fail fast and start nothing',
    { severity = 'medium' },
    function()
      local fake, logp = U.make_fake({ mode = 'crash' })
      U.setup_with_path(fake, { helper = { request_timeout_ms = 30000 } })
      exits(3)
      local t0 = vim.uv.hrtime()
      local err, _, done = U.request('cache.gc', { max_bytes = 1 }, 3000)
      a.truthy(done, 'the request settled')
      a.not_nil(err, 'an error')
      a.le(1000, (vim.uv.hrtime() - t0) / 1e6, 'ms to fail')
      a.eq(3, U.count(logp, 'start'))
    end
  )

  QA.check('R-196', ':KittyMarkdown refresh lets the plugin start the helper again', function()
    local fake, logp = U.make_fake({ mode = 'crash' })
    U.setup_with_path(fake)
    U.open_images_preview(IMG_DOC)
    QA.wait(3000, function()
      return U.count(logp, 'start') >= 1 and warn_count() >= 1
    end)
    exits(2)
    a.eq(3, U.count(logp, 'start'), 'three starts before refresh')
    exits(1)
    a.eq(3, U.count(logp, 'start'), 'stopped')
    vim.cmd('KittyMarkdown refresh')
    QA.answer_probe(true)
    QA.wait(3000, function()
      return U.count(logp, 'start') >= 4
    end)
    exits(1)
    a.ge(4, U.count(logp, 'start'), 'a start after refresh')
  end)

  QA.check(
    'R-244',
    'Three exits within 60 seconds give one ERROR message that names :KittyMarkdown refresh',
    function()
      local fake = U.make_fake({ mode = 'crash' })
      U.setup_with_path(fake)
      exits(3)
      QA.wait(500, function()
        return #refresh_msgs() > 0
      end)
      a.eq(1, #refresh_msgs(), 'ERROR messages that name the command')
    end
  )

  QA.check(
    'R-244',
    'The restart limit ERROR is not repeated by later requests',
    { severity = 'medium' },
    function()
      local fake = U.make_fake({ mode = 'crash' })
      U.setup_with_path(fake)
      exits(5)
      QA.wait(300, function()
        return false
      end)
      a.eq(1, #refresh_msgs(), 'ERROR messages that name the command')
    end
  )

  QA.check(
    'R-244',
    'No restart limit ERROR shows before the third exit',
    { severity = 'medium' },
    function()
      local fake = U.make_fake({ mode = 'crash' })
      U.setup_with_path(fake)
      exits(2)
      QA.wait(300, function()
        return false
      end)
      a.eq(0, #refresh_msgs())
    end
  )

  QA.check(
    'R-245',
    'While restarts are stopped, a preview shows each pixel block as a text-only box',
    function()
      local fake = U.make_fake({ mode = 'crash' })
      U.setup_with_path(fake)
      exits(3)
      U.open_images_preview(TWO_DOC)
      QA.wait(2000, function()
        local t = U.virt_text()
        return t:find('First picture alt', 1, true) and t:find('Alpha', 1, true)
      end)
      QA.wait(300, function()
        return false
      end)
      local t = U.virt_text()
      a.contains('First picture alt', t, 'the image box shows its alt text')
      a.contains('Alpha --> Beta', t, 'the diagram box shows its source')
      a.not_contains('loading', t, 'no loading box')
      a.not_contains('helper exited', t, 'no failure box')
    end
  )
end)

QA.describe('helper client: timeout and exit during a render', function()
  QA.before_each(before)
  QA.after_each(after)

  QA.check(
    'R-141',
    'A pixel block whose request gets no answer in time shows a failure box with the reason timeout',
    function()
      local fake = U.make_fake({ mode = 'hang' })
      U.setup_with_path(fake, { helper = { request_timeout_ms = 300 } })
      U.open_images_preview(IMG_DOC)
      a.truthy(U.wait_text('timeout', 4000), 'timeout in the box')
      a.contains('Harbor view at dusk', U.virt_text(), 'the box shows the alt text')
    end
  )

  QA.check(
    'R-141',
    'Lines that are not an answer to the request still end in a timeout box',
    function()
      local fake = U.make_fake({ mode = 'garbage' })
      U.setup_with_path(fake, { helper = { request_timeout_ms = 300 } })
      U.open_images_preview(IMG_DOC)
      a.truthy(U.wait_text('timeout', 4000), 'timeout in the box')
    end
  )

  QA.check(
    'R-141',
    'A diagram whose request gets no answer in time shows a timeout box',
    { severity = 'medium' },
    function()
      local fake = U.make_fake({ mode = 'hang' })
      U.setup_with_path(fake, { helper = { request_timeout_ms = 300 } })
      U.open_images_preview({ '```mermaid', 'flowchart LR', '  Gamma --> Delta', '```' })
      a.truthy(U.wait_text('timeout', 4000), 'timeout in the box')
    end
  )

  QA.check(
    'R-141',
    'An answer that comes before the timeout places the picture with no timeout box',
    function()
      local fake = U.make_fake({ delay = 150 })
      U.setup_with_path(fake, { helper = { request_timeout_ms = 2000 } })
      U.open_images_preview(IMG_DOC)
      QA.wait(3000, function()
        return U.virt_text():find(vim.fn.nr2char(0x10EEEE), 1, true) ~= nil
      end)
      a.not_contains('timeout', U.virt_text())
      a.contains(vim.fn.nr2char(0x10EEEE), U.virt_text(), 'placeholder cells')
    end
  )

  QA.check(
    'R-141',
    'An answer that comes after the timeout leaves the timeout box and raises no error',
    { severity = 'medium' },
    function()
      local fake = U.make_fake({ delay = 700 })
      U.setup_with_path(fake, { helper = { request_timeout_ms = 200 } })
      vim.v.errmsg = ''
      U.open_images_preview(IMG_DOC)
      a.truthy(U.wait_text('timeout', 3000), 'timeout first')
      QA.wait(1200, function()
        return false
      end)
      a.contains('timeout', U.virt_text(), 'still a timeout box')
      a.no_match('[Ll]ua', vim.v.errmsg, 'no Lua error')
    end
  )

  QA.check(
    'R-141',
    'The default request timeout is 30 seconds: a slow answer within a few seconds is not a timeout',
    { severity = 'medium' },
    function()
      local fake = U.make_fake({ delay = 2500 })
      U.setup_with_path(fake)
      U.open_images_preview(IMG_DOC)
      QA.wait(4000, function()
        return U.virt_text():find(vim.fn.nr2char(0x10EEEE), 1, true) ~= nil
      end)
      a.not_contains('timeout', U.virt_text())
    end
  )

  QA.check(
    'R-198',
    'A helper exit while a pixel block waits shows a failure box with the reason helper exited',
    function()
      local fake = U.make_fake({ mode = 'crash' })
      U.setup_with_path(fake)
      U.open_images_preview(IMG_DOC)
      a.truthy(U.wait_text('helper exited', 4000), 'helper exited in the box')
      a.contains('Harbor view at dusk', U.virt_text(), 'the box shows the alt text')
    end
  )

  QA.check(
    'R-198',
    'A helper exit fails every waiting block of the preview with helper exited',
    { severity = 'medium' },
    function()
      local fake = U.make_fake({ mode = 'crash' })
      U.setup_with_path(fake)
      U.open_images_preview(TWO_DOC)
      QA.wait(4000, function()
        local _, n = U.virt_text():gsub('helper exited', '')
        return n >= 2
      end)
      local _, n = U.virt_text():gsub('helper exited', '')
      a.eq(2, n, 'boxes with helper exited')
    end
  )

  QA.check(
    'R-198',
    'A helper killed from outside during a render shows helper exited, not timeout',
    { severity = 'medium' },
    function()
      local fake, logp = U.make_fake({ mode = 'hang' })
      U.setup_with_path(fake, { helper = { request_timeout_ms = 5000 } })
      U.open_images_preview(IMG_DOC)
      QA.wait(3000, function()
        return U.count(logp, 'method image.prepare') > 0
      end)
      local pid = U.pids(logp)[1]
      QA.need(pid, 'the fake helper pid')
      vim.uv.kill(pid, 'sigkill')
      a.truthy(U.wait_text('helper exited', 3000), 'helper exited in the box')
      a.not_contains('timeout', U.virt_text())
    end
  )
end)

QA.describe('helper client: no helper binary', function()
  QA.before_each(before)
  QA.after_each(after)

  local function no_helper()
    local helper = U.client()
    QA.need(type(helper.set_locator) == 'function', 'a locator hook in the helper module')
    helper.set_locator(function(cb)
      cb(nil)
    end)
  end

  QA.check(
    { 'R-134', 'R-194' },
    'With no helper binary, the plugin shows the no-helper message with the build command once',
    function()
      no_helper()
      U.open_images_preview(IMG_DOC)
      QA.wait(2000, function()
        return #U.at_level(log, vim.log.levels.ERROR) > 0
      end)
      local hits = 0
      for _, e in ipairs(log) do
        if e.msg == U.NO_HELPER then
          hits = hits + 1
        end
      end
      a.eq(1, hits, 'the exact message')
    end
  )

  QA.check(
    'R-134',
    'With no helper binary, the message is not repeated for later previews or requests',
    function()
      no_helper()
      U.open_images_preview(IMG_DOC)
      QA.wait(1500, function()
        return #log > 0
      end)
      U.open_doc(TWO_DOC, QA.tmpdir())
      require('kitty-markdown').open('split')
      U.request('cache.gc', { max_bytes = 1 }, 2000)
      QA.wait(500, function()
        return false
      end)
      local hits = 0
      for _, e in ipairs(log) do
        if type(e.msg) == 'string' and e.msg:find('no helper binary found', 1, true) then
          hits = hits + 1
        end
      end
      a.eq(1, hits, 'no-helper messages')
    end
  )

  QA.check('R-134', 'With no helper binary, pixel blocks show as text-only boxes', function()
    no_helper()
    U.open_images_preview(TWO_DOC)
    QA.wait(2000, function()
      local t = U.virt_text()
      return t:find('First picture alt', 1, true)
        and t:find('Alpha', 1, true)
        and not t:find('loading', 1, true)
    end)
    local t = U.virt_text()
    a.contains('First picture alt', t)
    a.contains('Alpha --> Beta', t)
    a.not_contains('loading', t)
  end)

  QA.check(
    'R-134',
    'With no helper binary, the error opens no popup window',
    { severity = 'medium' },
    function()
      no_helper()
      U.open_images_preview(IMG_DOC)
      QA.wait(1500, function()
        return #log > 0
      end)
      a.eq(0, U.win_snapshot().floats)
    end
  )

  QA.check(
    'R-194',
    'The no-helper message is shown at ERROR level',
    { severity = 'medium' },
    function()
      no_helper()
      U.open_images_preview(IMG_DOC)
      QA.wait(1500, function()
        return #log > 0
      end)
      a.contains(U.NO_HELPER, U.at_level(log, vim.log.levels.ERROR))
    end
  )
end)
