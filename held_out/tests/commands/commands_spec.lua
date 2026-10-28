local QA = require('qa')
local U = require('commands_util')
local a = QA.assert
local api = vim.api

local DOC = {
  '# Preview Title',
  '',
  'First paragraph with *emphasis* and `code`.',
  '',
  '- item one',
  '- item two',
  '',
  'Closing words here.',
}

local function wins()
  return #api.nvim_list_wins()
end

local function tabs()
  return #api.nvim_list_tabpages()
end

QA.describe('commands: the command and the module', function()
  QA.before_each(function()
    U.reset()
  end)

  QA.check('R-001', 'The :KittyMarkdown user command exists once the plugin is loaded', function()
    local cmds = api.nvim_get_commands({})
    a.not_nil(cmds.KittyMarkdown, 'user command')
  end)

  QA.check('R-001', 'Each of the four subcommands runs without a command error', function()
    U.src(DOC)
    for _, sub in ipairs({ 'open', 'refresh', 'toggle', 'toggle', 'close' }) do
      a.no_error(function()
        vim.cmd('KittyMarkdown ' .. sub)
      end, sub)
      local p = U.preview()
      if p and sub ~= 'close' then
        -- go back to the source window for the next subcommand
        for _, w in ipairs(api.nvim_list_wins()) do
          if vim.bo[api.nvim_win_get_buf(w)].filetype == 'markdown' then
            api.nvim_set_current_win(w)
          end
        end
      end
    end
  end)

  QA.check(
    'R-001',
    'Command completion offers only the four subcommands',
    { severity = 'low' },
    function()
      local got = vim.fn.getcompletion('KittyMarkdown ', 'cmdline')
      for _, c in ipairs(got) do
        a.contains(c, { 'open', 'close', 'toggle', 'refresh' }, 'completion candidate')
      end
      local after_open = vim.fn.getcompletion('KittyMarkdown open ', 'cmdline')
      for _, c in ipairs(after_open) do
        a.contains(c, { 'split', 'tab', 'float' }, 'placement completion candidate')
      end
    end
  )

  QA.check(
    'R-013',
    'The Lua module exposes setup, open, close, toggle, refresh, and layout as functions',
    function()
      local km = require('kitty-markdown')
      for _, f in ipairs({ 'setup', 'open', 'close', 'toggle', 'refresh', 'layout' }) do
        a.eq('function', type(km[f]), f)
      end
    end
  )

  QA.check(
    'R-013',
    'The Lua functions open, toggle, and close drive the preview like the command',
    function()
      local km = require('kitty-markdown')
      U.src(DOC)
      km.open('split')
      a.eq(1, #U.previews(), 'open() shows a preview')
      km.close()
      U.settle(30)
      a.eq(0, #U.previews(), 'close() closes it')
      km.toggle('float')
      a.eq(1, #U.previews(), 'toggle() opens a preview')
    end
  )
end)

QA.describe('commands: open and placement', function()
  QA.before_each(function()
    U.reset()
  end)

  QA.check(
    'R-002',
    'Open in a markdown buffer shows a preview window with the rendered document text',
    function()
      U.src(DOC)
      vim.cmd('KittyMarkdown open')
      local p, n = U.preview()
      a.eq(1, n, 'preview windows')
      local text = U.text(p.buf)
      a.contains('Preview Title', text)
      a.contains('First paragraph with', text)
      a.contains('Closing words here.', text)
      a.not_contains('*emphasis*', text, 'rendered, not raw markdown')
      a.eq(p.tab, api.nvim_get_current_tabpage(), 'the preview is in the current tabpage')
    end
  )

  QA.check(
    'R-002',
    'The preview shows the buffer contents, including unsaved edits',
    { severity = 'medium' },
    function()
      local buf = U.src(DOC)
      api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'Unsaved addition paragraph.' })
      vim.cmd('KittyMarkdown open split')
      local p = U.preview()
      a.contains('Unsaved addition paragraph.', U.text(p.buf))
    end
  )

  QA.check('R-003', 'The split placement opens a normal window in the same tabpage', function()
    U.src(DOC)
    local t0, w0 = tabs(), wins()
    vim.cmd('KittyMarkdown open split')
    local p = U.preview()
    a.not_nil(p, 'preview window')
    a.eq(t0, tabs(), 'tabpage count')
    a.eq(w0 + 1, wins(), 'window count')
    a.eq(api.nvim_get_current_tabpage(), p.tab)
    a.eq('', api.nvim_win_get_config(p.win).relative, 'not a float')
  end)

  QA.check('R-003', 'The tab placement opens the preview in a new tabpage', function()
    U.src(DOC)
    local t0 = tabs()
    local first = api.nvim_get_current_tabpage()
    vim.cmd('KittyMarkdown open tab')
    local p = U.preview()
    a.not_nil(p, 'preview window')
    a.eq(t0 + 1, tabs(), 'tabpage count')
    a.ne(first, p.tab, 'preview in a new tabpage')
  end)

  QA.check('R-003', 'The float placement opens the preview in a floating window', function()
    U.src(DOC)
    local t0 = tabs()
    vim.cmd('KittyMarkdown open float')
    local p = U.preview()
    a.not_nil(p, 'preview window')
    a.ne('', api.nvim_win_get_config(p.win).relative, 'floating window')
    a.eq(t0, tabs(), 'tabpage count')
  end)

  QA.check('R-003', 'Toggle accepts each placement argument', function()
    for _, place in ipairs({ 'split', 'tab', 'float' }) do
      U.reset()
      U.src(DOC)
      local t0 = tabs()
      vim.cmd('KittyMarkdown toggle ' .. place)
      local p = U.preview()
      a.not_nil(p, place .. ': preview window')
      local float = api.nvim_win_get_config(p.win).relative ~= ''
      a.eq(place == 'float', float, place .. ': floating')
      a.eq(place == 'tab' and t0 + 1 or t0, tabs(), place .. ': tabpage count')
    end
  end)

  QA.check(
    'R-004',
    'With no placement argument and no setup option, the preview opens in a new tabpage',
    function()
      U.reset({})
      -- a fresh setup without a placement option
      U.src(DOC)
      local t0 = tabs()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      a.not_nil(p, 'preview window')
      a.eq(t0 + 1, tabs(), 'default placement is a tab')
    end
  )

  QA.check(
    'R-004',
    'With no placement argument, open and toggle use the configured placement',
    function()
      U.reset({ placement = 'split' })
      U.src(DOC)
      local t0 = tabs()
      vim.cmd('KittyMarkdown open')
      local p = U.preview()
      a.eq(t0, tabs(), 'split: no new tabpage')
      a.eq('', api.nvim_win_get_config(p.win).relative, 'split: not a float')
      U.reset({ placement = 'float' })
      U.src(DOC)
      vim.cmd('KittyMarkdown toggle')
      p = U.preview()
      a.not_nil(p, 'float preview')
      a.ne('', api.nvim_win_get_config(p.win).relative, 'configured float placement')
    end
  )

  QA.check('R-004', 'A placement argument wins over the configured placement', function()
    U.reset({ placement = 'float' })
    U.src(DOC)
    local t0 = tabs()
    vim.cmd('KittyMarkdown open tab')
    a.eq(t0 + 1, tabs(), 'tab argument over float config')
  end)
end)

QA.describe('commands: bad subcommands and arguments', function()
  QA.before_each(function()
    U.reset()
  end)

  local function to_source(buf)
    for _, w in ipairs(api.nvim_list_wins()) do
      if api.nvim_win_get_buf(w) == buf then
        api.nvim_set_current_win(w)
      end
    end
  end

  QA.check(
    'R-249',
    'An unknown subcommand shows the unknown subcommand error with its name in quotes and opens nothing',
    function()
      U.src(DOC)
      local t0, w0 = tabs(), wins()
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown frobnicate')
      end)
      a.contains('kitty-markdown: unknown subcommand "frobnicate"', U.msgs(msgs))
      a.eq(t0, tabs(), 'tabpage count')
      a.eq(w0, wins(), 'window count')
      a.eq(0, #U.preview_bufs(), 'no preview')
    end
  )

  QA.check(
    'R-249',
    'Subcommands are matched exactly, so a subcommand in another letter case is unknown',
    { severity = 'medium' },
    function()
      U.src(DOC)
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown Open')
      end)
      a.contains('kitty-markdown: unknown subcommand "Open"', U.msgs(msgs))
      a.eq(0, #U.preview_bufs(), 'no preview')
    end
  )

  QA.check(
    'R-249',
    'An unknown subcommand leaves an open preview as it is',
    { severity = 'medium' },
    function()
      local buf = U.src(DOC)
      vim.cmd('KittyMarkdown open split')
      local p = U.preview()
      api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'Unwritten tail text.' })
      to_source(buf)
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown reload')
      end)
      a.contains('kitty-markdown: unknown subcommand "reload"', U.msgs(msgs))
      a.eq(1, #U.previews(), 'preview still open')
      a.not_contains('Unwritten tail text.', U.text(p.buf), 'no re-render')
    end
  )

  QA.check(
    'R-250',
    'A placement other than split, tab, or float shows the unexpected argument error and opens nothing',
    function()
      U.src(DOC)
      local t0, w0 = tabs(), wins()
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown open sideways')
      end)
      a.contains('kitty-markdown: unexpected argument "sideways"', U.msgs(msgs))
      a.eq(t0, tabs(), 'tabpage count')
      a.eq(w0, wins(), 'window count')
      a.eq(0, #U.preview_bufs(), 'no preview')
      msgs = U.collect(function()
        vim.cmd('KittyMarkdown toggle Split')
      end)
      a.contains('kitty-markdown: unexpected argument "Split"', U.msgs(msgs))
      a.eq(0, #U.preview_bufs(), 'toggle: no preview')
    end
  )

  QA.check(
    'R-250',
    'A placement after close shows the unexpected argument error and keeps the preview open',
    function()
      local buf = U.src(DOC)
      vim.cmd('KittyMarkdown open split')
      to_source(buf)
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown close split')
      end)
      a.contains('kitty-markdown: unexpected argument "split"', U.msgs(msgs))
      a.eq(1, #U.previews(), 'preview still open')
    end
  )

  QA.check(
    'R-250',
    'A placement after refresh shows the unexpected argument error and does not re-render',
    function()
      local buf = U.src(DOC)
      vim.cmd('KittyMarkdown open split')
      local p = U.preview()
      api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'Only a refresh shows this.' })
      to_source(buf)
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown refresh tab')
      end)
      a.contains('kitty-markdown: unexpected argument "tab"', U.msgs(msgs))
      a.not_contains('Only a refresh shows this.', U.text(p.buf), 'no re-render')
    end
  )

  QA.check(
    'R-250',
    'A third argument shows the unexpected argument error naming it and opens nothing',
    function()
      U.src(DOC)
      local t0, w0 = tabs(), wins()
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown open split extra')
      end)
      a.contains('kitty-markdown: unexpected argument "extra"', U.msgs(msgs))
      a.eq(t0, tabs(), 'tabpage count')
      a.eq(w0, wins(), 'window count')
      a.eq(0, #U.preview_bufs(), 'no preview')
    end
  )

  QA.check('R-250', 'A third argument to toggle closes nothing', { severity = 'medium' }, function()
    local buf = U.src(DOC)
    vim.cmd('KittyMarkdown open split')
    to_source(buf)
    local msgs = U.collect(function()
      vim.cmd('KittyMarkdown toggle float again')
    end)
    a.contains('kitty-markdown: unexpected argument "again"', U.msgs(msgs))
    a.eq(1, #U.previews(), 'preview still open')
  end)

  QA.check('R-251', 'The command without a subcommand is rejected by Neovim with E471', function()
    U.src(DOC)
    local ok, err = pcall(vim.cmd, 'KittyMarkdown')
    a.falsy(ok, 'command rejected')
    a.matches('E471', tostring(err))
    a.eq(0, #U.preview_bufs(), 'no preview')
  end)
end)

QA.describe('commands: source buffer untouched', function()
  QA.before_each(function()
    U.reset()
  end)

  local function snapshot(buf)
    local marks = {}
    for name, id in pairs(api.nvim_get_namespaces()) do
      marks[name] = api.nvim_buf_get_extmarks(buf, id, 0, -1, { details = true })
    end
    local all = api.nvim_buf_get_extmarks(buf, -1, 0, -1, { details = true })
    return {
      lines = U.lines(buf),
      tick = api.nvim_buf_get_changedtick(buf),
      modified = vim.bo[buf].modified,
      modifiable = vim.bo[buf].modifiable,
      filetype = vim.bo[buf].filetype,
      all_marks = all,
      marks = marks,
    }
  end

  local function source_win(buf)
    for _, w in ipairs(api.nvim_list_wins()) do
      if api.nvim_win_get_buf(w) == buf then
        return w
      end
    end
  end

  QA.check(
    'R-005',
    'Opening, refreshing, and closing a preview leaves the source text, changedtick, and extmarks unchanged',
    function()
      local buf = U.src({
        '# Head',
        '',
        'Text with [a link](https://example.org) and **bold**.',
        '',
        '```lua',
        'local x = 1',
        '```',
        '',
        '| a | b |',
        '| - | - |',
        '| 1 | 2 |',
        '',
        '![pic](missing.png)',
      })
      local before = snapshot(buf)
      vim.cmd('KittyMarkdown open split')
      U.settle(200)
      a.eq(before, snapshot(buf), 'after open')
      api.nvim_set_current_win(source_win(buf))
      vim.cmd('KittyMarkdown refresh')
      U.settle(200)
      a.eq(before, snapshot(buf), 'after refresh')
      vim.cmd('KittyMarkdown close')
      U.settle(100)
      a.eq(before, snapshot(buf), 'after close')
    end
  )

  QA.check(
    'R-005',
    'The preview does not add highlights or matches to the source window',
    { severity = 'medium' },
    function()
      local buf = U.src({ '# Head', '', 'Some *text* here.', '', '> quote' })
      local win = api.nvim_get_current_win()
      local matches = vim.fn.getmatches(win)
      local whl = vim.wo[win].winhighlight
      local conceal = vim.wo[win].conceallevel
      vim.cmd('KittyMarkdown open split')
      U.settle(150)
      a.eq(matches, vim.fn.getmatches(win), 'window matches')
      a.eq(whl, vim.wo[win].winhighlight, 'source winhighlight')
      a.eq(conceal, vim.wo[win].conceallevel, 'source conceallevel')
      a.eq({}, api.nvim_buf_get_extmarks(buf, -1, 0, -1, {}), 'no extmarks in the source')
    end
  )

  QA.check(
    'R-005',
    'Writing the source with a preview open adds no extmarks to the source buffer',
    function()
      local buf = U.src(DOC)
      vim.cmd('KittyMarkdown open split')
      api.nvim_set_current_win(source_win(buf))
      api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'More text.' })
      local lines = U.lines(buf)
      U.write(buf)
      U.settle(200)
      a.eq(lines, U.lines(buf), 'source lines')
      a.eq({}, api.nvim_buf_get_extmarks(buf, -1, 0, -1, {}), 'source extmarks')
    end
  )
