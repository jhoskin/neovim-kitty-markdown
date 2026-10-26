local QA = require('qa')
local U = require('core_util')
local a = QA.assert

-- The spec names the palettes (GitHub light and GitHub dark markdown) but
-- not the exact hex values, which GitHub revises. The checks therefore test
-- the properties both GitHub palettes have: dark text on the light palette,
-- light text on the dark palette, blue links, and a muted color distinct
-- from the text color.

local function clear_groups()
  for name in pairs(vim.api.nvim_get_hl(0, {})) do
    if type(name) == 'string' and name:find('^KittyMarkdown') then
      vim.api.nvim_set_hl(0, name, {})
    end
  end
end

local function setup(opts)
  require('kitty-markdown').setup(opts or {})
end

local function fg(name, win)
  local h = win and U.win_hl(win, name) or U.hl(name)
  return h.fg
end

local function is_blue(n)
  local r, g, b = U.rgb(n)
  return b > r + 40 and b > g
end

QA.describe('core: theme', function()
  local groups

  QA.before_each(function()
    QA.reset_plugin()
    clear_groups()
    vim.cmd('set background=light')
    groups = U.discover_groups()
  end)

  QA.after_each(function()
    pcall(function()
      require('kitty-markdown').close()
    end)
    QA.reset_plugin()
  end)

  -- R-071 ------------------------------------------------------------------

  QA.check('R-071', 'Every highlight group the layout uses is named KittyMarkdown*', function()
    local bad = {}
    for _, h in ipairs(groups.model.highlights) do
      local _, _, _, g = U.hl_parts(h)
      if type(g) ~= 'string' or not g:find('^KittyMarkdown') then
        bad[#bad + 1] = tostring(g)
      end
    end
    a.eq({}, bad)
  end)

  QA.check('R-071', 'After setup every KittyMarkdown* group the layout uses is defined', function()
    setup({})
    local doc = QA.layout({
      '# H1',
      '## H2',
      '### H3',
      '#### H4',
      '##### H5',
      '###### H6',
      '',
      'text *em* **strong** ~~del~~ `code` [link](https://x.y) $\\alpha$',
      '',
      '> quote',
      '',
      '> [!NOTE]',
      '> note',
      '',
      '- [ ] todo',
      '- [x] done',
      '',
      '1. one',
      '',
      '| a | b |',
      '| - | - |',
      '| 1 | 2 |',
      '',
      '```lua',
      'x = 1',
      '```',
      '',
      '---',
      '',
      '<div>html</div>',
      '',
      'ref[^1]',
      '',
      '[^1]: foot',
    }, { width = 80 })
    local undefined = {}
    local seen = {}
    for _, h in ipairs(doc.highlights) do
      local _, _, _, g = U.hl_parts(h)
      if type(g) == 'string' and not seen[g] then
        seen[g] = true
        if next(U.hl(g)) == nil then
          undefined[#undefined + 1] = g
        end
      end
    end
    table.sort(undefined)
    a.eq({}, undefined)
  end)

  QA.check(
    'R-071',
    'The light palette has dark heading text and a blue link color, as GitHub light does',
    function()
      setup({ theme = 'light' })
      a.not_nil(groups.heading, 'heading group')
      a.not_nil(groups.link, 'link group')
      local hf, lf = fg(groups.heading), fg(groups.link)
      a.not_nil(hf, 'heading fg')
      a.not_nil(lf, 'link fg')
      a.le(0.1, U.luminance(hf), 'heading text luminance')
      a.truthy(is_blue(lf), ('link color %06x is blue'):format(lf))
    end
  )

  QA.check(
    'R-071',
    'The dark palette has light heading text and a blue link color, as GitHub dark does',
    function()
      setup({ theme = 'dark' })
      local hf, lf = fg(groups.heading), fg(groups.link)
      a.not_nil(hf, 'heading fg')
      a.not_nil(lf, 'link fg')
      a.ge(0.6, U.luminance(hf), 'heading text luminance')
      a.truthy(is_blue(lf), ('link color %06x is blue'):format(lf))
    end
  )

  QA.check(
    'R-071',
    'The muted color differs from the text color in both palettes',
    { severity = 'medium' },
    function()
      a.not_nil(groups.html, 'muted group from an HTML block')
      for _, t in ipairs({ 'light', 'dark' }) do
        setup({ theme = t })
        local m, h = fg(groups.html), fg(groups.heading)
        a.not_nil(m, t .. ' muted fg')
        a.ne(h, m, t .. ' muted equals text color')
      end
    end
  )

  QA.check(
    'R-071',
    'The code span background differs between the light and the dark palettes',
    { severity = 'medium' },
    function()
      a.not_nil(groups.code, 'code group')
      setup({ theme = 'light' })
      local lb = U.hl(groups.code).bg
      setup({ theme = 'dark' })
      local db = U.hl(groups.code).bg
      a.not_nil(lb, 'light code bg')
      a.not_nil(db, 'dark code bg')
      a.ne(lb, db)
      a.truthy(U.luminance(lb) > U.luminance(db), 'light code bg lighter than dark code bg')
    end
  )

  -- R-072 / R-073 ------------------------------------------------------------

  QA.check(
    'R-072',
    'With the automatic theme and a dark background the groups use the dark palette',
    function()
      vim.cmd('set background=dark')
      setup({ theme = 'auto' })
      a.ge(0.6, U.luminance(fg(groups.heading)), 'heading text luminance')
    end
  )

  QA.check(
    'R-073',
    'With the automatic theme and a light background the groups use the light palette',
    function()
      vim.cmd('set background=light')
      setup({ theme = 'auto' })
      a.le(0.1, U.luminance(fg(groups.heading)), 'heading text luminance')
    end
  )

  QA.check(
    { 'R-072', 'R-073' },
    'The automatic theme is the default when the theme option is not set',
    function()
      vim.cmd('set background=dark')
      setup({})
      a.ge(0.6, U.luminance(fg(groups.heading)), 'dark heading text')
      QA.reset_plugin()
      clear_groups()
      vim.cmd('set background=light')
      setup({})
      a.le(0.1, U.luminance(fg(groups.heading)), 'light heading text')
    end
  )

  QA.check(
    { 'R-072', 'R-073' },
    'An open preview shows the palette that matches the background',
    function()
      vim.cmd('set background=dark')
      setup({})
      local win = U.open_preview({ '# Title', '', '[l](https://x.y)' })
      a.ge(0.6, U.luminance(fg(groups.heading, win)), 'heading text in the preview window')
    end
  )

  -- R-074 ------------------------------------------------------------------

  QA.check(
    'R-074',
    'The light theme option keeps the light palette on a dark background',
    function()
      vim.cmd('set background=dark')
      setup({ theme = 'light' })
      a.le(0.1, U.luminance(fg(groups.heading)), 'heading text luminance')
    end
  )

  QA.check('R-074', 'The dark theme option keeps the dark palette on a light background', function()
    vim.cmd('set background=light')
    setup({ theme = 'dark' })
    a.ge(0.6, U.luminance(fg(groups.heading)), 'heading text luminance')
  end)

  QA.check(
    'R-074',
    'A fixed theme option does not change when the background changes under an open preview',
    function()
      vim.cmd('set background=light')
      setup({ theme = 'dark' })
      local win = U.open_preview({ '# Title' })
      local before = fg(groups.heading, win)
      vim.cmd('set background=light')
      vim.cmd('set background=dark')
      vim.cmd('set background=light')
      QA.wait(200, function()
        return false
      end)
      a.eq(before, fg(groups.heading, win))
      a.ge(0.6, U.luminance(fg(groups.heading, win)))
    end
  )

  QA.check(
    'R-203',
    'A theme passed in the layout options fixes the model whatever the background',
    { severity = 'medium' },
    function()
      vim.cmd('set background=dark')
      local doc = { '![img](pic.png)' }
      local m1 = QA.layout(doc, { width = 60, theme = 'light' })
      local m2 = QA.layout(doc, { width = 60, theme = 'light' })
      vim.cmd('set background=light')
      local m3 = QA.layout(doc, { width = 60, theme = 'light' })
      a.eq(m1, m2)
      a.eq(m1, m3, 'a fixed theme in opts gives the same model whatever the background')
    end
  )

  -- R-075 ------------------------------------------------------------------

  QA.check(
    'R-075',
    'Changing the background re-themes an open preview to the other palette',
    function()
      vim.cmd('set background=light')
      setup({})
      local win = U.open_preview({ '# Title', '', 'text' })
      a.le(0.1, U.luminance(fg(groups.heading, win)), 'light before')
      vim.cmd('set background=dark')
      QA.wait(1000, function()
        return U.luminance(fg(groups.heading, win) or 0) >= 0.6
      end)
      a.ge(0.6, U.luminance(fg(groups.heading, win)), 'dark after the change')
      vim.cmd('set background=light')
      QA.wait(1000, function()
        return U.luminance(fg(groups.heading, win) or 0xffffff) <= 0.1
      end)
      a.le(0.1, U.luminance(fg(groups.heading, win)), 'light again')
    end
  )

  QA.check(
    'R-075',
    'Changing the background re-themes every open preview, not only the current one',
    function()
      vim.cmd('set background=light')
      setup({})
      local w1 = U.open_preview({ '# One' })
      local cur = vim.api.nvim_get_current_win()
      vim.cmd('tabnew')
      local w2 = U.open_preview({ '# Two' }, 'split')
      a.ne(w1, w2)
      vim.cmd('set background=dark')
      QA.wait(1000, function()
        return U.luminance(fg(groups.heading, w1) or 0) >= 0.6
          and U.luminance(fg(groups.heading, w2) or 0) >= 0.6
      end)
      a.ge(0.6, U.luminance(fg(groups.heading, w1)), 'first preview')
      a.ge(0.6, U.luminance(fg(groups.heading, w2)), 'second preview')
      pcall(vim.api.nvim_set_current_win, cur)
    end
  )

  QA.check(
    'R-075',
    'Re-theming keeps the preview content and its highlights',
    { severity = 'medium' },
    function()
      vim.cmd('set background=light')
      setup({})
      local win, pbuf = U.open_preview({ '# Title', '', 'some text here' })
      local before = vim.api.nvim_buf_get_lines(pbuf, 0, -1, false)
      vim.cmd('set background=dark')
      QA.wait(300, function()
        return false
      end)
      a.truthy(vim.api.nvim_win_is_valid(win), 'preview window still open')
      a.eq(before, vim.api.nvim_buf_get_lines(pbuf, 0, -1, false))
      local marks = vim.api.nvim_buf_get_extmarks(pbuf, -1, 0, -1, { details = true })
      local n = 0
      for _, m in ipairs(marks) do
        if m[4] and m[4].hl_group then
          n = n + 1
        end
      end
      a.ge(1, n, 'highlight extmarks after re-theme')
    end
  )

  -- R-076 ------------------------------------------------------------------

  QA.check(
    'R-076',
    'A user highlight for a KittyMarkdown* group set before setup and open is kept',
    function()
      a.not_nil(groups.heading, 'heading group')
      vim.api.nvim_set_hl(0, groups.heading, { fg = '#ff00aa' })
      setup({})
      local win = U.open_preview({ '# Title' })
      a.eq(0xff00aa, fg(groups.heading, win))
      a.eq(0xff00aa, fg(groups.heading))
    end
  )

  QA.check(
    { 'R-076', 'R-281' },
    'A user highlight set after setup but before the preview opens is kept',
    function()
      setup({})
      vim.api.nvim_set_hl(0, groups.heading, { fg = '#00aa77' })
      local win = U.open_preview({ '# Title' })
      a.eq(0x00aa77, fg(groups.heading, win))
    end
  )

  QA.check('R-076', 'A user highlight link for a KittyMarkdown* group is kept', function()
    a.not_nil(groups.link, 'link group')
    vim.api.nvim_set_hl(0, 'QaUserLinkTarget', { fg = '#123456' })
    vim.api.nvim_set_hl(0, groups.link, { link = 'QaUserLinkTarget' })
    setup({})
    U.open_preview({ '[l](https://x.y)' })
    local raw = vim.api.nvim_get_hl(0, { name = groups.link })
    a.eq('QaUserLinkTarget', raw.link)
  end)

  QA.check(
    'R-076',
    'Keeping one user group still defines the other KittyMarkdown* groups',
    function()
      vim.api.nvim_set_hl(0, groups.heading, { fg = '#ff00aa' })
      setup({})
      U.open_preview({ '# Title', '', '[l](https://x.y)' })
      a.not_nil(fg(groups.link), 'link group defined')
      a.ne(0xff00aa, fg(groups.link))
    end
  )

  QA.check(
    'R-076',
    'Setup called twice does not take over a user group',
    { severity = 'medium' },
    function()
      vim.api.nvim_set_hl(0, groups.heading, { fg = '#ff00aa' })
      setup({})
      setup({ theme = 'dark' })
      U.open_preview({ '# Title' })
      a.eq(0xff00aa, fg(groups.heading))
    end
  )

  -- R-281 ------------------------------------------------------------------

  QA.check(
    'R-281',
    'A user highlight set while a preview is open survives a background change',
    function()
      setup({})
      local win = U.open_preview({ '# Title' })
      vim.api.nvim_set_hl(0, groups.heading, { fg = '#aa5500' })
      vim.cmd('set background=dark')
      QA.wait(300, function()
        return false
      end)
      a.eq(0xaa5500, fg(groups.heading, win))
      a.eq(0xaa5500, fg(groups.heading))
    end
  )

  QA.check(
    'R-281',
    'A user highlight set while a preview is open survives opening another preview',
    function()
      setup({})
      U.open_preview({ '# One' })
      vim.api.nvim_set_hl(0, groups.heading, { fg = '#5500aa' })
      vim.cmd('tabnew')
      local w2 = U.open_preview({ '# Two' })
      a.eq(0x5500aa, fg(groups.heading, w2))
    end
  )

  QA.check('R-281', 'A user highlight set after setup survives a second setup call', function()
    setup({})
    vim.api.nvim_set_hl(0, groups.heading, { fg = '#0055aa' })
    setup({ theme = 'dark' })
    a.eq(0x0055aa, fg(groups.heading))
  end)

  -- R-282 ------------------------------------------------------------------

  local function undefined_groups()
    local missing = {}
    for _, g in ipairs({ groups.heading, groups.link, groups.code, groups.math, groups.html }) do
      if g and next(U.hl(g)) == nil then
        missing[#missing + 1] = g
      end
    end
    return missing
  end

  QA.check(
    'R-282',
    'After a colorscheme loads, the KittyMarkdown* groups are defined again',
    function()
      setup({})
      U.open_preview({ '# Title', '', '[l](https://x.y)' })
      vim.cmd('colorscheme default')
      QA.wait(300, function()
        return #undefined_groups() == 0
      end)
      a.eq({}, undefined_groups())
    end
  )

  QA.check(
    'R-282',
    'After a colorscheme loads with no preview open yet, the groups are defined again',
    { severity = 'medium' },
    function()
      setup({})
      vim.cmd('colorscheme default')
      QA.wait(300, function()
        return #undefined_groups() == 0
      end)
      a.eq({}, undefined_groups())
    end
  )

  QA.check(
    'R-282',
    'A KittyMarkdown* group that the colorscheme defines keeps the colorscheme definition',
    function()
      setup({})
      U.open_preview({ '# Title' })
      local dir = QA.tmpdir()
      QA.write(
        dir .. '/colors/qacolors.lua',
        table.concat({
          'vim.cmd("hi clear")',
          'vim.g.colors_name = "qacolors"',
          ('vim.api.nvim_set_hl(0, %q, { fg = "#abcdef" })'):format(groups.heading),
        }, '\n')
      )
      vim.opt.runtimepath:prepend(dir)
      vim.cmd('colorscheme qacolors')
      QA.wait(300, function()
        return false
      end)
      a.eq(0xabcdef, fg(groups.heading))
      a.not_nil(fg(groups.link), 'a group the colorscheme left out is defined again')
    end
  )

  -- R-283 ------------------------------------------------------------------

  local GITHUB_BG = { light = 0xffffff, dark = 0x0d1117 }

  local function contrast(c1, c2)
    local l1, l2 = U.luminance(c1), U.luminance(c2)
    if l1 < l2 then
      l1, l2 = l2, l1
    end
    return (l1 + 0.05) / (l2 + 0.05)
  end

  for _, t in ipairs({ 'light', 'dark' }) do
    QA.check(
      'R-283',
      'Text and link colors have a contrast of at least 4.5:1 against the '
        .. t
        .. ' palette background',
      function()
        setup({ theme = t })
        local win = U.open_preview({ '# Title', '', 'text [l](https://x.y)' })
        local text_group = (vim.wo[win].winhighlight or ''):match('Normal:([%w_]+)')
          or groups.heading
        local text = U.win_hl(win, text_group)
        a.not_nil(text.fg, 'text color of ' .. text_group)
        local bg = text.bg or GITHUB_BG[t]
        local link = fg(groups.link, win)
        a.not_nil(link, 'link color')
        a.ge(4.5, contrast(text.fg, bg), ('text %06x on %06x'):format(text.fg, bg))
        a.ge(4.5, contrast(link, bg), ('link %06x on %06x'):format(link, bg))
      end
    )
  end
end)
