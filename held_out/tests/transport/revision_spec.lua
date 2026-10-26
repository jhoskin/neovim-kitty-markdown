local QA = require('qa')
local U = require('transport_util')
local a = QA.assert

local IMG_DOC = { '# Rev', '', 'Some text.', '', '![revision picture](a.png)', '', 'After.' }
local INFO = vim.log.levels.INFO

local function open_img(dir)
  QA.make_png(dir .. '/a.png', 80, 60)
  return U.open_doc(dir, IMG_DOC)
end

-- The notify entries whose message is exactly `want`.
local function entries(log, want)
  local out = {}
  for _, e in ipairs(log) do
    if type(e.msg) == 'string' and vim.trim(e.msg) == want then
      out[#out + 1] = e
    end
  end
  return out
end

-- Checks that `want` reached :messages once and, through vim.notify, at
-- the INFO level.
local function assert_info_once(read, log, want)
  a.eq(1, U.count_line(read(), want), 'exact lines in :messages')
  local hits = entries(log, want)
  QA.need(#hits > 0, 'the mode message goes through vim.notify, so its level can be read')
  a.eq(1, #hits, 'notify calls with the message')
  local lvl = hits[1].level
  a.truthy(lvl == nil or lvl == INFO, 'level of the message: ' .. tostring(lvl))
end

local function bare_da1_count(text)
  local n = 0
  for _ in text:gmatch('\27%[0?c') do
    n = n + 1
  end
  return n
end

QA.describe('transport: revision 2026-10-25', function()
  local restore
  QA.after_each(function()
    if restore then
      restore()
      restore = nil
    end
    QA.reset_plugin()
    vim.env.TMUX = nil
    vim.o.termguicolors = true
  end)

  QA.check(
    'R-303',
    'Inside tmux the device attributes request is sent bare, outside the passthrough',
    function()
      local cap, dir = U.env({ tmux = '/tmp/tmux-501/default,77,0', helper = false })
      open_img(dir)
      local wraps, outside = U.split_tmux(cap:text())
      a.eq(1, bare_da1_count(outside), 'bare DA1 requests outside any passthrough')
      for i, w in ipairs(wraps) do
        a.eq(0, bare_da1_count(w.inner), 'DA1 inside passthrough ' .. i)
      end
      a.eq(1, #U.queries(cap), 'the graphics query still goes through the passthrough')
      a.no_match('\27_G', outside, 'graphics sequence outside a passthrough')
    end
  )

  QA.check(
    'R-303',
    'A refresh inside tmux also sends the device attributes request bare',
    function()
      local cap, dir = U.env({ tmux = '/tmp/tmux-501/default,77,0' })
      local src = open_img(dir)
      U.answer_yes(cap)
      a.truthy(U.wait_placed(src, 1), 'placed')
      cap:clear()
      vim.api.nvim_set_current_win(vim.fn.bufwinid(src))
      vim.cmd('KittyMarkdown refresh')
      U.settle(30)
      local wraps, outside = U.split_tmux(cap:text())
      a.eq(1, bare_da1_count(outside), 'bare DA1 requests after refresh')
      for i, w in ipairs(wraps) do
        a.eq(0, bare_da1_count(w.inner), 'DA1 inside passthrough ' .. i)
      end
    end
  )

  QA.check('R-303', 'Outside tmux the device attributes request is sent bare as well', function()
    local cap, dir = U.env({ helper = false })
    open_img(dir)
    a.not_contains('\27Ptmux;', cap:text(), 'passthrough without TMUX')
    a.eq(1, bare_da1_count(cap:text()), 'DA1 requests')
  end)

  QA.check(
    'R-304',
    'Neovim instances started at the same time pick different random start ids',
    function()
      local _, dir = U.env({ helper = false })
      local script = dir .. '/first_id.lua'
      QA.write(
        script,
        table.concat({
          'vim.opt.runtimepath:prepend(' .. vim.inspect(QA.root) .. ')',
          'local id = require("kitty-markdown.transport").alloc_id()',
          'io.stdout:write(tostring(id))',
        }, '\n')
      )
      local procs = {}
      for i = 1, 5 do
        procs[i] = vim.system({ vim.v.progpath, '--clean', '-l', script }, { text = true })
      end
      local seen, distinct = {}, 0
      for i, p in ipairs(procs) do
        local res = p:wait(10000)
        local id = tonumber(vim.trim(res.stdout or ''))
        a.not_nil(id, 'start id of instance ' .. i .. ': ' .. tostring(res.stderr))
        a.ge(1, id, 'start id')
        a.le(2 ^ 23, id, 'start id')
        if not seen[id] then
          seen[id] = true
          distinct = distinct + 1
        end
      end
      a.ge(4, distinct, 'distinct start ids among 5 instances')
    end
  )

  QA.check('R-304', 'The start id is not a fixed small value', { severity = 'medium' }, function()
    local _, dir = U.env({ helper = false })
    local script = dir .. '/first_id.lua'
    QA.write(
      script,
      table.concat({
        'vim.opt.runtimepath:prepend(' .. vim.inspect(QA.root) .. ')',
        'io.stdout:write(tostring(require("kitty-markdown.transport").alloc_id()))',
      }, '\n')
    )
    local ids = {}
    for i = 1, 3 do
      local res = QA.sh({ vim.v.progpath, '--clean', '-l', script }, { timeout = 10000 })
      ids[i] = tonumber(vim.trim(res.stdout or ''))
      a.not_nil(ids[i], 'start id printed')
    end
    local high = 0
    for _, id in ipairs(ids) do
      if id > 1000 then
        high = high + 1
      end
    end
    a.ge(1, high, 'start ids above a small fixed range: ' .. vim.inspect(ids))
  end)

  QA.check(
    'R-305',
    'Turning images on writes the exact image mode message once at the INFO level',
    function()
      local cap, dir = U.env()
      local read = U.messages_reader()
      local log
      log, restore = U.notify_log()
      local src = open_img(dir)
      U.answer_yes(cap)
      a.truthy(U.wait_placed(src, 1), 'placed')
      U.settle(600)
      assert_info_once(read, log, U.IMAGES_MSG)
    end
  )

  QA.check(
    'R-305',
    'The image mode message is not written when the probe turns images off',
    function()
      local _, dir = U.env({ helper = false })
      local read = U.messages_reader()
      open_img(dir)
      U.answer_no()
      a.truthy(U.wait_mode('text'), 'text-only mode')
      U.settle(100)
      a.eq(0, U.count_line(read(), U.IMAGES_MSG), 'image mode messages')
    end
  )

  QA.check(
    { 'R-306', 'R-240' },
    'A device attributes reply alone writes the exact text-only message with the reason no graphics reply at INFO',
    function()
      local _, dir = U.env({ helper = false })
      local read = U.messages_reader()
      local log
      log, restore = U.notify_log()
      open_img(dir)
      U.answer_no()
      a.truthy(U.wait_mode('text'), 'text-only mode')
      U.settle(100)
      assert_info_once(read, log, U.TEXT_MSG .. 'no graphics reply')
    end
  )

  QA.check(
    { 'R-306', 'R-240' },
    'No reply writes the exact text-only message with the reason probe timeout at INFO',
    function()
      local _, dir = U.env({ helper = false })
      local read = U.messages_reader()
      local log
      log, restore = U.notify_log()
      open_img(dir)
      a.truthy(U.wait_mode('text', 3000), 'text-only mode after the timeout')
      U.settle(100)
      assert_info_once(read, log, U.TEXT_MSG .. 'probe timeout')
    end
  )

  QA.check(
    { 'R-306', 'R-240' },
    'With termguicolors off the exact text-only message gives the reason termguicolors off at INFO',
    function()
      local cap, dir = U.env({ termguicolors = false })
      local read = U.messages_reader()
      local log
      log, restore = U.notify_log()
      open_img(dir)
      U.answer_yes(cap)
      a.truthy(
        QA.wait(1500, function()
          return U.count_line(read(), U.TEXT_MSG .. 'termguicolors off') > 0
        end),
        'the message arrives'
      )
      U.settle(100)
      assert_info_once(read, log, U.TEXT_MSG .. 'termguicolors off')
      a.eq(0, U.count_line(read(), U.IMAGES_MSG), 'no image mode message')
    end
  )

  QA.check(
    { 'R-306', 'R-240' },
    'With a zero pixel size the exact text-only message gives the reason cell size unknown at INFO',
    function()
      local cap, dir = U.env()
      require('kitty-markdown.transport').set_winsize_reader(function()
        return 24, 80, 0, 0
      end)
      local read = U.messages_reader()
      local log
      log, restore = U.notify_log()
      open_img(dir)
      U.answer_yes(cap)
      a.truthy(
        QA.wait(1500, function()
          return U.count_line(read(), U.TEXT_MSG .. 'cell size unknown') > 0
        end),
        'the message arrives'
      )
      U.settle(100)
      assert_info_once(read, log, U.TEXT_MSG .. 'cell size unknown')
      a.eq(0, U.count_line(read(), U.IMAGES_MSG), 'no image mode message')
    end
  )

  QA.check(
    'R-307',
    'A graphics reply that arrives after the probe timeout is ignored: the preview stays text-only',
    function()
      local cap, dir = U.env()
      local read = U.messages_reader()
      local src = open_img(dir)
      a.truthy(U.wait_mode('text', 3000), 'text-only mode after the timeout')
      U.settle(50)
      U.answer_yes(cap)
      U.settle(400)
      a.eq('text', U.mode(), 'mode after the late reply')
      a.eq(0, #U.transmissions(cap), 'no image data after the late reply')
      a.eq(0, #U.placeholder_marks(U.preview_buf(src)), 'no placeholder cells')
      a.eq(0, U.count_line(read(), U.IMAGES_MSG), 'no image mode message')
      local box = U.mark_with(U.preview_buf(src), 'revision picture')
      a.truthy(box and U.is_box(box.rows), 'the image still shows as a text-only box')
    end
  )

  QA.check(
    'R-307',
    'After a late graphics reply was ignored, the next probe can still turn images on',
    function()
      local cap, dir = U.env()
      local src = open_img(dir)
      a.truthy(U.wait_mode('text', 3000), 'text-only mode after the timeout')
      U.answer_yes(cap)
      U.settle(100)
      cap:clear()
      vim.api.nvim_set_current_win(vim.fn.bufwinid(src))
      vim.cmd('KittyMarkdown refresh')
      U.answer_yes(cap)
      a.truthy(U.wait_mode('images'), 'image mode after the next probe')
      a.truthy(U.wait_placed(src, 1), 'the image shows')
    end
  )

  QA.check(
    'R-307',
    'A late device attributes reply after the timeout writes no second mode message',
    { severity = 'medium' },
    function()
      local _, dir = U.env({ helper = false })
      local read = U.messages_reader()
      open_img(dir)
      a.truthy(U.wait_mode('text', 3000), 'text-only mode after the timeout')
      U.answer_no()
      U.settle(200)
      local msgs = read()
      a.eq(1, U.count_line(msgs, U.TEXT_MSG .. 'probe timeout'), 'timeout messages')
      a.eq(
        0,
        U.count_line(msgs, U.TEXT_MSG .. 'no graphics reply'),
        'a second reason after the timeout'
      )
    end
  )

  QA.check(
    'R-309',
    'A graphics error reply to the probe counts as an answer and turns images on',
    function()
      local cap, dir = U.env()
      local read = U.messages_reader()
      local src = open_img(dir)
      U.fire('\27_Gi=' .. U.query_id(cap) .. ';EINVAL:Unknown error' .. U.ST)
      U.fire('\27[?62;c')
      a.truthy(U.wait_mode('images'), 'image mode after an error reply')
      a.truthy(U.wait_placed(src, 1), 'the image shows')
      U.settle(50)
      a.eq(1, U.count_line(read(), U.IMAGES_MSG), 'image mode message')
      a.eq(0, U.count_line(read(), U.TEXT_MSG .. 'no graphics reply'), 'no text-only message')
    end
  )

  QA.check(
    'R-309',
    'A graphics error reply with no device attributes reply after it also turns images on',
    function()
      local cap, dir = U.env()
      local src = open_img(dir)
      U.fire('\27_Gi=' .. U.query_id(cap) .. ';ENOTSUPPORTED:no such format' .. U.ST)
      a.truthy(U.wait_mode('images', 1500), 'image mode')
      a.truthy(U.wait_placed(src, 1), 'the image shows')
    end
  )

  QA.check(
    'R-308',
    'Every delete the plugin sends while previews close and change uses d=I',
    function()
      local cap, dir = U.env()
      QA.make_png(dir .. '/b.png', 30, 30, { 0, 0, 200, 255 })
      local src = open_img(dir)
      U.answer_yes(cap)
      a.truthy(U.wait_placed(src, 1), 'placed')
      vim.api.nvim_set_current_win(vim.fn.bufwinid(src))
      vim.api.nvim_buf_set_lines(src, 4, 5, false, { '![revision picture](b.png)' })
      vim.cmd('silent write')
      a.truthy(
        QA.wait(8000, function()
          return #U.transmissions(cap) >= 2
        end),
        'the changed image is transmitted'
      )
      require('kitty-markdown.preview').close(src)
      U.settle(50)
      local d = U.deletes(cap)
      a.ge(2, #d, 'deletes for the changed and the closed image')
      for i, c in ipairs(d) do
        a.eq('I', c.control.d, 'delete ' .. i)
      end
    end
  )
end)