end)

QA.describe('commands: one preview per source', function()
  QA.before_each(function()
    U.reset()
  end)

  QA.check('R-006', 'Opening twice keeps one preview for the source buffer', function()
    local buf = U.src(DOC)
    vim.cmd('KittyMarkdown open split')
    api.nvim_set_current_buf(buf)
    for _, w in ipairs(api.nvim_list_wins()) do
      if api.nvim_win_get_buf(w) == buf then
        api.nvim_set_current_win(w)
      end
    end
    vim.cmd('KittyMarkdown open split')
    a.eq(1, #U.previews(), 'preview windows')
    a.eq(1, #U.preview_bufs(), 'preview buffers')
  end)

  QA.check(
    'R-006',
    'Opening again with another placement does not make a second preview',
    function()
      local buf = U.src(DOC)
      vim.cmd('KittyMarkdown open tab')
      vim.cmd('tabfirst')
      api.nvim_set_current_buf(buf)
      vim.cmd('KittyMarkdown open float')
      vim.cmd('tabfirst')
      vim.cmd('KittyMarkdown open split')
      a.eq(1, #U.preview_bufs(), 'preview buffers')
      a.eq(1, #U.previews(), 'preview windows')
    end
  )

  QA.check(
    'R-006',
    'Two source buffers get two separate previews',
    { severity = 'medium' },
    function()
      U.src(DOC, { name = 'one.md' })
      vim.cmd('KittyMarkdown open tab')
      vim.cmd('tabfirst')
      U.src({ '# Other', '', 'Second document body.' }, { name = 'two.md' })
      vim.cmd('KittyMarkdown open tab')
      a.eq(2, #U.preview_bufs(), 'preview buffers')
      local texts = {}
      for _, b in ipairs(U.preview_bufs()) do
        texts[#texts + 1] = U.text(b)
      end
      local joined = table.concat(texts, '\n')
      a.contains('Preview Title', joined)
      a.contains('Second document body.', joined)
    end
  )

  QA.check('R-007', 'Open with a preview already present focuses that preview (split)', function()
    local buf = U.src(DOC)
    local src_win = api.nvim_get_current_win()
    vim.cmd('KittyMarkdown open split')
    local p = U.preview()
    api.nvim_set_current_win(src_win)
    local w0 = wins()
    vim.cmd('KittyMarkdown open')
    a.eq(p.win, api.nvim_get_current_win(), 'focus moved to the existing preview')
    a.eq(w0, wins(), 'no new window')
    a.eq(buf, api.nvim_win_get_buf(src_win), 'source window kept')
  end)

  QA.check(
    'R-007',
    'Open with a preview in another tabpage switches to that tabpage and window',
    function()
      U.src(DOC)
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      vim.cmd('tabfirst')
      local t0 = tabs()
      vim.cmd('KittyMarkdown open tab')
      a.eq(t0, tabs(), 'no new tabpage')
      a.eq(p.win, api.nvim_get_current_win(), 'focus on the existing preview')
    end
  )
end)

QA.describe('commands: close and toggle', function()
  QA.before_each(function()
    U.reset()
  end)

  QA.check('R-008', 'Close removes the preview window and the preview buffer', function()
    U.src(DOC)
    vim.cmd('KittyMarkdown open split')
    local p = U.preview()
    vim.cmd('KittyMarkdown close')
    U.settle(50)
    a.falsy(api.nvim_win_is_valid(p.win), 'preview window closed')
    a.falsy(api.nvim_buf_is_valid(p.buf) and api.nvim_buf_is_loaded(p.buf), 'preview buffer gone')
    a.eq(0, #U.previews())
  end)

  QA.check(
    { 'R-008', 'R-252' },
    'Closing a tab preview deletes the preview buffer and closes its tab page',
    function()
      local buf = U.src(DOC)
      local t0 = tabs()
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      vim.cmd('tabfirst')
      vim.cmd('KittyMarkdown close')
      U.settle(50)
      a.eq(0, #U.preview_bufs(), 'preview buffer deleted')
      a.truthy(api.nvim_buf_is_loaded(buf), 'source still loaded')
      a.eq(t0, tabs(), 'the tab page of the preview is closed')
      a.falsy(api.nvim_win_is_valid(p.win), 'preview window closed')
    end
  )

  QA.check('R-008', 'Close deletes every image of the preview from the terminal by id', function()
    local cap = U.image_mode()
    local dir = QA.tmpdir()
    QA.make_png(dir .. '/a.png', 60, 30)
    QA.make_png(dir .. '/b.png', 50, 40)
    U.src({ '# Pics', '', 'one', '', '![a](a.png)', '', 'two', '', '![b](b.png)' }, { dir = dir })
    vim.cmd('KittyMarkdown open split')
    local p = U.preview()
    a.truthy(
      QA.wait(8000, function()
        local n = 0
        for _, v in ipairs(U.virt(p.buf)) do
          if table.concat(v.rows):find(U.PLACEHOLDER, 1, true) then
            n = n + 1
          end
        end
        return n == 2
      end),
      'both pictures placed'
    )
    local ids = U.transmits(cap)
    a.ge(2, #ids, 'transmitted ids')
    cap:clear()
    vim.cmd('KittyMarkdown close')
    U.settle(50)
    local deleted = U.deletes(cap)
    for _, id in ipairs(ids) do
      a.contains(id, deleted, 'image ' .. id .. ' deleted on close')
    end
  end)

  QA.check(
    'R-008',
    'Close in a buffer without a preview shows no error and opens nothing',
    { severity = 'low' },
    function()
      U.src(DOC)
      local w0 = wins()
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown close')
      end)
      a.eq(w0, wins())
      for _, m in ipairs(msgs) do
        a.falsy(m.raised, 'no Lua error raised: ' .. m.msg)
      end
    end
  )

  QA.check('R-009', 'Toggle closes an existing preview', function()
    U.src(DOC)
    vim.cmd('KittyMarkdown open split')
    local p = U.preview()
    vim.cmd('KittyMarkdown toggle')
    U.settle(50)
    a.falsy(api.nvim_win_is_valid(p.win), 'preview window closed')
    a.eq(0, #U.previews())
  end)

  QA.check('R-010', 'Toggle opens a preview when none exists', function()
    U.src(DOC)
    vim.cmd('KittyMarkdown toggle split')
    local p, n = U.preview()
    a.eq(1, n)
    a.contains('Preview Title', U.text(p.buf))
  end)

  QA.check({ 'R-009', 'R-010' }, 'Repeated toggles alternate between open and closed', function()
    local buf = U.src(DOC)
    for i = 1, 4 do
      for _, w in ipairs(api.nvim_list_wins()) do
        if api.nvim_win_get_buf(w) == buf then
          api.nvim_set_current_win(w)
        end
      end
      vim.cmd('KittyMarkdown toggle split')
      U.settle(30)
      a.eq(i % 2, #U.previews(), 'after toggle ' .. i)
    end
  end)
end)

QA.describe('commands: source unload and delete', function()
  QA.before_each(function()
    U.reset()
  end)

  for _, how in ipairs({ 'bdelete!', 'bunload!', 'bwipeout!' }) do
    QA.check(
      'R-011',
      'Deleting, unloading, or wiping the source buffer closes its preview',
      function()
        -- another listed, loaded buffer, so the source is not the last one
        local other = api.nvim_create_buf(true, false)
        vim.fn.bufload(other)
        local buf = U.src(DOC)
        vim.cmd('KittyMarkdown open split')
        local p = U.preview()
        vim.cmd(how .. ' ' .. buf)
        U.settle(100)
        -- the window may stay when it is the last one, but it must not show
        -- the preview any more
        a.falsy(
          api.nvim_win_is_valid(p.win) and api.nvim_win_get_buf(p.win) == p.buf,
          how .. ': preview window closed'
        )
        a.eq(0, #U.previews(), how .. ': no window shows a preview')
        a.eq(0, #U.preview_bufs(), how .. ': preview buffer gone')
      end
    )
  end

  -- A named, loaded buffer with text, so an empty buffer is told apart
  -- from it.
  local function other_buffer()
    local path = QA.tmpdir() .. '/other.txt'
    QA.write(path, 'other buffer text\n')
    local other = vim.fn.bufadd(path)
    vim.fn.bufload(other)
    vim.bo[other].buflisted = true
    return other
  end

  local function is_empty_buffer(b)
    return api.nvim_buf_get_name(b) == ''
      and vim.deep_equal({ '' }, api.nvim_buf_get_lines(b, 0, -1, false))
      and vim.bo[b].filetype ~= 'kitty-markdown'
  end

  for _, how in ipairs({ 'bdelete!', 'bunload!', 'bwipeout!' }) do
    QA.check(
      'R-347',
      'When the source is no longer loaded and the preview is the only window, the window shows an empty buffer',
      function()
        local other = other_buffer()
        local buf = U.src(DOC)
        vim.cmd('KittyMarkdown open split')
        local p = U.preview()
        vim.cmd(how .. ' ' .. buf)
        U.settle(100)
        a.eq(0, #U.preview_bufs(), how .. ': preview buffer deleted')
        a.eq(1, #api.nvim_list_wins(), how .. ': one window left')
        local w = api.nvim_list_wins()[1]
        local shown = api.nvim_win_get_buf(w)
        a.ne(other, shown, how .. ': not some other listed buffer')
        a.truthy(
          is_empty_buffer(shown),
          how .. ': empty buffer, got ' .. api.nvim_buf_get_name(shown)
        )
        a.eq(p.win, w, how .. ': in the window of the preview')
      end
    )
  end

  QA.check(
    'R-346',
    'Closing a preview that is the only window of the only tab page shows the source buffer in that window',
    function()
      other_buffer()
      local buf = U.src(DOC)
      vim.cmd('KittyMarkdown open split')
      local p = U.preview()
      api.nvim_set_current_win(p.win)
      vim.cmd('only')
      a.eq(1, #api.nvim_list_wins(), 'the preview is the only window')
      vim.cmd('KittyMarkdown close')
      U.settle(50)
      a.eq(0, #U.preview_bufs(), 'preview buffer deleted')
      a.truthy(api.nvim_win_is_valid(p.win), 'window stays')
      a.eq(buf, api.nvim_win_get_buf(p.win), 'shows the source buffer')
    end
  )

  QA.check(
    'R-346',
    'Pressing q in a preview that is the only window shows the source buffer in that window',
    { severity = 'medium' },
    function()
      local buf = U.src(DOC)
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      vim.cmd('tabonly')
      a.eq(1, #api.nvim_list_tabpages(), 'one tab page')
      a.eq(1, #api.nvim_list_wins(), 'one window')
      api.nvim_set_current_win(p.win)
      api.nvim_feedkeys('q', 'xt', false)
      U.settle(50)
      a.eq(0, #U.preview_bufs(), 'preview buffer deleted')
      a.truthy(api.nvim_win_is_valid(p.win), 'window stays')
      a.eq(buf, api.nvim_win_get_buf(p.win), 'shows the source buffer')
    end
  )

  QA.check(
    'R-252',
    'Toggling off a tab preview from inside it closes its tab page',
    { severity = 'medium' },
    function()
      local buf = U.src(DOC)
      local t0 = tabs()
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      api.nvim_set_current_win(p.win)
      vim.cmd('KittyMarkdown toggle')
      U.settle(50)
      a.eq(0, #U.preview_bufs(), 'preview buffer deleted')
      a.eq(t0, tabs(), 'tab page closed')
      a.falsy(api.nvim_win_is_valid(p.win), 'preview window closed')
      a.truthy(api.nvim_buf_is_loaded(buf), 'source still loaded')
    end
  )

  QA.check(
    'R-252',
    'Closing a preview that is the last window of a split tab page closes that tab page',
    { severity = 'medium' },
    function()
      local buf = U.src(DOC)
      vim.cmd('tabnew')
      api.nvim_set_current_buf(buf)
      local t0 = tabs()
      vim.cmd('KittyMarkdown open split')
      local p = U.preview()
      api.nvim_set_current_win(p.win)
      vim.cmd('only')
      vim.cmd('KittyMarkdown close')
      U.settle(50)
      a.eq(0, #U.preview_bufs(), 'preview buffer deleted')
      a.eq(t0 - 1, tabs(), 'that tab page closed')
    end
  )

  QA.check(
    'R-008',
    'Closing a split preview that is not the last window closes its window',
    { severity = 'medium' },
    function()
      U.src(DOC)
      local w0 = wins()
      vim.cmd('KittyMarkdown open split')
      local p = U.preview()
      vim.cmd('KittyMarkdown close')
      U.settle(50)
      a.falsy(api.nvim_win_is_valid(p.win), 'window closed')
      a.eq(w0, wins(), 'window count back')
    end
  )

  QA.check('R-011', 'Unloading the source closes a tab preview too', function()
    local buf = U.src(DOC)
    vim.cmd('KittyMarkdown open tab')
    vim.cmd('tabfirst')
    vim.cmd('bdelete! ' .. buf)
    U.settle(100)
    a.eq(0, #U.previews(), 'preview windows')
    a.eq(0, #U.preview_bufs(), 'preview buffers')
  end)

  QA.check(
    'R-011',
    'Deleting one source keeps the preview of another source',
    { severity = 'medium' },
    function()
      local one = U.src(DOC, { name = 'one.md' })
      vim.cmd('KittyMarkdown open tab')
      vim.cmd('tabfirst')
      U.src({ '# Other', '', 'Still here text.' }, { name = 'two.md' })
      vim.cmd('KittyMarkdown open tab')
      vim.cmd('tabfirst')
      vim.cmd('bdelete! ' .. one)
      U.settle(100)
      local bufs = U.preview_bufs()
      a.eq(1, #bufs, 'one preview left')
      a.contains('Still here text.', U.text(bufs[1]))
    end
  )
end)

QA.describe('commands: non-markdown buffers and old Neovim', function()
  QA.before_each(function()
    U.reset()
  end)

  QA.check(
    'R-012',
    'Open in a buffer that is not markdown shows an error and opens no preview',
    function()
      U.src({ 'local x = 1' }, { name = 'code.lua', ft = 'lua' })
      local w0, t0 = wins(), tabs()
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown open')
      end)
      a.eq(w0, wins(), 'window count')
      a.eq(t0, tabs(), 'tabpage count')
      a.eq(0, #U.preview_bufs(), 'no preview buffer')
      local err = false
      for _, m in ipairs(msgs) do
        if
          m.level == vim.log.levels.ERROR
          or m.raised
          or (m.from_messages and m.msg:find('kitty%-markdown'))
        then
          err = true
        end
      end
      a.truthy(err, 'an error message was shown')
    end
  )

  QA.check(
    'R-012',
    'Toggle and the Lua open in a buffer that is not markdown open no preview',
    { severity = 'medium' },
    function()
      U.src({ 'plain text' }, { name = 'notes.txt', ft = 'text' })
      local w0 = wins()
      U.collect(function()
        vim.cmd('KittyMarkdown toggle split')
      end)
      U.collect(function()
        require('kitty-markdown').open('float')
      end)
      a.eq(w0, wins(), 'window count')
      a.eq(0, #U.preview_bufs(), 'no preview buffer')
    end
  )

  QA.check(
    'R-165',
    'The error for a buffer that is not markdown names its filetype in the specified message',
    function()
      U.src({ 'local x = 1' }, { name = 'code.lua', ft = 'lua' })
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown open')
      end)
      a.contains('kitty-markdown: not a markdown buffer (filetype is "lua")', U.msgs(msgs))
    end
  )

  QA.check(
    'R-165',
    'The error for a buffer with no filetype shows an empty filetype in quotes',
    function()
      local buf = api.nvim_create_buf(true, false)
      api.nvim_set_current_buf(buf)
      vim.bo[buf].filetype = ''
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown open')
      end)
      a.contains('kitty-markdown: not a markdown buffer (filetype is "")', U.msgs(msgs))
      a.eq(0, #U.preview_bufs())
    end
  )

  local function fake_old_nvim()
    local orig_has, orig_version = vim.fn.has, vim.version
    vim.fn.has = function(f)
      if type(f) == 'string' and f:match('^nvim%-0%.1[2-9]') then
        return 0
      end
      return orig_has(f)
    end
    local old = setmetatable(
      { major = 0, minor = 11, patch = 5, prerelease = false },
      getmetatable(orig_version())
    )
    vim.version = setmetatable({}, {
      __index = orig_version,
      __call = function()
        return old
      end,
    })
    return function()
      vim.fn.has, vim.version = orig_has, orig_version
    end
  end

  QA.check(
    'R-166',
    'On Neovim older than 0.12, open shows the version message and opens no preview',
    function()
      U.src(DOC)
      local w0, t0 = wins(), tabs()
      local restore = fake_old_nvim()
      local ok, msgs = pcall(U.collect, function()
        vim.cmd('KittyMarkdown open split')
      end)
      restore()
      a.truthy(ok, tostring(msgs))
      a.contains('kitty-markdown: Neovim 0.12 or later is required', U.msgs(msgs))
      a.eq(w0, wins(), 'window count')
      a.eq(t0, tabs(), 'tabpage count')
      a.eq(0, #U.preview_bufs())
    end
  )

  QA.check(
    'R-166',
    'On Neovim older than 0.12, the Lua open and toggle also refuse with the version message',
    { severity = 'medium' },
    function()
      U.src(DOC)
      local w0 = wins()
      local restore = fake_old_nvim()
      local ok, msgs = pcall(U.collect, function()
        require('kitty-markdown').open('tab')
        require('kitty-markdown').toggle('float')
      end)
      restore()
      a.truthy(ok, tostring(msgs))
      a.contains('kitty-markdown: Neovim 0.12 or later is required', U.msgs(msgs))
      a.eq(w0, wins())
      a.eq(0, #U.preview_bufs())
    end
  )
end)

QA.describe('commands: preview buffer and window', function()
  QA.before_each(function()
    U.reset()
  end)

  for _, place in ipairs({ 'split', 'tab', 'float' }) do
    QA.check(
      'R-014',
      'The preview buffer has the kitty-markdown filetype and is not modifiable',
      function()
        U.src(DOC)
        vim.cmd('KittyMarkdown open ' .. place)
        local p = U.preview()
        a.eq('kitty-markdown', vim.bo[p.buf].filetype, place)
        a.falsy(vim.bo[p.buf].modifiable, place .. ': modifiable')
        a.errors(function()
          api.nvim_buf_set_lines(p.buf, 0, 1, false, { 'edit' })
        end, nil, place .. ': edit refused')
      end
    )
  end

  QA.check('R-014', 'The preview buffer stays not modifiable after a re-render', function()
    local buf = U.src(DOC)
    vim.cmd('KittyMarkdown open split')
    local p = U.preview()
    api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'Added.' })
    U.write(buf)
    U.settle(100)
    a.contains('Added.', U.text(p.buf), 'the write re-rendered')
    a.falsy(vim.bo[p.buf].modifiable)
    a.eq('kitty-markdown', vim.bo[p.buf].filetype)
  end)

  for _, place in ipairs({ 'split', 'tab', 'float' }) do
    QA.check(
      'R-015',
      'The preview window hides line numbers, the sign column, and the fold column, and does not wrap',
      function()
        vim.o.number = true
        vim.o.signcolumn = 'yes'
        vim.o.foldcolumn = '2'
        vim.o.wrap = true
        U.src(DOC)
        vim.cmd('KittyMarkdown open ' .. place)
        local p = U.preview()
        local wo = vim.wo[p.win]
        a.falsy(wo.number, place .. ': number')
        a.eq('no', wo.signcolumn, place .. ': signcolumn')
        a.eq('0', wo.foldcolumn, place .. ': foldcolumn')
        a.falsy(wo.wrap, place .. ': wrap')
        vim.o.number, vim.o.signcolumn, vim.o.foldcolumn, vim.o.wrap = false, 'auto', '0', true
      end
    )
  end

  for _, place in ipairs({ 'split', 'tab', 'float' }) do
    QA.check(
      'R-253',
      'The preview window hides the absolute and the relative line numbers',
      function()
        vim.o.relativenumber = true
        vim.o.number = true
        U.src(DOC)
        vim.cmd('KittyMarkdown open ' .. place)
        local p = U.preview()
        local rnu, nu = vim.wo[p.win].relativenumber, vim.wo[p.win].number
        vim.o.relativenumber, vim.o.number = false, false
        a.falsy(rnu, place .. ': relativenumber')
        a.falsy(nu, place .. ': number')
      end
    )
  end

  QA.check(
    'R-015',
    'The preview window options survive a re-render',
    { severity = 'medium' },
    function()
      vim.o.number = true
      local buf = U.src(DOC)
      vim.cmd('KittyMarkdown open split')
      local p = U.preview()
      api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'More.' })
      U.write(buf)
      U.settle(100)
      a.falsy(vim.wo[p.win].number)
      a.falsy(vim.wo[p.win].wrap)
      a.eq('no', vim.wo[p.win].signcolumn)
      vim.o.number = false
    end
  )

  QA.check(
    { 'R-167', 'R-254' },
    'The preview buffer is named kitty-markdown:// followed by the full path of the source',
    function()
      local _, path = U.src(DOC, { name = 'named-doc.md' })
      vim.cmd('KittyMarkdown open split')
      local p = U.preview()
      a.eq('kitty-markdown://' .. path, api.nvim_buf_get_name(p.buf), 'preview buffer name')
    end
  )

  QA.check(
    'R-254',
    'The preview name uses the full path even when the source was opened by a relative name',
    function()
      local dir = QA.tmpdir()
      dir = vim.uv.fs_realpath(dir) or dir
      QA.write(dir .. '/sub/rel-doc.md', table.concat(DOC, '\n') .. '\n')
      vim.fn.chdir(dir)
      vim.cmd('silent edit sub/rel-doc.md')
      vim.bo.filetype = 'markdown'
      vim.cmd('KittyMarkdown open split')
      local p = U.preview()
      local name = api.nvim_buf_get_name(p.buf)
      vim.fn.chdir(QA.root)
      a.eq('kitty-markdown://' .. dir .. '/sub/rel-doc.md', name)
    end
  )

  QA.check(
    'R-255',
    'A source buffer without a name gives the preview the bare kitty-markdown:// name',
    function()
      local buf = api.nvim_create_buf(true, false)
      api.nvim_buf_set_lines(buf, 0, -1, false, DOC)
      api.nvim_set_current_buf(buf)
      vim.bo[buf].filetype = 'markdown'
      vim.cmd('KittyMarkdown open split')
      local p = U.preview()
      a.not_nil(p, 'preview opened')
      a.eq('kitty-markdown://', api.nvim_buf_get_name(p.buf))
    end
  )

  QA.check(
    'R-167',
    'Previews of two sources have two distinct names that each carry their source name',
    { severity = 'medium' },
    function()
      local one = U.src(DOC, { name = 'alpha.md' })
      vim.cmd('KittyMarkdown open tab')
      vim.cmd('tabfirst')
      local two = U.src(DOC, { name = 'beta.md' })
      vim.cmd('KittyMarkdown open tab')
      local names = {}
      for _, b in ipairs(U.preview_bufs()) do
        names[#names + 1] = api.nvim_buf_get_name(b)
      end
      a.eq(2, #names)
      a.contains('kitty-markdown://' .. api.nvim_buf_get_name(one), names)
      a.contains('kitty-markdown://' .. api.nvim_buf_get_name(two), names)
    end
  )
end)

QA.describe('commands: run inside the preview window', function()
  QA.before_each(function()
    U.reset()
  end)

  QA.check(
    'R-215',
    'Close run in the preview window closes that preview without a filetype error',
    function()
      U.src(DOC)
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      api.nvim_set_current_win(p.win)
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown close')
      end)
      U.settle(50)
      a.eq(0, #U.previews(), 'preview closed')
      for _, m in ipairs(msgs) do
        a.no_match('not a markdown buffer', m.msg)
      end
    end
  )

  QA.check('R-215', 'Toggle run in the preview window closes that preview', function()
    U.src(DOC)
    vim.cmd('KittyMarkdown open float')
    local p = U.preview()
    api.nvim_set_current_win(p.win)
    local msgs = U.collect(function()
      vim.cmd('KittyMarkdown toggle')
    end)
    U.settle(50)
    a.eq(0, #U.previews(), 'preview closed')
    a.eq(0, #U.preview_bufs(), 'no second preview of the preview')
    for _, m in ipairs(msgs) do
      a.no_match('not a markdown buffer', m.msg)
    end
  end)

  QA.check(
    'R-215',
    'Refresh run in the preview window re-renders from the source buffer',
    function()
      local buf = U.src(DOC)
      vim.cmd('KittyMarkdown open tab')
      local p = U.preview()
      api.nvim_buf_set_lines(buf, -1, -1, false, { '', 'Text added before refresh.' })
      api.nvim_set_current_win(p.win)
      local msgs = U.collect(function()
        vim.cmd('KittyMarkdown refresh')
      end)
      U.settle(100)
      a.contains('Text added before refresh.', U.text(p.buf), 'refreshed from the source')
      for _, m in ipairs(msgs) do
        a.no_match('not a markdown buffer', m.msg)
      end
    end
  )
end)
