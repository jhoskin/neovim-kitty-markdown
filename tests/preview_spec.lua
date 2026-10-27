local config = require('kitty-markdown.config')
local cache = require('kitty-markdown.cache')
local theme = require('kitty-markdown.theme')
local transport = require('kitty-markdown.transport')
local helper = require('kitty-markdown.helper')
local preview = require('kitty-markdown.preview')

local ESC = '\27'
local ST = ESC .. '\\'
local APC = ESC .. '_G'
local PLACEHOLDER = vim.fn.nr2char(0x10EEEE, 1)

local here = vim.fn.fnamemodify(debug.getinfo(1, 'S').source:sub(2), ':p')
local root = vim.fs.dirname(vim.fs.dirname(here))
local fixtures = root .. '/tests/fixtures/preview'
local fake = fixtures .. '/fake-helper'

local function wait_for(pred, ms)
  return vim.wait(ms or 5000, pred, 10)
end

-- Let scheduled callbacks run without waiting for a condition.
local function settle(ms)
  vim.wait(ms or 50, function()
    return false
  end, 10)
end

local function fire(sequence)
  vim.api.nvim_exec_autocmds('TermResponse', { data = { sequence = sequence } })
end

local function be32(n)
  return string.char(
    math.floor(n / 2 ^ 24) % 256,
    math.floor(n / 2 ^ 16) % 256,
    math.floor(n / 256) % 256,
    n % 256
  )
end

-- A PNG with a real IHDR chunk, which is all the fake helper and the
-- transport read.
local function write_png(path, width, height)
  local data = '\137PNG\r\n\26\n'
    .. be32(13)
    .. 'IHDR'
    .. be32(width)
    .. be32(height)
    .. '\8\6\0\0\0'
    .. be32(0)
    .. string.rep('x', 300)
  local f = assert(io.open(path, 'wb'))
  f:write(data)
  f:close()
  return path, data
end

local function ns_id()
  return vim.api.nvim_get_namespaces()['kitty-markdown-preview']
end

