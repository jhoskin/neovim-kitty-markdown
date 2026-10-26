local QA = require('qa')
local U = require('transport_util')
local a = QA.assert

local IMG_DOC =
  { '# Probe', '', 'Some text before.', '', '![first picture](a.png)', '', 'Text after.' }

local function doc_with_png(dir, name)
  QA.make_png(dir .. '/a.png', 120, 70)
  return U.open_doc(dir, IMG_DOC, name)
end

local function index_of(text, needle, init)
  return text:find(needle, init or 1, true)
end

QA.describe('transport: probe', function()
  QA.after_each(function()
    QA.reset_plugin()
    vim.env.TMUX = nil
    vim.o.termguicolors = true
  end)

  QA.check(
    'R-117',
    'Loading the plugin and calling setup write nothing to the terminal before a preview opens',
    function()
      local cap = U.env()
      require('kitty-markdown').setup({ placement = 'split' })
      U.settle(30)
      a.eq('', cap:text(), 'bytes written before the first preview')
    end
  )

  QA.check(
    'R-117',
    'Opening the first preview sends a kitty graphics query action followed by a Primary Device Attributes request',
    function()
      local cap, dir = U.env()
      doc_with_png(dir)
      local text = cap:text()
      local queries = U.queries(cap)
      a.eq(1, #queries, 'graphics query actions sent')
      a.not_nil(queries[1].control.i, 'the query carries an image id so the terminal answers')
      a.ne('0', queries[1].control.i, 'query id is not zero')
      local q = index_of(text, queries[1].raw)
      local da1 = index_of(text, '\27[c') or index_of(text, '\27[0c')
      a.not_nil(da1, 'a DA1 request (ESC [ c) is sent')
      a.truthy(q and da1 and q < da1, 'the graphics query comes before the DA1 request')
    end
  )

  QA.check(
    'R-117',
    'The query action is a valid kitty query: direct data with a payload',
    function()
      local cap, dir = U.env()
      doc_with_png(dir)
      local q = U.queries(cap)[1]
      a.not_nil(q, 'a query action')
      local t = q.control.t or 'd'
      a.eq('d', t, 'the query uses direct transmission')
      a.truthy(#q.payload > 0, 'the query carries a payload')
      a.no_error(function()
        assert(vim.base64.decode(q.payload))
      end, 'the query payload is base64')
    end
  )

  QA.check(
    'R-117',
    'A second preview opened after the first one does not send another probe',
    function()
      local cap, dir = U.env()
      doc_with_png(dir)
      U.answer_yes(cap)
      a.truthy(U.wait_mode('images'), 'image mode after the reply')
      local before = #U.queries(cap)
      vim.cmd('tabnew')
      U.open_doc(dir, { '# Other', '', 'text' }, 'other.md')
      U.settle(30)
      a.eq(before, #U.queries(cap), 'graphics queries after the second open')
    end
  )

  QA.check(
    'R-118',
    'A graphics reply before the timeout puts the preview in image mode and the image shows as placeholder cells',
    function()
      local cap, dir = U.env()
      local src = doc_with_png(dir)
      U.answer_yes(cap)
      a.truthy(U.wait_mode('images'), 'image mode after the reply')
      local ok = U.wait_placed(src, 1)
      a.truthy(ok, 'the image block shows placeholder cells')
    end
  )

  QA.check('R-118', 'The standard kitty reply sequence to the probe turns image mode on', function()
    local cap, dir = U.env()
    local src = doc_with_png(dir)
    QA.answer_probe(true)
    a.truthy(U.wait_mode('images'), 'image mode after the reply')
    a.truthy(U.wait_placed(src, 1), 'the image block shows placeholder cells')
    a.truthy(#U.transmissions(cap) >= 1, 'the image is transmitted')
  end)

  QA.check(
    'R-118',
    'An unrelated terminal response that arrives before the graphics reply does not end the probe',
    function()
      local cap, dir = U.env()
      local src = doc_with_png(dir)
      U.fire('\27]11;rgb:1e1e/1e1e/1e1e\27\\')
      U.fire('\27[?2026;2$y')
      U.settle(30)
      a.ne('text', U.mode(), 'mode after unrelated responses')
      U.answer_yes(cap)
      a.truthy(U.wait_mode('images'), 'image mode after the graphics reply')
      a.truthy(U.wait_placed(src, 1), 'the image block shows placeholder cells')
    end
  )

  QA.check(
    { 'R-119', 'R-240' },
    'With no reply at all the preview goes to text-only mode with the reason probe timeout after about 500 ms, not earlier',
    function()
      local cap, dir = U.env({ helper = false })
      local read = U.messages_reader()
      local t0 = vim.uv.hrtime()
      doc_with_png(dir)
      U.settle(380)
      a.ne('images', U.mode(), 'mode before the timeout')
      a.eq(0, U.count_with(read(), 'probe timeout'), 'probe timeout reported before 400 ms')
      local ok = QA.wait(3000, function()
        return U.count_with(read(), 'probe timeout') > 0
      end)
      local ms = (vim.uv.hrtime() - t0) / 1e6
      a.truthy(ok, 'the reason probe timeout reaches :messages')
      a.ge(400, ms, 'time until the timeout')
      a.eq('text', U.mode(), 'text-only mode')
      a.eq(0, #U.transmissions(cap), 'no image data transmitted in text-only mode')
    end
  )

  QA.check(
    { 'R-119', 'R-240' },
    'A device attributes reply without a graphics reply gives text-only mode with the reason no graphics reply',
    function()
      local cap, dir = U.env({ helper = false })
      local read = U.messages_reader()
      local src = doc_with_png(dir)
      U.answer_no()
      a.truthy(U.wait_mode('text', 400), 'text-only mode well before the timeout')
      a.truthy(
        QA.wait(400, function()
          return U.count_with(read(), 'no graphics reply') > 0
        end),
        'the reason no graphics reply reaches :messages'
      )
      a.eq(0, U.count_with(read(), 'probe timeout'), 'the timeout reason is not given')
      U.settle(600)
      a.eq(0, U.count_with(read(), 'probe timeout'), 'the timeout does not fire after the answer')
      a.eq(0, #U.placeholder_marks(U.preview_buf(src)), 'no placeholder cells')
      a.eq(0, #U.transmissions(cap), 'no image data transmitted')
    end
  )

  QA.check(
    { 'R-185', 'R-240' },
    'With termguicolors off the preview runs in text-only mode with the reason termguicolors off, even when the terminal answers',
    function()
      local cap, dir = U.env({ termguicolors = false })
      local read = U.messages_reader()
      local src = doc_with_png(dir)
      U.answer_yes(cap)
      a.truthy(
        QA.wait(1500, function()
          return U.count_with(read(), 'termguicolors off') > 0
        end),
        'the reason termguicolors off reaches :messages'
      )
      U.settle(100)
      a.eq('text', U.mode(), 'text-only mode')
      a.eq(0, #U.placeholder_marks(U.preview_buf(src)), 'no placeholder cells')
      a.eq(0, #U.transmissions(cap), 'no image data transmitted')
      local box = U.mark_with(U.preview_buf(src), 'first picture')
      a.not_nil(box, 'the image shows as a box with its alt text')
    end
  )

  QA.check(
    'R-185',
    'After termguicolors is turned on, a refresh leaves text-only mode and shows images',
    function()
      local cap, dir = U.env({ termguicolors = false })
      local src = doc_with_png(dir)
      U.answer_yes(cap)
      a.truthy(U.wait_mode('text', 1500), 'text-only mode while termguicolors is off')
      vim.o.termguicolors = true
      vim.api.nvim_set_current_win(vim.fn.bufwinid(src))
      cap:clear()
      vim.cmd('KittyMarkdown refresh')
      U.answer_yes(cap)
      a.truthy(U.wait_mode('images'), 'image mode after the refresh')
      a.truthy(U.wait_placed(src, 1), 'the image shows')
    end
  )

  QA.check(
    { 'R-184', 'R-240' },
    'A terminal pixel size of zero gives text-only mode with the reason cell size unknown',
    function()
      local cap, dir = U.env()
      local transport = require('kitty-markdown.transport')
      transport.set_winsize_reader(function()
        return 24, 80, 0, 0
      end)
      local read = U.messages_reader()
      local src = doc_with_png(dir)
      U.answer_yes(cap)
      a.truthy(
        QA.wait(1500, function()
          return U.count_with(read(), 'cell size unknown') > 0
        end),
        'the reason cell size unknown reaches :messages'
      )
      U.settle(100)
      a.eq('text', U.mode(), 'text-only mode')
      a.eq(0, #U.placeholder_marks(U.preview_buf(src)), 'no placeholder cells')
      a.eq(0, #U.transmissions(cap), 'no image data transmitted')
    end
  )

  QA.check(
    'R-184',
    'A pixel size of zero in only one direction also gives text-only mode',
    function()
      local cap, dir = U.env()
      local transport = require('kitty-markdown.transport')
      transport.set_winsize_reader(function()
        return 24, 80, 800, 0
      end)
      local read = U.messages_reader()
      local src = doc_with_png(dir)
      U.answer_yes(cap)
      a.truthy(
        QA.wait(1500, function()
          return U.count_with(read(), 'cell size unknown') > 0
        end),
        'the reason cell size unknown reaches :messages'
      )
      U.settle(100)
      a.eq(0, #U.placeholder_marks(U.preview_buf(src)), 'no placeholder cells')
    end
  )

  QA.check('R-240', 'A positive graphics reply writes none of the text-only reasons', function()
    local cap, dir = U.env()
    local read = U.messages_reader()
    local src = doc_with_png(dir)
    U.answer_yes(cap)
    a.truthy(U.wait_placed(src, 1), 'placed')
    U.settle(600)
    local msgs = read()
    for _, r in ipairs({
      'no graphics reply',
      'probe timeout',
      'termguicolors off',
      'cell size unknown',
    }) do
      a.eq(0, U.count_with(msgs, r), 'reason ' .. r)
    end
  end)

  QA.check(
    'R-190',
    'Opening a preview returns at once while the probe waits, and the event loop keeps running',
    function()
      local _, dir = U.env({ helper = false })
      QA.make_png(dir .. '/a.png', 120, 70)
      local fired = false
      vim.defer_fn(function()
        fired = true
      end, 20)
      local t0 = vim.uv.hrtime()
      U.open_doc(dir, IMG_DOC)
      local ms = (vim.uv.hrtime() - t0) / 1e6
      a.le(250, ms, 'open time in ms with an unanswered probe')
      a.truthy(
        QA.wait(300, function()
          return fired
        end),
        'a timer callback runs while the probe waits'
      )
      a.ne('text', U.mode(), 'the probe is still waiting after the timer ran')
    end
  )

  QA.check('R-190', 'The probe answer is taken from the TermResponse event', function()
    local cap, dir = U.env()
    doc_with_png(dir)
    a.truthy(
      #vim.api.nvim_get_autocmds({ event = 'TermResponse' }) > 0,
      'a TermResponse autocmd while the probe waits'
    )
    U.answer_yes(cap)
    a.truthy(U.wait_mode('images'), 'the event answer turns images on')
  end)

  QA.check(
    'R-239',
    'The first probe result writes the mode and its reason to :messages once',
    function()
      local _, dir = U.env({ helper = false })
      local read = U.messages_reader()
      doc_with_png(dir)
      U.answer_no()
      a.truthy(U.wait_mode('text'), 'text-only mode')
      U.settle(700)
      local msgs = read()
      a.eq(1, U.count_with(msgs, 'no graphics reply'), 'messages with the reason')
      local line
      for _, l in ipairs(msgs) do
        if l:find('no graphics reply', 1, true) then
          line = l
        end
      end
      a.matches('[Tt]ext', line, 'the message names the text-only mode')
      vim.cmd('tabnew')
      U.open_doc(dir, { '# Other', '', '![x](a.png)' }, 'other.md')
      U.settle(100)
      a.eq(1, U.count_with(read(), 'no graphics reply'), 'messages after a second preview opens')
    end
  )

  QA.check(
    'R-239',
    'A re-probe with the same result writes no new mode message; a changed result writes one',
    function()
      local cap, dir = U.env()
      local read = U.messages_reader()
      local src = doc_with_png(dir)
      U.answer_no()
      a.truthy(U.wait_mode('text'), 'text-only mode')
      U.settle(50)
      vim.api.nvim_set_current_win(vim.fn.bufwinid(src))
      cap:clear()
      vim.cmd('KittyMarkdown refresh')
      U.answer_no()
      U.settle(700)
      local after_same = read()
      a.eq(
        1,
        U.count_with(after_same, 'no graphics reply'),
        'reason messages after an unchanged re-probe'
      )
      cap:clear()
      vim.cmd('KittyMarkdown refresh')
      U.answer_yes(cap)
      a.truthy(U.wait_mode('images'), 'image mode after the second re-probe')
      U.settle(100)
      local msgs = read()
      local new = vim.list_slice(msgs, #after_same + 1)
      local mode_lines = 0
      for _, l in ipairs(new) do
        local low = l:lower()
        if
          low:find('image', 1, true)
          or low:find('graphics', 1, true)
          or low:find('pixel', 1, true)
        then
          mode_lines = mode_lines + 1
        end
      end
      a.eq(1, mode_lines, 'one message names the new image mode')
    end
  )
end)
