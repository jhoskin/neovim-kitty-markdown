-- qa: serial
local QA = require('qa')
local U = require('commands_util')
local a = QA.assert
local api = vim.api

local function ms_since(t0)
  return (vim.uv.hrtime() - t0) / 1e6
end

local function median(xs)
  table.sort(xs)
  return xs[math.floor((#xs + 1) / 2)]
end

-- A mixed document of exactly `n` lines (R-256): headings, wrapped
-- paragraphs with inline markup and links, nested lists, tables, code
-- fences, blockquotes, and images.
local function big_doc(n, tag)
  local lines = { '# Large document ' .. tag, '' }
  local i = 0
  while #lines < n do
    i = i + 1
    local kind = i % 7
    if kind == 6 then
      vim.list_extend(lines, { '![figure ' .. i .. '](p.png)', '' })
    elseif kind == 0 then
      vim.list_extend(lines, { '## Section ' .. i, '' })
    elseif kind == 1 then
      vim.list_extend(lines, {
        'Paragraph '
          .. i
          .. ' has **bold**, *italic*, `code`, and a [link](https://example.com/'
          .. i
          .. ').',
        'It continues on a second source line with ~~struck~~ words and more text to wrap around.',
        '',
      })
    elseif kind == 2 then
      vim.list_extend(
        lines,
        { '- item ' .. i .. ' one', '- item two', '  - nested item', '- [x] done task', '' }
      )
    elseif kind == 3 then
      vim.list_extend(lines, {
        '```lua',
        'local x' .. i .. ' = { a = 1, b = "two" }',
        'print(x' .. i .. '.a)',
        '```',
        '',
      })
    elseif kind == 4 then
      vim.list_extend(lines, {
        '| col a | col b | col c |',
        '| --- | :-: | --: |',
        '| ' .. i .. ' | mid | right |',
        '| x | y | z |',
        '',
      })
    else
      vim.list_extend(lines, { '> A quote ' .. i .. ' with *style*.', '> Second quote line.', '' })
    end
  end
  while #lines > n do
    table.remove(lines)
  end
  return lines
end

QA.describe('performance: open a large document', function()
  QA.before_each(function()
    U.reset({ placement = 'split' })
  end)

  QA.check(
    { 'R-023', 'R-256' },
    'Opening a preview of the specified 1000-line mixed document shows its first text within 200 ms',
    function()
      -- warm up: load the plugin modules once with a small document
      U.src({ '# warm', '', 'small' }, { name = 'warm.md' })
      vim.cmd('KittyMarkdown open')
      vim.cmd('KittyMarkdown close')
      U.settle(50)
      local times = {}
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/p.png', 120, 60)
      for run = 1, 3 do
        local lines = big_doc(1000, 'run' .. run)
        a.eq(1000, #lines)
        local buf = U.src(lines, { name = 'big' .. run .. '.md', dir = dir })
        local last = 'Section'
        local t0 = vim.uv.hrtime()
        vim.cmd('KittyMarkdown open')
        local p = U.preview()
        local ok = p
          and QA.wait(2000, function()
            return U.text(p.buf):find('Large document run' .. run, 1, true) ~= nil
          end, 1)
        times[#times + 1] = ms_since(t0)
        a.truthy(ok, 'preview text shown (run ' .. run .. ')')
        a.not_nil(U.find_line(p.buf, last))
        api.nvim_set_current_buf(buf)
        vim.cmd('KittyMarkdown close')
        U.settle(50)
      end
      a.le(200, median(times), 'median open time in ms over ' .. vim.inspect(times))
    end
  )

  QA.check(
    'R-023',
    'Opening the 1000-line document in image mode shows its first text within 200 ms',
    { severity = 'medium' },
    function()
      U.image_mode()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/p.png', 120, 60)
      local lines = big_doc(1000, 'img')
      U.src(lines, { dir = dir })
      local t0 = vim.uv.hrtime()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      local ok = QA.wait(2000, function()
        return U.text(p.buf):find('Large document img', 1, true) ~= nil
      end, 1)
      local ms = ms_since(t0)
      a.truthy(ok)
      a.le(200, ms, 'open time in ms')
    end
  )
end)

QA.describe('performance: helper timing', function()
  QA.before_each(function()
    U.reset()
  end)

  local function flow(n)
    return ('flowchart LR\n  A%d[Start %d] --> B%d{Check}\n  B%d -->|yes| C%d[Done]\n  B%d -->|no| A%d'):format(
      n,
      n,
      n,
      n,
      n,
      n,
      n
    )
  end

  QA.check(
    { 'R-024', 'R-257' },
    'A running helper returns an uncached small flowchart within 100 ms',
    function()
      local sess = QA.helper_session()
      local warm = sess:request(
        'mermaid.render',
        { source = flow(0), width_px = 800, theme = 'default', background = 'transparent' },
        30000
      )
      a.not_nil(warm, 'warm-up render answered')
      a.is_nil(warm.error, 'warm-up render error: ' .. vim.inspect(warm.error))
      local times = {}
      for n = 1, 5 do
        local t0 = vim.uv.hrtime()
        local res = sess:request(
          'mermaid.render',
          { source = flow(n), width_px = 800, theme = 'default', background = 'transparent' },
          5000
        )
        times[#times + 1] = ms_since(t0)
        a.not_nil(res, 'render ' .. n .. ' answered')
        a.is_nil(res.error, 'render ' .. n .. ' error: ' .. vim.inspect(res.error))
        a.falsy(res.result and res.result.cached, 'a new diagram, not a cache hit')
      end
      sess:close()
      a.le(100, median(times), 'median render time in ms over ' .. vim.inspect(times))
    end
  )

  -- A flowchart with exactly 20 nodes, unique per `n` so it is not cached.
  local function flow20(n)
    local out = { 'flowchart TD' }
    for k = 1, 19 do
      out[#out + 1] = ('  N%d_%d[Node %d of run %d] --> N%d_%d'):format(n, k, k, n, n, k + 1)
    end
    out[#out + 1] = ('  N%d_20[Last node %d]'):format(n, n)
    return table.concat(out, '\n')
  end

  QA.check(
    { 'R-024', 'R-257' },
    'A running helper returns an uncached flowchart of 20 nodes within 100 ms',
    function()
      local sess = QA.helper_session()
      local warm = sess:request(
        'mermaid.render',
        { source = flow20(0), width_px = 800, theme = 'default', background = 'transparent' },
        30000
      )
      a.not_nil(warm, 'warm-up render answered')
      a.is_nil(warm.error, 'warm-up render error: ' .. vim.inspect(warm and warm.error))
      local times = {}
      for n = 1, 5 do
        local t0 = vim.uv.hrtime()
        local res = sess:request(
          'mermaid.render',
          { source = flow20(n), width_px = 800, theme = 'default', background = 'transparent' },
          5000
        )
        times[#times + 1] = ms_since(t0)
        a.not_nil(res, 'render ' .. n .. ' answered')
        a.is_nil(res.error, 'render ' .. n .. ' error: ' .. vim.inspect(res.error))
        a.falsy(res.result and res.result.cached, 'not a cache hit')
      end
      sess:close()
      a.le(100, median(times), 'median render time in ms over ' .. vim.inspect(times))
    end
  )

  QA.check('R-025', 'A freshly started helper answers its first request within 500 ms', function()
    -- warm up the file cache with one start
    local warm = QA.helper_session()
    warm:request('probe', {}, 10000)
    warm:close()
    local times = {}
    for _ = 1, 3 do
      local t0 = vim.uv.hrtime()
      local sess = QA.helper_session()
      local res = sess:request('probe', {}, 5000)
      times[#times + 1] = ms_since(t0)
      a.not_nil(res, 'probe answered')
      a.is_nil(res.error, vim.inspect(res and res.error))
      sess:close()
    end
    a.le(500, median(times), 'median first answer in ms over ' .. vim.inspect(times))
  end)

  QA.check(
    'R-025',
    'When the plugin starts the helper, the helper is ready for requests within 500 ms',
    { severity = 'medium' },
    function()
      local helper = require('kitty-markdown.helper')
      QA.need(type(helper.ensure) == 'function', 'a way to start the helper from the plugin')
      local warm_done = false
      helper.ensure(function()
        warm_done = true
      end)
      QA.wait(10000, function()
        return warm_done
      end)
      if type(helper.shutdown) == 'function' then
        helper.shutdown(2000)
      end
      U.reset()
      helper = require('kitty-markdown.helper')
      local done, err = false, nil
      local t0 = vim.uv.hrtime()
      helper.ensure(function(e)
        done, err = true, e
      end)
      QA.wait(5000, function()
        return done
      end, 1)
      local ms = ms_since(t0)
      a.truthy(done, 'helper became ready')
      a.is_nil(err, vim.inspect(err))
      a.le(500, ms, 'start to ready in ms')
    end
  )
end)

QA.describe('performance: scroll sync debounce', function()
  QA.before_each(function()
    U.reset({ placement = 'split' })
  end)

  local function doc()
    local lines = { '# Debounce', '' }
    local heads = {}
    for i = 1, 60 do
      heads[i] = #lines + 1
      lines[#lines + 1] = '## Part ' .. i
      lines[#lines + 1] = ''
      for k = 1, 8 do
        lines[#lines + 1] = 'Body ' .. i .. '.' .. k
        lines[#lines + 1] = ''
      end
    end
    return lines, heads
  end

  -- Moves the source cursor every `step_ms` for `count` moves while it
  -- samples the preview top line every 2 ms. Returns the times (ms from
  -- the first move) at which the top line changed, and the duration of
  -- the movement.
  local function run(src_win, pwin, heads, count, step_ms, start)
    local changes = {}
    local t0 = vim.uv.hrtime()
    local last = U.topline(pwin)
    local function sample()
      local now = U.topline(pwin)
      if now ~= last then
        changes[#changes + 1] = ms_since(t0)
        last = now
      end
      return false
    end
    local last_move = 0
    for k = 1, count do
      api.nvim_set_current_win(src_win)
      api.nvim_win_set_cursor(src_win, { heads[start + k * 2], 0 })
      last_move = ms_since(t0)
      api.nvim_exec_autocmds('CursorMoved', { buffer = api.nvim_win_get_buf(src_win) })
      vim.wait(step_ms, sample, 2)
    end
    local moving = ms_since(t0)
    vim.wait(400, sample, 2)
    return changes, moving, last_move
  end

  QA.check(
    'R-163',
    'Scroll sync scrolls at most once per 100 ms while the cursor keeps moving',
    function()
      local lines, heads = doc()
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      local changes, moving = run(src_win, p.win, heads, 12, 20, 2)
      a.ge(1, #changes, 'the preview scrolled at least once')
      a.le(
        math.ceil(moving / 100) + 1,
        #changes,
        ('scrolls %s during %d ms of movement'):format(vim.inspect(changes), moving)
      )
      for k = 2, #changes do
        a.ge(90, changes[k] - changes[k - 1], 'gap between scrolls: ' .. vim.inspect(changes))
      end
    end
  )

  QA.check(
    'R-163',
    'After the cursor stops, the preview scrolls once to the final position',
    function()
      local lines, heads = doc()
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      run(src_win, p.win, heads, 8, 15, 4)
      local target = U.find_line(p.buf, 'Part ' .. (4 + 8 * 2))
      local top = U.topline(p.win)
      local h = api.nvim_win_get_height(p.win)
      a.truthy(
        top <= target and target < top + h,
        ('final block %d visible in %d..%d'):format(target, top, top + h - 1)
      )
    end
  )

  QA.check(
    'R-163',
    'A single cursor move is synced within a short delay, not lost to the debounce',
    { severity = 'medium' },
    function()
      local lines, heads = doc()
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      local top0 = U.topline(p.win)
      local t0 = vim.uv.hrtime()
      api.nvim_win_set_cursor(src_win, { heads[40], 0 })
      api.nvim_exec_autocmds('CursorMoved', { buffer = api.nvim_win_get_buf(src_win) })
      local moved = QA.wait(1000, function()
        return U.topline(p.win) ~= top0
      end, 2)
      local ms = ms_since(t0)
      a.truthy(moved, 'the preview scrolled')
      a.le(300, ms, 'delay in ms')
    end
  )

  QA.check(
    'R-342',
    'While the cursor keeps moving, scroll sync waits and scrolls only after the movement pauses',
    function()
      local lines, heads = doc()
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      -- 15 moves, 20 ms apart: the movement never pauses for 100 ms
      local changes, _, last_move = run(src_win, p.win, heads, 15, 20, 2)
      a.ge(1, #changes, 'the preview scrolled after the movement')
      -- 50 ms of slack for the cached loop time of the timers
      a.ge(
        last_move + 50,
        changes[1],
        ('first scroll at %s ms, last move at %d ms'):format(vim.inspect(changes), last_move)
      )
    end
  )

  QA.check(
    'R-342',
    'Scroll sync does not scroll at the start of a cursor movement',
    { severity = 'medium' },
    function()
      local lines, heads = doc()
      U.src(lines)
      local src_win = api.nvim_get_current_win()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      local top0 = U.topline(p.win)
      api.nvim_win_set_cursor(src_win, { heads[45], 0 })
      api.nvim_exec_autocmds('CursorMoved', { buffer = api.nvim_win_get_buf(src_win) })
      U.settle(30)
      a.eq(top0, U.topline(p.win), 'no scroll right after the move')
      a.truthy(
        QA.wait(1000, function()
          return U.topline(p.win) ~= top0
        end, 2),
        'scrolled after the pause'
      )
    end
  )
end)