describe('preview', function()
  local tmp, png, out, notices, real_notify, real_open, opened

  local function make_buf(lines, name)
    local path = tmp .. '/' .. (name or 'doc.md')
    vim.fn.writefile(lines, path)
    local buf = vim.fn.bufadd(path)
    vim.fn.bufload(buf)
    vim.bo[buf].filetype = 'markdown'
    vim.api.nvim_win_set_buf(0, buf)
    return buf
  end

  local function fixture_buf()
    return make_buf(vim.fn.readfile(fixtures .. '/doc.md'))
  end

  local function answer_probe(images)
    if images then
      fire(APC .. 'i=31;OK' .. ST)
    else
      fire(ESC .. '[?62;c')
    end
    assert.is_true(wait_for(function()
      return preview.mode() ~= nil
    end))
  end

  -- The state entry and descriptor of the n-th pixel block in model order.
  local function block(buf, n)
    local p = preview.get(buf)
    local desc = p.model.blocks[n]
    return p.blocks[desc.key], desc
  end

  local function wait_state(buf, n, state)
    assert.is_true(
      wait_for(function()
        local b = block(buf, n)
        return b ~= nil and b.state == state
      end),
      'block ' .. n .. ' never reached ' .. state
    )
    return block(buf, n)
  end

  -- The virtual lines below a block's anchor, one string per row.
  local function rows_of(buf, b)
    local p = preview.get(buf)
    local mark =
      vim.api.nvim_buf_get_extmark_by_id(p.buf, ns_id(), b.extmark_id, { details = true })
    local rows = {}
    for _, row in ipairs(mark[3].virt_lines or {}) do
      local parts = {}
      for _, chunk in ipairs(row) do
        parts[#parts + 1] = chunk[1]
      end
      rows[#rows + 1] = table.concat(parts)
    end
    return rows, mark
  end

  local function bytes_matching(pattern)
    local n = 0
    for _, chunk in ipairs(out) do
      if chunk:find(pattern) then
        n = n + 1
      end
    end
    return n
  end

  local function notices_at(level)
    local got = {}
    for _, n in ipairs(notices) do
      if n.level == level then
        got[#got + 1] = n.msg
      end
    end
    return got
  end

  before_each(function()
    tmp = vim.fn.tempname()
    vim.fn.mkdir(tmp, 'p')
    -- Buffer names carry the resolved path, so the expected names must too.
    tmp = vim.uv.fs_realpath(tmp)
    png = write_png(tmp .. '/pic.png', 400, 200)
    vim.env.KM_NVIM = vim.v.progpath
    vim.env.KM_PREVIEW_PNG = png
    vim.env.KM_FAKE_CACHED = nil
    vim.env.KM_FAKE_FETCHED = nil
    vim.env.KM_FAKE_PREPARED = nil
    vim.env.TMUX = nil
    vim.o.termguicolors = true
    vim.o.background = 'light'
    config.setup({ placement = 'split' })
    cache.reset()
    theme.reset()
    preview.reset()
    helper.reset()
    transport.reset()
    helper.set_locator(function(cb)
      cb(fake, 'user')
    end)
    out = {}
    transport.set_writer(function(bytes)
      out[#out + 1] = bytes
    end)
    transport.set_winsize_reader(function()
      return 24, 80, 1600, 480
    end)
    notices = {}
    real_notify = vim.notify
    vim.notify = function(msg, level)
      notices[#notices + 1] = { msg = msg, level = level }
    end
    opened = {}
    real_open = vim.ui.open
    vim.ui.open = function(target)
      opened[#opened + 1] = target
      return nil, nil
    end
  end)

  after_each(function()
    preview.reset()
    helper.reset()
    transport.reset()
    theme.reset()
    vim.notify = real_notify
    vim.ui.open = real_open
    vim.cmd('silent! tabonly')
    vim.cmd('silent! only')
    for _, buf in ipairs(vim.api.nvim_list_bufs()) do
      if vim.api.nvim_buf_get_name(buf):find(tmp, 1, true) then
        pcall(vim.api.nvim_buf_delete, buf, { force = true })
      end
    end
    vim.fn.delete(tmp, 'rf')
    vim.o.background = 'light'
    config.setup({})
  end)

  describe('open (R-002, R-006, R-007, R-012, R-014, R-015, R-165, R-167)', function()
    it('creates a scratch preview buffer with the documented options', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      assert.is_not_nil(p)
      assert.are.equal('kitty-markdown://' .. tmp .. '/doc.md', vim.api.nvim_buf_get_name(p.buf))
      assert.are.equal('kitty-markdown', vim.bo[p.buf].filetype)
      assert.are.equal('nofile', vim.bo[p.buf].buftype)
      assert.are.equal('wipe', vim.bo[p.buf].bufhidden)
      assert.is_false(vim.bo[p.buf].modifiable)
      assert.is_false(vim.bo[p.buf].swapfile)
      assert.is_false(vim.wo[p.win].number)
      assert.is_false(vim.wo[p.win].relativenumber)
      assert.is_false(vim.wo[p.win].wrap)
      assert.are.equal('no', vim.wo[p.win].signcolumn)
      assert.are.equal('0', vim.wo[p.win].foldcolumn)
      assert.are.equal('Normal:KittyMarkdownText', vim.wo[p.win].winhighlight)
      assert.are.equal(buf, vim.api.nvim_win_get_buf(0), 'the source keeps the focus')
      assert.are.equal(p, preview.get(buf))
      assert.are.equal(p, preview.get(p.buf), 'the preview buffer resolves to its source')
    end)

    it('keeps one preview per source and focuses it on a second open', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      local again = preview.open(buf, 'split')
      assert.are.equal(p, again)
      assert.are.equal(p.win, vim.api.nvim_get_current_win())
      local count = 0
      for _, w in ipairs(vim.api.nvim_list_wins()) do
        if vim.api.nvim_win_get_buf(w) == p.buf then
          count = count + 1
        end
      end
      assert.are.equal(1, count)
    end)

    it('refuses a buffer whose filetype is not markdown', function()
      local buf = vim.api.nvim_create_buf(false, true)
      vim.bo[buf].filetype = 'text'
      assert.is_nil(preview.open(buf, 'split'))
      assert.is_nil(preview.get(buf))
      assert.are.same(
        { 'kitty-markdown: not a markdown buffer (filetype is "text")' },
        notices_at(vim.log.levels.ERROR)
      )
    end)

    it('opens a tab and a float by placement (R-003)', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'tab')
      assert.are.equal(2, vim.fn.tabpagenr('$'))
      assert.are.equal(p.win, vim.api.nvim_get_current_win())
      preview.close(buf)
      -- R-252: another tab page exists, so the preview tab page closes
      assert.are.equal(1, vim.fn.tabpagenr('$'))
      local f = preview.open(buf, 'float')
      local cfg = vim.api.nvim_win_get_config(f.win)
      assert.are.equal('editor', cfg.relative)
      assert.are.equal(math.min(config.get().text_width + 4, vim.o.columns - 4), cfg.width)
      assert.are.equal(vim.o.lines - 4, cfg.height)
      assert.is_not_nil(cfg.border)
    end)

    it('toggles (R-009, R-010)', function()
      local buf = fixture_buf()
      local p = preview.toggle(buf, 'split')
      assert.is_not_nil(p)
      assert.is_nil(preview.toggle(buf, 'split'))
      assert.is_nil(preview.get(buf))
      assert.is_false(vim.api.nvim_win_is_valid(p.win))
    end)
  end)

  describe('text first (R-019, R-023, R-171)', function()
    it('fills the buffer before the probe or the helper answer', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      local lines = vim.api.nvim_buf_get_lines(p.buf, 0, -1, false)
      assert.is_true(#lines > 5)
      assert.matches('Title One', lines[1])
      assert.is_nil(preview.mode())
      assert.are.equal('text', p.mode)
      local b = block(buf, 1)
      assert.are.equal('text', b.state)
      local rows = rows_of(buf, b)
      assert.matches('Local alt', rows[1])
      assert.matches('Local alt', rows[2])
    end)

    it('shows 1000 lines within 200 ms', function()
      local lines = {}
      for i = 1, 500 do
        lines[#lines + 1] = string.format('Paragraph %d with *emphasis* and `code` in it.', i)
        lines[#lines + 1] = ''
      end
      local buf = make_buf(lines)
      local started = vim.uv.hrtime()
      local p = preview.open(buf, 'split')
      local elapsed_ms = (vim.uv.hrtime() - started) / 1e6
      -- wrapped paragraphs add lines, so the count is at least the source's
      assert.is_true(vim.api.nvim_buf_line_count(p.buf) >= 999)
      assert.is_true(elapsed_ms < 200, string.format('took %.0f ms', elapsed_ms))
    end)

    it('runs tree-sitter on code regions and keeps the code background (R-047, R-048)', function()
      local buf = make_buf({ '```lua', 'local x = 1', '```', '', '```nosuchlang', 'keep', '```' })
      local p = preview.open(buf, 'split')
      local marks = vim.api.nvim_buf_get_extmarks(p.buf, ns_id(), 0, -1, { details = true })
      local groups = {}
      for _, mark in ipairs(marks) do
        local group = mark[4].hl_group
        if group then
          groups[group] = (groups[group] or 0) + 1
        end
      end
      assert.is_not_nil(groups['@keyword.lua'])
      assert.is_not_nil(groups['KittyMarkdownCodeBg'])
      for group in pairs(groups) do
        assert.is_nil(group:match('nosuchlang'))
      end
    end)
  end)

  describe('probe and mode (R-117 to R-121, R-131, R-184)', function()
    it('sends the probe once on the first open and re-renders when images are on', function()
      local buf = fixture_buf()
      preview.open(buf, 'split')
      assert.are.equal(1, bytes_matching('a=q'))
      assert.are.equal(1, bytes_matching('^' .. ESC .. '%[c$'))
      answer_probe(true)
      assert.are.equal('images', preview.mode())
      local b = wait_state(buf, 1, 'placed')
      assert.are.equal('images', preview.get(buf).mode)
      assert.is_not_nil(b.image_id)
    end)

    it('stays in text-only mode with boxes when the probe fails', function()
      local buf = fixture_buf()
      preview.open(buf, 'split')
      answer_probe(false)
      assert.are.equal('text', preview.mode())
      local image = block(buf, 1)
      assert.are.equal('text', image.state)
      local rows = rows_of(buf, image)
      assert.are.equal(3, #rows)
      assert.matches('^  ╭─ Local alt ─.*─╮$', rows[1])
      assert.matches('^  │ Local alt +│$', rows[2])
      assert.matches('^  ╰─.*─╯$', rows[3])
      local mermaid = block(buf, 2)
      local source_rows = rows_of(buf, mermaid)
      assert.matches('flowchart', source_rows[1])
      assert.matches('flowchart LR', source_rows[2])
      assert.matches('A %-%-> B', source_rows[3])
      assert.are.equal(4, #source_rows)
      local infos = notices_at(vim.log.levels.INFO)
      assert.are.equal(1, #infos)
      assert.matches('text%-only mode: no graphics reply', infos[1])
      assert.are.equal(0, bytes_matching('a=t'))
    end)
  end)

  describe('pixel blocks (R-020, R-021, R-093, R-123 to R-128, R-169)', function()
    it('shows a loading box, then the placeholder rows', function()
      local buf = fixture_buf()
      preview.open(buf, 'split')
      answer_probe(true)
      local b, desc = block(buf, 1)
      if b.state == 'loading' then
        local rows = rows_of(buf, b)
        assert.matches('loading', rows[2])
        assert.matches('Local alt', rows[1])
      end
      b = wait_state(buf, 1, 'placed')
      assert.are.equal(png, b.path)
      assert.are.equal(400, b.width_px)
      assert.are.equal(200, b.height_px)
      assert.are.equal(10, b.rows)
      assert.are.equal(20, b.cols)
      local rows, mark = rows_of(buf, b)
      assert.are.equal(desc.line - 1, mark[1])
      assert.are.equal(10, #rows)
      assert.are.equal('  ' .. PLACEHOLDER, rows[1]:sub(1, 2 + #PLACEHOLDER))
      assert.are.equal(1, bytes_matching('a=t,f=100,t=d,i=' .. b.image_id .. ',q=2'))
      assert.are.equal(1, bytes_matching('a=p,U=1,i=' .. b.image_id .. ',c=20,r=10,q=2'))
      local group = vim.api.nvim_get_hl(0, { name = transport.hl_group(b.image_id) })
      assert.are.equal(b.image_id % 2 ^ 24, group.fg)
    end)

    it('renders mermaid through the helper with the palette theme (R-098, R-100)', function()
      local buf = fixture_buf()
      preview.open(buf, 'split')
      answer_probe(true)
      local b = wait_state(buf, 2, 'placed')
      assert.are.equal(png, b.path)
      assert.are.equal(1, bytes_matching('a=t,f=100,t=d,i=' .. b.image_id))
    end)

    it('keeps a placed block across a re-render without a transmit (R-026, R-170)', function()
      local buf = fixture_buf()
      preview.open(buf, 'split')
      answer_probe(true)
      local before = wait_state(buf, 1, 'placed')
      wait_state(buf, 2, 'placed')
      local sent = #out
      vim.api.nvim_exec_autocmds('BufWritePost', { buffer = buf })
      local after = block(buf, 1)
      assert.are.equal('placed', after.state)
      assert.are.equal(before.image_id, after.image_id)
      assert.are.equal(sent, #out)
      local rows = rows_of(buf, after)
      assert.are.equal(10, #rows)
    end)

    it('deletes the image of a block that vanished (R-128)', function()
      local buf = fixture_buf()
      preview.open(buf, 'split')
      answer_probe(true)
      local b = wait_state(buf, 1, 'placed')
      local id = b.image_id
      vim.api.nvim_buf_set_lines(buf, 0, -1, false, { 'No images here.' })
      vim.api.nvim_exec_autocmds('BufWritePost', { buffer = buf })
      assert.are.equal(1, bytes_matching('a=d,d=I,i=' .. id .. ',q=2'))
      assert.is_nil(next(preview.get(buf).blocks))
    end)

    it('re-probes and re-transmits every placed image on refresh (R-017, R-130)', function()
      local buf = fixture_buf()
      preview.open(buf, 'split')
      answer_probe(true)
      local b = wait_state(buf, 1, 'placed')
      wait_state(buf, 2, 'placed')
      assert.are.equal(1, bytes_matching('a=t,f=100,t=d,i=' .. b.image_id))
      assert.is_true(preview.render(buf, { reprobe = true }))
      assert.are.equal(2, bytes_matching('a=q'))
      assert.are.equal(2, bytes_matching('a=t,f=100,t=d,i=' .. b.image_id))
      assert.are.equal(2, bytes_matching('a=p,U=1,i=' .. b.image_id))
      assert.are.equal('placed', block(buf, 1).state)
      answer_probe(true)
    end)

    it('lists the paths of placed blocks (R-152)', function()
      local buf = fixture_buf()
      assert.are.same({}, preview.in_use_paths())
      preview.open(buf, 'split')
      answer_probe(true)
      wait_state(buf, 1, 'placed')
      wait_state(buf, 2, 'placed')
      assert.are.same({ png, png }, preview.in_use_paths())
      assert.are.same({ png, png }, cache.keep_set())
    end)

    it('asks for garbage collection once after the batch settles (R-202)', function()
      config.setup({ placement = 'split', cache = { max_bytes = 1 } })
      local calls = 0
      local real_gc = cache.gc_if_needed
      cache.gc_if_needed = function(cb)
        calls = calls + 1
        return real_gc(cb)
      end
      local buf = fixture_buf()
      preview.open(buf, 'split')
      answer_probe(true)
      wait_state(buf, 1, 'placed')
      wait_state(buf, 2, 'placed')
      assert.is_true(wait_for(function()
        return calls == 1 and cache.estimate() == 0
      end))
      settle(100)
      assert.are.equal(1, calls)
      cache.gc_if_needed = real_gc
    end)

    it('honors the <img> limits in the prepare request (R-092)', function()
      local buf = make_buf({ '<img src="pic.png" alt="Small" width="100" height="40">' })
      preview.open(buf, 'split')
      answer_probe(true)
      local b = wait_state(buf, 1, 'placed')
      assert.are.equal(40, b.height_px)
      assert.are.equal(80, b.width_px)
      assert.are.equal(2, b.rows)
      assert.are.equal(4, b.cols)
    end)

    it('renders text-only after a flip back to text mode (R-131)', function()
      local buf = fixture_buf()
      preview.open(buf, 'split')
      answer_probe(true)
      local b = wait_state(buf, 1, 'placed')
      local id = b.image_id
      preview.render(buf, { reprobe = true })
      fire(ESC .. '[?62;c')
      assert.is_true(wait_for(function()
        return preview.mode() == 'text'
      end))
      assert.is_true(wait_for(function()
        return block(buf, 1).state == 'text'
      end))
      assert.are.equal(1, bytes_matching('a=d,d=I,i=' .. id))
      assert.matches('Local alt', rows_of(buf, block(buf, 1))[2])
    end)
  end)

  describe(
    'failure boxes (R-082 to R-085, R-089, R-096, R-111 to R-113, R-172, R-173, R-177, R-198)',
    function()
      local function failure_rows(buf, n)
        local b = wait_state(buf, n, 'failed')
        local rows = rows_of(buf, b)
        return rows, b
      end

      it('maps helper error codes to the reasons', function()
        local buf = make_buf({
          '![One](err-timeout.png)',
          '',
          '![Two](err-too_large.png)',
          '',
          '![Three](err-unsupported_format.png)',
          '',
          '![Four](err-io.png)',
          '',
          '![Five](https://example.com/err-fetch_failed.png)',
          '',
          '![Six](err-internal.png)',
          '',
          '![Seven](missing.png)',
          '',
          '```mermaid',
          '%% err parse_error 3',
          'flowchart LR',
          '```',
          '',
          '```mermaid',
          '%% err unsupported_feature',
          'flowchart LR',
          '```',
          '',
          '```mermaid',
          '%% err unsupported_diagram_type',
          'zenuml',
          '```',
        })
        preview.open(buf, 'split')
        answer_probe(true)
        local expected = {
          'timeout',
          'too large',
          'unsupported format',
          'cannot read file',
          'fetch failed',
          'helper error',
          'cannot read file',
          'Parse error on line 3',
          'unsupported: elk',
          'unsupported diagram type',
        }
        for n, reason in ipairs(expected) do
          local rows, b = failure_rows(buf, n)
          assert.are.equal(reason, b.reason)
          assert.are.equal(3, #rows)
          assert.matches('^  ╭─ .*─╮$', rows[1])
          assert.matches('^  │ ' .. vim.pesc(reason) .. ' +│$', rows[2])
        end
        local errors = notices_at(vim.log.levels.ERROR)
        assert.are.equal(10, #errors)
        assert.matches('^kitty%-markdown: image One: timeout: ', errors[1])
        local parse
        for _, msg in ipairs(errors) do
          if msg:find('parse_error', 1, true) then
            parse = msg
          end
        end
        assert.is_not_nil(parse)
        assert.matches(
          '^kitty%-markdown: mermaid flowchart: parse_error: Parse error on line 3:',
          parse
        )
        assert.matches('Expecting', parse)
      end)

      it('shows remote images off without a request (R-085)', function()
        config.setup({ placement = 'split', remote_images = { enabled = false } })
        local buf = make_buf({ '![Remote](https://example.com/r.png)' })
        preview.open(buf, 'split')
        answer_probe(true)
        local rows, b = failure_rows(buf, 1)
        assert.are.equal('remote images off', b.reason)
        assert.matches('remote images off', rows[2])
        assert.are.same({}, notices_at(vim.log.levels.ERROR))
      end)

      it('shows timeout when the helper does not answer in time (R-141)', function()
        config.setup({ placement = 'split', helper = { request_timeout_ms = 50 } })
        local buf = make_buf({ '![Slow](slow.png)' })
        preview.open(buf, 'split')
        answer_probe(true)
        local _, b = failure_rows(buf, 1)
        assert.are.equal('timeout', b.reason)
      end)

      it('shows helper exited when the helper dies with a block pending', function()
        local buf = make_buf({ '![Boom](crash.png)' })
        preview.open(buf, 'split')
        answer_probe(true)
        local _, b = failure_rows(buf, 1)
        assert.are.equal('helper exited', b.reason)
        assert.is_true(#notices_at(vim.log.levels.WARN) >= 1)
      end)

      it('falls back to a text box without a helper (R-134)', function()
        helper.set_locator(function(cb)
          cb(nil)
        end)
        local buf = fixture_buf()
        preview.open(buf, 'split')
        answer_probe(true)
        local b = wait_state(buf, 1, 'text')
        local rows = rows_of(buf, b)
        assert.matches('Local alt', rows[2])
        local errors = notices_at(vim.log.levels.ERROR)
        assert.are.equal(1, #errors)
        assert.matches('no helper binary found', errors[1])
      end)

      it('fails a block whose rows exceed the placeholder limit', function()
        write_png(tmp .. '/tall.png', 100, 20000)
        transport.set_winsize_reader(function()
          return 24, 80, 1600, 48
        end)
        local buf = make_buf({ '![Tall](tall.png)' })
        preview.open(buf, 'split')
        answer_probe(true)
        local _, b = failure_rows(buf, 1)
        assert.are.equal('too tall', b.reason)
        assert.are.equal(1, bytes_matching('a=d,d=I,i=' .. b.image_id))
      end)
    end
  )

  describe('close of the last window of a tab page (R-252, R-346, R-347)', function()
    local function check_tab_closed(p)
      assert.is_false(vim.api.nvim_win_is_valid(p.win))
      assert.is_false(vim.api.nvim_buf_is_valid(p.buf))
      assert.are.equal(1, vim.fn.tabpagenr('$'))
    end

    local function check_released(p, other)
      assert.is_true(vim.api.nvim_win_is_valid(p.win))
      assert.is_false(vim.api.nvim_buf_is_valid(p.buf))
      local shown = vim.api.nvim_win_get_buf(p.win)
      assert.are_not.equal(p.buf, shown)
      if other then
        assert.are.equal(other, shown)
      end
      assert.are.equal(1, vim.fn.tabpagenr('$'))
    end

    it('closes the tab page of a closed tab preview (R-252)', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'tab')
      answer_probe(true)
      local b = wait_state(buf, 1, 'placed')
      assert.is_true(preview.close(buf))
      check_tab_closed(p)
      assert.are.equal(1, bytes_matching('a=d,d=I,i=' .. b.image_id .. ',q=2'))
      assert.is_nil(preview.get(buf))
    end)

    it('closes the tab page when a tab preview is toggled off from inside it', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'tab')
      assert.are.equal(p.win, vim.api.nvim_get_current_win())
      assert.is_nil(preview.toggle(p.buf))
      check_tab_closed(p)
      assert.is_nil(preview.get(buf))
    end)

    it('closes the tab page when q closes a tab preview', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'tab')
      vim.api.nvim_feedkeys('q', 'x', false)
      check_tab_closed(p)
    end)

    it('closes the tab page of a tab preview that also holds a float', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'tab')
      local float = vim.api.nvim_open_win(vim.api.nvim_create_buf(false, true), false, {
        relative = 'editor',
        row = 1,
        col = 1,
        width = 10,
        height = 2,
      })
      assert.is_true(preview.close(buf))
      check_tab_closed(p)
      assert.is_false(vim.api.nvim_win_is_valid(float))
    end)

    it('closes the tab page of a split preview that became its last window', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      vim.api.nvim_set_current_win(p.win)
      vim.cmd('only')
      vim.cmd('tab split')
      assert.is_true(preview.close(buf))
      check_tab_closed(p)
    end)

    it('keeps the only window of the only tab page on the source (R-346)', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      vim.api.nvim_set_current_win(p.win)
      vim.cmd('only')
      assert.is_true(preview.close(buf))
      check_released(p, buf)
      assert.is_nil(preview.get(buf))
      -- the preview window options do not stay on the source
      assert.are.equal(vim.go.number, vim.wo[p.win].number)
      assert.are.equal(vim.go.wrap, vim.wo[p.win].wrap)
      assert.are.equal(vim.go.winhighlight, vim.wo[p.win].winhighlight)
    end)

    it('keeps the only window on an empty buffer when the source is wiped (R-347)', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'tab')
      vim.api.nvim_buf_delete(buf, { force = true })
      assert.is_true(wait_for(function()
        return not vim.api.nvim_buf_is_valid(p.buf)
      end))
      -- the wipe closed the tab page of the source window
      check_released(p)
      local shown = vim.api.nvim_win_get_buf(p.win)
      assert.are.equal('', vim.api.nvim_buf_get_name(shown))
      assert.are.same({ '' }, vim.api.nvim_buf_get_lines(shown, 0, -1, false))
      assert.is_nil(preview.get(buf))
    end)

    it('still closes a tab preview window that shares its tab page', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'tab')
      vim.cmd('split')
      vim.api.nvim_win_set_buf(0, vim.api.nvim_create_buf(true, false))
      assert.is_true(preview.close(buf))
      assert.is_false(vim.api.nvim_win_is_valid(p.win))
      assert.is_false(vim.api.nvim_buf_is_valid(p.buf))
      assert.are.equal(2, vim.fn.tabpagenr('$'))
    end)
  end)

  describe('close (R-008, R-011)', function()
    it('deletes images, closes the window, and wipes the buffer', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      answer_probe(true)
      local b = wait_state(buf, 1, 'placed')
      wait_state(buf, 2, 'placed')
      assert.is_true(preview.close(buf))
      assert.are.equal(1, bytes_matching('a=d,d=I,i=' .. b.image_id .. ',q=2'))
      assert.is_false(vim.api.nvim_win_is_valid(p.win))
      assert.is_false(vim.api.nvim_buf_is_valid(p.buf))
      assert.is_nil(preview.get(buf))
      assert.is_false(preview.close(buf))
    end)

    it('closes when the source buffer is wiped', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      vim.api.nvim_win_set_buf(0, vim.api.nvim_create_buf(true, false))
      vim.api.nvim_buf_delete(buf, { force = true })
      assert.is_true(wait_for(function()
        return not vim.api.nvim_win_is_valid(p.win)
      end))
      assert.is_nil(preview.get(buf))
      assert.is_false(vim.api.nvim_buf_is_valid(p.buf))
    end)

    it('drops its state when the user closes the preview window', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      answer_probe(true)
      local b = wait_state(buf, 1, 'placed')
      vim.api.nvim_win_close(p.win, true)
      assert.is_true(wait_for(function()
        return preview.get(buf) == nil
      end))
      assert.are.equal(1, bytes_matching('a=d,d=I,i=' .. b.image_id))
      assert.is_true(wait_for(function()
        return not vim.api.nvim_buf_is_valid(p.buf)
      end))
    end)
  end)

  describe('keys (R-158 to R-161)', function()
    it('q closes the preview', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      vim.api.nvim_set_current_win(p.win)
      vim.api.nvim_feedkeys('q', 'x', false)
      assert.is_nil(preview.get(buf))
    end)

    local function cursor_on(p, needle)
      for i, line in ipairs(p.model.lines) do
        local col = line:find(needle, 1, true)
        if col then
          vim.api.nvim_win_set_cursor(p.win, { i, col - 1 })
          return i
        end
      end
      error('no ' .. needle)
    end

    it('gx opens a URL and a file with the system opener', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      cursor_on(p, 'link')
      assert.is_not_nil(preview.follow(buf))
      cursor_on(p, 'file')
      vim.api.nvim_set_current_win(p.win)
      vim.api.nvim_feedkeys('gx', 'x', false)
      assert.are.same({ 'https://example.com/page', tmp .. '/other.md' }, opened)
    end)

    it('gx on an image anchor line opens the image file', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      local _, desc = block(buf, 1)
      vim.api.nvim_win_set_cursor(p.win, { desc.line, 0 })
      assert.are.equal('image', preview.follow(buf).kind)
      assert.are.same({ png }, opened)
    end)

    it('gx on an anchor link scrolls to the heading', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      cursor_on(p, 'anchor')
      assert.are.equal('anchor', preview.follow(buf).kind)
      assert.are.equal(p.model.anchors['section-two'], vim.api.nvim_win_get_cursor(p.win)[1])
      assert.are.same({}, opened)
    end)

    it('gx off a link does nothing', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      cursor_on(p, 'Title One')
      assert.is_nil(preview.follow(buf))
      assert.are.same({}, opened)
    end)
  end)

  describe('scroll sync (R-162 to R-164, R-211)', function()
    it('maps a source line to the start of its block', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      local model = p.model
      local function line_of(needle)
        for i, line in ipairs(model.lines) do
          if line:find(needle, 1, true) then
            return i
          end
        end
        error('no ' .. needle)
      end
      assert.are.equal(1, preview.sync_target(model, 1))
      assert.are.equal(line_of('A paragraph'), preview.sync_target(model, 3))
      -- without block ranges the second source line lands inside it
      local inside = preview.sync_target(model, 4)
      assert.is_true(inside >= line_of('A paragraph') and inside < model.blocks[1].line)
      -- with them it lands on the first line of the paragraph (R-248)
      assert.are.equal(line_of('A paragraph'), preview.sync_target(model, 4, p.source_blocks))
      -- the image anchor line, not the separator before or after it
      assert.are.equal(model.blocks[1].line, preview.sync_target(model, 6))
      assert.are.equal(line_of('Section Two'), preview.sync_target(model, 12))
      assert.are.equal(model.blocks[2].line, preview.sync_target(model, 17))
      assert.are.equal(line_of('Last paragraph'), preview.sync_target(model, 99))
      assert.are.equal(1, preview.sync_target(model, 0))
    end)

    it('puts the target in the top third of the window', function()
      assert.are.equal(1, preview.sync_topline(3, 30))
      assert.are.equal(31, preview.sync_topline(41, 30))
      assert.are.equal(1, preview.sync_topline(10, 30))
    end)

    it('scrolls a split preview from the source cursor', function()
      local lines = {}
      for i = 1, 60 do
        lines[#lines + 1] = 'Paragraph ' .. i
        lines[#lines + 1] = ''
      end
      local buf = make_buf(lines)
      local p = preview.open(buf, 'split')
      local target = preview.sync(buf, 81)
      assert.are.equal(81, target)
      local height = vim.api.nvim_win_get_height(p.win)
      local topline = vim.api.nvim_win_call(p.win, function()
        return vim.fn.line('w0')
      end)
      assert.are.equal(81 - math.floor(height / 3), topline)
      vim.api.nvim_win_set_cursor(0, { 41, 0 })
      vim.api.nvim_exec_autocmds('CursorMoved', { buffer = buf })
      assert.is_true(wait_for(function()
        return vim.api.nvim_win_call(p.win, function()
          return vim.fn.line('w0')
        end) == 41 - math.floor(height / 3)
      end, 1000))
    end)

    it('does not sync a tab or a float', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'tab')
      assert.is_nil(p.sync_timer)
      preview.close(buf)
      config.setup({ placement = 'split', scroll_sync = false })
      local q = preview.open(buf, 'split')
      assert.is_nil(q.sync_timer)
    end)
  end)

  describe('scroll sync to the block start (R-248, R-340, R-341)', function()
    local function line_of(model, needle)
      for i, line in ipairs(model.lines) do
        if line:find(needle, 1, true) then
          return i
        end
      end
      error('no ' .. needle)
    end

    it('targets the first line of the block under the cursor', function()
      local lines = {
        '---',
        'title: x',
        '---',
        '',
        '# Heading',
        '',
      }
      local para = #lines + 1
      for i = 1, 6 do
        lines[#lines + 1] = 'Paragraph line ' .. i .. ' with words.'
      end
      lines[#lines + 1] = ''
      local list = #lines + 1
      for i = 1, 30 do
        lines[#lines + 1] = '- item ' .. i
        if i % 5 == 0 then
          lines[#lines + 1] = '  - nested ' .. i
        end
      end
      local list_end = #lines
      lines[#lines + 1] = ''
      local quote = #lines + 1
      lines[#lines + 1] = '> quote one'
      lines[#lines + 1] = '> quote two'
      lines[#lines + 1] = '> quote three'
      lines[#lines + 1] = ''
      local code = #lines + 1
      lines[#lines + 1] = '```lua'
      lines[#lines + 1] = 'local a = 1'
      lines[#lines + 1] = 'local b = 2'
      lines[#lines + 1] = '```'
      lines[#lines + 1] = ''
      lines[#lines + 1] = '| a | b |'
      local tbl = #lines
      lines[#lines + 1] = '| - | - |'
      lines[#lines + 1] = '| 1 | 2 |'
      lines[#lines + 1] = '| 3 | 4 |'
      local buf = make_buf(lines)
      local p = preview.open(buf, 'split')
      local model = p.model
      local first_para = line_of(model, 'Paragraph line 1')
      for l = para, para + 5 do
        assert.are.equal(first_para, preview.sync(buf, l), 'paragraph line ' .. l)
      end
      local first_item = line_of(model, 'item 1')
      for _, l in ipairs({ list, list + 1, list + 12, list + 20, list_end }) do
        assert.are.equal(first_item, preview.sync(buf, l), 'list line ' .. l)
      end
      local first_quote = line_of(model, 'quote one')
      assert.are.equal(first_quote, preview.sync(buf, quote + 2))
      local code_start = preview.sync(buf, code)
      assert.are.equal(code_start, preview.sync(buf, code + 2))
      local table_start = preview.sync(buf, tbl)
      assert.are.equal(table_start, preview.sync(buf, tbl + 3))
      -- R-340: a blank line belongs to the block above
      assert.are.equal(first_para, preview.sync(buf, para + 6))
      assert.are.equal(first_item, preview.sync(buf, list_end + 1))
      -- R-341: front matter goes to line 1
      assert.are.equal(1, preview.sync(buf, 2))
      assert.are.equal(line_of(model, 'Heading'), preview.sync(buf, 5))
    end)

    it('follows the source cursor to the block start', function()
      local lines = { '# Top', '' }
      for i = 1, 80 do
        lines[#lines + 1] = 'Line ' .. i .. ' of one long paragraph.'
      end
      local buf = make_buf(lines)
      local p = preview.open(buf, 'split')
      local start = line_of(p.model, 'Line 1 ')
      vim.api.nvim_win_set_cursor(0, { 70, 0 })
      vim.api.nvim_exec_autocmds('CursorMoved', { buffer = buf })
      assert.is_true(wait_for(function()
        return vim.api.nvim_win_get_cursor(p.win)[1] == start
      end, 1000))
    end)
  end)

  describe('refresh (R-017, R-130, R-196, R-244, R-310)', function()
    it('re-transmits the placed images of every open preview', function()
      -- narrow text, so the second split does not change the block keys
      config.setup({ placement = 'split', text_width = 16 })
      local a = make_buf(vim.fn.readfile(fixtures .. '/doc.md'), 'a.md')
      preview.open(a, 'split')
      answer_probe(true)
      local ba = wait_state(a, 1, 'placed')
      local b2 = make_buf({ '![B alt](pic.png)' }, 'b.md')
      preview.open(b2, 'split')
      local bb = wait_state(b2, 1, 'placed')
      local c = make_buf({ '# Not shown' }, 'c.txt')
      vim.bo[c].filetype = 'text'
      for _, id in ipairs({ ba.image_id, bb.image_id }) do
        assert.are.equal(1, bytes_matching('a=t,f=100,t=d,i=' .. id .. ','))
      end
      -- from the source of one preview
      preview.render(a, { reprobe = true })
      for _, id in ipairs({ ba.image_id, bb.image_id }) do
        assert.are.equal(2, bytes_matching('a=t,f=100,t=d,i=' .. id .. ','))
        assert.are.equal(2, bytes_matching('a=p,U=1,i=' .. id .. ','))
      end
      answer_probe(true)
      -- the probe callback runs scheduled; let it finish before the next
      settle()
      -- from a buffer that has no preview
      require('kitty-markdown').refresh()
      assert.are.equal(c, vim.api.nvim_get_current_buf())
      for _, id in ipairs({ ba.image_id, bb.image_id }) do
        assert.are.equal(3, bytes_matching('a=t,f=100,t=d,i=' .. id .. ','))
      end
      assert.are.equal(3, bytes_matching('a=q'))
      answer_probe(true)
      assert.are.equal('placed', block(a, 1).state)
      assert.are.equal('placed', block(b2, 1).state)
    end)

    local function crash()
      local done = false
      helper.request('image.prepare', { path = tmp .. '/crash.png', width_px = 10 }, function()
        done = true
      end)
      assert.is_true(wait_for(function()
        return done and not helper.is_running()
      end))
    end

    local function restart_errors()
      local n = 0
      for _, msg in ipairs(notices_at(vim.log.levels.ERROR)) do
        if msg:find('60 seconds', 1, true) then
          n = n + 1
        end
      end
      return n
    end

    it('clears the restart stop with no preview and needs three new exits', function()
      for _ = 1, 3 do
        crash()
      end
      assert.are.equal('failed', helper.state())
      assert.are.equal(1, restart_errors())
      local other = vim.api.nvim_create_buf(true, false)
      vim.bo[other].filetype = 'lua'
      vim.api.nvim_set_current_buf(other)
      require('kitty-markdown').refresh()
      assert.are.equal('absent', helper.state())
      crash()
      assert.are.equal('absent', helper.state())
      crash()
      assert.are.equal('absent', helper.state())
      assert.are.equal(1, restart_errors())
      crash()
      assert.are.equal('failed', helper.state())
      assert.are.equal(2, restart_errors())
      -- an unnamed scratch buffer with no filetype clears it as well
      vim.api.nvim_set_current_buf(vim.api.nvim_create_buf(false, true))
      require('kitty-markdown').refresh()
      assert.are.equal('absent', helper.state())
    end)
  end)

  describe('helper exit messages (R-195, R-198, R-313)', function()
    it('writes one WARN and nothing per block when the helper dies', function()
      local buf = make_buf({
        '![Slow one](slow.png)',
        '',
        '![Slow two](slow-2.png)',
        '',
        '![Boom](crash.png)',
        '',
        '```mermaid',
        'flowchart LR',
        '  A --> B',
        '```',
        '',
        '![Remote](https://example.com/slow.png)',
      })
      preview.open(buf, 'split')
      answer_probe(true)
      for n = 1, 3 do
        assert.are.equal('helper exited', wait_state(buf, n, 'failed').reason)
      end
      settle(600)
      local own = {}
      for _, n in ipairs(notices) do
        if n.level ~= vim.log.levels.INFO then
          own[#own + 1] = n
        end
      end
      assert.are.equal(1, #own)
      assert.are.equal(vim.log.levels.WARN, own[1].level)
      assert.matches('helper exited with', own[1].msg)
    end)
  end)

  describe('URL schemes (R-079, R-225, R-287)', function()
    it('fails every scheme other than http and https without a request', function()
      local file = tmp .. '/pic.png'
      local sources = {
        'file://' .. file,
        'FILE://' .. file,
        'File:' .. file,
        'ftp://example.com/a.png',
        'data:image/png;base64,iVBORw0KGgo=',
        'gopher://example.com/a.png',
        'x-custom+app.v2:thing.png',
        's3://bucket/key.png',
        'mailto:a@example.com',
      }
      local lines = {}
      for i, src in ipairs(sources) do
        lines[#lines + 1] = string.format('![S%d](%s)', i, src)
        lines[#lines + 1] = ''
      end
      local remote_before = vim.fn.glob(cache.dirs().remote .. '/*', true, true)
      local requests = {}
      local real_request = helper.request
      helper.request = function(method, params, cb)
        requests[#requests + 1] = { method = method, params = params }
        return real_request(method, params, cb)
      end
      local buf = make_buf(lines)
      preview.open(buf, 'split')
      answer_probe(true)
      for n = 1, #sources do
        local b = wait_state(buf, n, 'failed')
        assert.are.equal('fetch failed', b.reason, sources[n])
        assert.matches('fetch failed', rows_of(buf, b)[2])
      end
      helper.request = real_request
      for _, r in ipairs(requests) do
        assert.is_true(r.method ~= 'fetch' and r.method ~= 'image.prepare', r.method)
      end
      local errors = notices_at(vim.log.levels.ERROR)
      assert.are.equal(#sources, #errors)
      assert.are.equal(
        'kitty-markdown: image S1: fetch_failed: file://'
          .. file
          .. ': unsupported URL scheme "file"',
        errors[1]
      )
      assert.are.same(remote_before, vim.fn.glob(cache.dirs().remote .. '/*', true, true))
    end)

    it('fetches http and https in any case and reads plain paths', function()
      local buf = make_buf({
        '![A](https://example.com/a.png)',
        '',
        '![B](HTTP://example.com/b.png)',
        '',
        '![C](HtTpS://example.com/c.png)',
        '',
        '![D](pic.png)',
        '',
        '![E](' .. tmp .. '/pic.png)',
      })
      preview.open(buf, 'split')
      answer_probe(true)
      for n = 1, 5 do
        wait_state(buf, n, 'placed')
      end
    end)

    it('shows remote images off for http with remote images disabled', function()
      config.setup({ placement = 'split', remote_images = { enabled = false } })
      local buf = make_buf({ '![R](https://example.com/r.png)', '', '![F](file:///x.png)' })
      preview.open(buf, 'split')
      answer_probe(true)
      assert.are.equal('remote images off', wait_state(buf, 1, 'failed').reason)
      assert.are.equal('fetch failed', wait_state(buf, 2, 'failed').reason)
    end)
  end)

  describe(':messages text (R-288)', function()
    it('names the path or URL and the cause of each failure', function()
      local buf = make_buf({
        '![Bad](err-unsupported_format.png)',
        '',
        '![](err-too_large.png)',
        '',
        '![Gone](https://example.com/err-fetch_failed.png)',
        '',
        '![Missing](missing.png)',
      })
      preview.open(buf, 'split')
      answer_probe(true)
      for n = 1, 4 do
        wait_state(buf, n, 'failed')
      end
      local errors = notices_at(vim.log.levels.ERROR)
      assert.are.equal(4, #errors)
      assert.are.equal(
        'kitty-markdown: image Bad: unsupported_format: '
          .. tmp
          .. '/err-unsupported_format.png: fake unsupported_format',
        errors[1]
      )
      assert.are.equal(
        'kitty-markdown: image image: too_large: ' .. tmp .. '/err-too_large.png: fake too_large',
        errors[2]
      )
      assert.are.equal(
        'kitty-markdown: image Gone: fetch_failed: '
          .. 'https://example.com/err-fetch_failed.png: fake fetch_failed',
        errors[3]
      )
      -- a message that names the path already keeps its own text
      assert.are.equal(
        'kitty-markdown: image Missing: io: cannot read ' .. tmp .. '/missing.png',
        errors[4]
      )
    end)
  end)

  describe('cache keep set (R-152, R-328)', function()
    it('keeps the prepared PNG and the fetched file of a remote image', function()
      vim.fn.mkdir(tmp .. '/remote', 'p')
      vim.fn.mkdir(tmp .. '/images', 'p')
      local fetched = tmp .. '/remote/abc'
      local prepared = tmp .. '/images/def.png'
      vim.fn.writefile(vim.fn.readblob(png), fetched)
      vim.fn.writefile(vim.fn.readblob(png), prepared)
      vim.env.KM_FAKE_FETCHED = fetched
      vim.env.KM_FAKE_PREPARED = prepared
      config.setup({ placement = 'split', cache = { max_bytes = 1 } })
      local sent
      local real_request = helper.request
      helper.request = function(method, params, cb)
        if method == 'cache.gc' then
          sent = params
        end
        return real_request(method, params, cb)
      end
      local buf = make_buf({ '![R](https://example.com/r.png)', '', '![L](pic.png)' })
      preview.open(buf, 'split')
      answer_probe(true)
      wait_state(buf, 1, 'placed')
      wait_state(buf, 2, 'placed')
      assert.is_true(wait_for(function()
        return sent ~= nil
      end))
      helper.request = real_request
      local keep = vim.deepcopy(sent.keep)
      table.sort(keep)
      assert.are.same({ prepared, fetched }, { keep[1], keep[3] })
      assert.is_true(vim.tbl_contains(keep, fetched))
      assert.is_true(vim.tbl_contains(keep, prepared))
      assert.are.equal(3, #keep)
    end)
  end)

  describe('theme (R-075)', function()
    it('re-applies the groups and re-renders on a background change', function()
      local buf = fixture_buf()
      local p = preview.open(buf, 'split')
      assert.are.equal('light', p.model.theme)
      vim.o.background = 'dark'
      vim.api.nvim_exec_autocmds('OptionSet', { pattern = 'background' })
      assert.are.equal('dark', p.model.theme)
      assert.are.equal(0xf0f6fc, vim.api.nvim_get_hl(0, { name = 'KittyMarkdownH1' }).fg)
    end)
  end)
end)
