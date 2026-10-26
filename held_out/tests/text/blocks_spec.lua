local QA = require('qa')
local T = require('text_util')
local a = QA.assert

local function first_char(s)
  local t = vim.trim(s)
  local list = vim.fn.str2list(t)
  return list[1] and vim.fn.nr2char(list[1]) or '', t
end

-- Display column (0-based) where `needle` starts on a line.
local function dcol(line, needle)
  local s = line:find(needle, 1, true)
  if not s then
    return nil
  end
  return T.dw(line:sub(1, s - 1))
end

-- Display column of the first non-space character.
local function lead(line)
  return T.dw(line:match('^(%s*)'))
end

QA.describe('text: headings', function()
  QA.before_each(function()
    QA.reset_plugin()
    T.ensure_theme()
  end)

  local SIX = { '# One', '## Two', '### Three', '#### Four', '##### Five', '###### Six' }
  local NAMES = { 'One', 'Two', 'Three', 'Four', 'Five', 'Six' }

  QA.check(
    'R-027',
    'Each heading level 1 to 6 has its own KittyMarkdown highlight group over the heading text',
    function()
      local model = T.layout(SIX, 80)
      local seen = {}
      for lvl, name in ipairs(NAMES) do
        local every = T.plugin_groups((T.groups_over(model, name)))
        a.ge(1, #every, 'a KittyMarkdown group over heading level ' .. lvl)
        local key = table.concat(vim.fn.sort(every), ',')
        a.is_nil(
          seen[key],
          'level ' .. lvl .. ' shares its group with level ' .. tostring(seen[key])
        )
        seen[key] = lvl
      end
    end
  )

  QA.check('R-027', 'Each heading shows a Nerd Font icon before its text', function()
    local model = T.layout(SIX, 80)
    for lvl, name in ipairs(NAMES) do
      local ln = T.find(model, name)
      a.not_nil(ln, name)
      local before = model.lines[ln]:sub(1, model.lines[ln]:find(name, 1, true) - 1)
      a.truthy(T.has_pua(before), 'Nerd Font (private use) icon before heading level ' .. lvl)
    end
  end)

  QA.check(
    'R-027',
    'A setext heading uses the same group and icon as the ATX heading of its level',
    function()
      local atx = T.layout({ '# Alpha', '', '## Beta' }, 80)
      local setext = T.layout({ 'Alpha', '=====', '', 'Beta', '----' }, 80)
      for _, name in ipairs({ 'Alpha', 'Beta' }) do
        local g1 = vim.fn.sort(T.plugin_groups((T.groups_over(atx, name))))
        local g2 = vim.fn.sort(T.plugin_groups((T.groups_over(setext, name))))
        a.eq(g1, g2, 'groups of ' .. name)
        local l1 = atx.lines[T.find(atx, name)]
        local l2 = setext.lines[T.find(setext, name)]
        a.eq(l1, l2, 'rendered line of ' .. name)
      end
    end
  )

  QA.check(
    'R-027',
    'Inline styles inside a heading keep the heading group on the whole heading text',
    { severity = 'medium' },
    function()
      local model = T.layout({ '## Plain *styled* `code` end' }, 80)
      local hg = T.plugin_groups((T.groups_over(model, 'Plain')))
      a.ge(1, #hg)
      for _, w in ipairs({ 'styled', 'code', 'end' }) do
        local _, union = T.groups_over(model, w)
        local shared = false
        for _, g in ipairs(hg) do
          if vim.tbl_contains(union, g) then
            shared = true
          end
        end
        a.truthy(shared, 'heading group on ' .. w)
      end
    end
  )

  QA.check('R-028', 'ATX heading markers, including closing hashes, are not shown', function()
    local model =
      T.layout({ '# Title one #', '', '### Title three ###', '', '#No space is a paragraph' }, 80)
    local body = T.nonempty(model)
    a.no_match('#', body[1])
    a.no_match('#', body[2])
    a.matches('Title one$', body[1])
    a.matches('Title three$', body[2])
    a.eq('#No space is a paragraph', body[3], 'text without a space after # is not a heading')
  end)

  QA.check('R-028', 'Setext heading underlines are not shown', function()
    local model =
      T.layout({ 'Big title', '=========', '', 'Small title', '-----------', '', 'para' }, 80)
    for _, l in ipairs(model.lines) do
      a.no_match('===', l)
      a.no_match('%-%-%-', l)
    end
    a.eq(3, #T.nonempty(model), 'two headings and a paragraph')
  end)

  QA.check(
    'R-028',
    'A heading with emphasis or a link loses those markers too',
    { severity = 'medium' },
    function()
      local model = T.layout({ '# A **b** [c](http://d.e/)' }, 80)
      local body = T.nonempty(model)[1]
      a.no_match('[#%*%[%]%(%)]', body)
      a.matches('A b c$', body)
    end
  )
end)

QA.describe('text: lists', function()
  QA.before_each(function()
    QA.reset_plugin()
  end)

  QA.check(
    'R-039',
    'Unordered items with any marker show a bullet icon in place of the marker',
    function()
      local model = T.layout({ '- dash', '', '* star', '', '+ plus' }, 80)
      for _, w in ipairs({ 'dash', 'star', 'plus' }) do
        local ln = T.find(model, w)
        a.not_nil(ln, w)
        local c, t = first_char(model.lines[ln])
        a.no_match('^[-*+]', t, 'marker shown for ' .. w)
        a.truthy(T.has_non_ascii(c), 'bullet icon before ' .. w)
        a.ne(w:sub(1, 1), c, 'something precedes the item text')
      end
    end
  )

  QA.check(
    'R-039',
    'Items of the same list share one bullet icon',
    { severity = 'medium' },
    function()
      local model = T.layout({ '- one', '- two', '- three' }, 80)
      local c = {}
      for _, l in ipairs(T.nonempty(model)) do
        c[#c + 1] = first_char(l)
      end
      a.eq(c[1], c[2])
      a.eq(c[1], c[3])
    end
  )

  QA.check(
    'R-039',
    'A list item that wraps keeps its text in the text width and shows one bullet',
    { severity = 'medium' },
    function()
      local w = 30
      local item = string.rep('word ', 20)
      local model = T.layout({ '- ' .. item }, w)
      local tw = T.text_width(w)
      local bullets = 0
      local c0 = first_char(T.nonempty(model)[1])
      for _, l in ipairs(T.nonempty(model)) do
        a.le(tw, T.dw(l), 'wrapped item line width')
        if first_char(l) == c0 then
          bullets = bullets + 1
        end
      end
      a.eq(1, bullets, 'one bullet for one item')
    end
  )

  QA.check(
    'R-040',
    'Ordered items show their number followed by a period for both marker styles',
    function()
      local model = T.layout({ '1. first', '2. second', '', 'para', '', '1) uno', '2) dos' }, 80)
      local body = T.nonempty(model)
      a.eq('1. first', body[1])
      a.eq('2. second', body[2])
      a.eq('1. uno', body[4])
      a.eq('2. dos', body[5])
    end
  )

  QA.check('R-040', 'An ordered list keeps its start number and counts up from it', function()
    local model = T.layout({ '7. seven', '8. eight', '9. nine', '10. ten' }, 80)
    a.eq({ '7. seven', '8. eight', '9. nine', '10. ten' }, T.nonempty(model))
  end)

  QA.check(
    'R-040',
    'An ordered list written with one repeated number shows each item with its position number',
    { severity = 'medium' },
    function()
      local model = T.layout({ '1. a', '1. b', '1. c' }, 80)
      a.eq({ '1. a', '2. b', '3. c' }, T.nonempty(model))
    end
  )

  QA.check('R-041', 'Each nested list level is indented 2 cells more than its parent', function()
    local model = T.layout({ '- top', '  - mid', '    - low', '      - deepest' }, 80)
    local cols = {}
    for _, w in ipairs({ 'top', 'mid', 'low', 'deepest' }) do
      local ln = T.find(model, w)
      a.not_nil(ln, w)
      cols[#cols + 1] = { marker = lead(model.lines[ln]), text = dcol(model.lines[ln], w) }
    end
    for i = 2, #cols do
      local d_marker = cols[i].marker - cols[i - 1].marker
      local d_text = cols[i].marker - cols[i - 1].text
      a.truthy(
        d_marker == 2 or d_text == 2,
        ('level %d indent: %d from the parent marker, %d from the parent text'):format(
          i,
          d_marker,
          d_text
        )
      )
    end
  end)

  QA.check('R-041', 'Nested ordered and unordered lists indent 2 cells per level', function()
    local model = T.layout({ '1. top', '   - mid', '     1. low' }, 80)
    local cols = {}
    for _, w in ipairs({ 'top', 'mid', 'low' }) do
      local ln = T.find(model, w)
      a.not_nil(ln, w)
      cols[#cols + 1] = { marker = lead(model.lines[ln]), text = dcol(model.lines[ln], w) }
    end
    for i = 2, #cols do
      local d_marker = cols[i].marker - cols[i - 1].marker
      local d_text = cols[i].marker - cols[i - 1].text
      a.truthy(
        d_marker == 2 or d_text == 2,
        ('level %d indent: %d from parent marker, %d from parent text'):format(i, d_marker, d_text)
      )
    end
  end)

  QA.check(
    'R-041',
    'Indentation of nested items does not depend on how far the source indents them',
    { severity = 'medium' },
    function()
      local m2 = T.layout({ '- top', '  - child' }, 80)
      local m4 = T.layout({ '- top', '    - child' }, 80)
      a.eq(m2.lines, m4.lines)
    end
  )

  QA.check('R-041', 'Siblings after a nested list return to their own level', function()
    local model = T.layout({ '- one', '  - nested', '- two' }, 80)
    a.eq(lead(model.lines[T.find(model, 'one')]), lead(model.lines[T.find(model, 'two')]))
  end)

  QA.check(
    'R-042',
    'A task item shows a checkbox icon in place of both the bullet and the task marker',
    function()
      local plain = T.layout({ '- plain item' }, 80)
      local bullet = first_char(T.nonempty(plain)[1])
      local model = T.layout({ '- [x] done thing', '- [ ] open thing' }, 80)
      local body = T.nonempty(model)
      a.eq(2, #body)
      for _, l in ipairs(body) do
        a.no_match('%[[ xX]%]', l, 'task marker shown')
        a.not_contains(bullet, l, 'bullet icon shown on a task item')
      end
      a.matches('done thing$', body[1])
      a.matches('open thing$', body[2])
    end
  )

  QA.check('R-042', 'Checked and unchecked task items show different checkbox icons', function()
    local model = T.layout({ '- [x] done thing', '- [ ] open thing' }, 80)
    local body = T.nonempty(model)
    local i1 = body[1]:sub(1, body[1]:find('done', 1, true) - 1)
    local i2 = body[2]:sub(1, body[2]:find('open', 1, true) - 1)
    a.truthy(T.has_non_ascii(i1), 'checked icon')
    a.truthy(T.has_non_ascii(i2), 'unchecked icon')
    a.ne(vim.trim(i1), vim.trim(i2))
  end)

  QA.check('R-042', 'An uppercase X marks a task item checked', { severity = 'low' }, function()
    local lower = T.nonempty(T.layout({ '- [x] t' }, 80))[1]
    local upper = T.nonempty(T.layout({ '- [X] t' }, 80))[1]
    a.eq(lower, upper)
  end)

  QA.check(
    'R-042',
    'A task item in an ordered list shows its checkbox and no task marker',
    { severity = 'medium' },
    function()
      local model = T.layout({ '1. [x] first', '2. [ ] second' }, 80)
      for _, l in ipairs(T.nonempty(model)) do
        a.no_match('%[[ xX]%]', l)
      end
    end
  )

  QA.check(
    'R-042',
    'Brackets that are not at the start of an item stay as written',
    { severity = 'medium' },
    function()
      local model = T.layout({ '- text [x] later' }, 80)
      a.contains('text [x] later', T.nonempty(model)[1])
    end
  )
end)

QA.describe('text: blockquotes and alerts', function()
  QA.before_each(function()
    QA.reset_plugin()
    T.ensure_theme()
  end)

  local BARS = { '▎', '│', '┃', '▏', '▍', '▌', '|', '▐', '║' }
  local function bar_of(line)
    local c = first_char(line)
    return vim.tbl_contains(BARS, c) and c or nil
  end

  QA.check(
    'R-051',
    'Each blockquote line starts with a vertical bar and shows no quote marker',
    function()
      local model = T.layout({ '> first line', '> second line', '>', '> third para' }, 80)
      for _, l in ipairs(model.lines) do
        if l:find('%a') then
          a.not_nil(bar_of(l), 'bar on ' .. l)
          a.no_match('>', l)
        end
      end
    end
  )

  QA.check('R-051', 'The quote bar is in the left margin, before the quoted text', function()
    local model = T.layout({ '> quoted words here' }, 80)
    local l = model.lines[T.find(model, 'quoted')]
    local bc = dcol(l, bar_of(l) or '\1')
    a.not_nil(bc, 'a bar')
    a.truthy(bc < dcol(l, 'quoted'), 'bar before the text')
  end)

  QA.check(
    'R-051',
    'Quoted text uses a quote highlight group that plain text does not use',
    function()
      local model = T.layout({ 'plain words', '', '> quoted words' }, 80)
      local q = T.plugin_groups((T.groups_over(model, 'quoted')))
      a.ge(1, #q, 'a KittyMarkdown group over the quote text')
      local _, p = T.groups_over(model, 'plain')
      for _, g in ipairs(q) do
        a.not_contains(g, p)
      end
    end
  )

  QA.check(
    'R-051',
    'A wrapped quote line keeps the bar on every rendered line and fits the text width',
    function()
      local w = 30
      local model = T.layout({ '> ' .. string.rep('long quote text ', 10) }, w)
      local n = 0
      for _, l in ipairs(model.lines) do
        if l:find('%a') then
          n = n + 1
          a.not_nil(bar_of(l), 'bar on wrapped line')
          a.le(T.text_width(w), T.dw(vim.trim(l)), 'quote line width')
        end
      end
      a.ge(3, n)
    end
  )

  QA.check(
    'R-051',
    'A lazy continuation line belongs to the blockquote',
    { severity = 'medium' },
    function()
      local model = T.layout({ '> start of quote', 'lazy continuation' }, 80)
      local l = model.lines[T.find(model, 'lazy')]
      a.not_nil(bar_of(l), 'bar on the lazy line')
    end
  )

  QA.check(
    'R-051',
    'A nested blockquote shows a bar for each level',
    { severity = 'low' },
    function()
      local model = T.layout({ '> outer', '>> inner' }, 80)
      local l = model.lines[T.find(model, 'inner')]
      local n = 0
      for _, c in ipairs(vim.fn.split(l:sub(1, l:find('inner', 1, true) - 1), '\\zs')) do
        if vim.tbl_contains(BARS, c) then
          n = n + 1
        end
      end
      a.eq(2, n)
    end
  )

  local KINDS = { 'NOTE', 'TIP', 'IMPORTANT', 'WARNING', 'CAUTION' }

  local function alert(kind)
    return T.layout({ '> [!' .. kind .. ']', '> body of the alert' }, 80)
  end

  QA.check('R-052', 'Each GitHub alert kind shows its title and no alert marker', function()
    for _, k in ipairs(KINDS) do
      local model = alert(k)
      local text = T.text(model)
      a.not_contains('[!', text, k .. ' marker shown')
      local title = k:sub(1, 1) .. k:sub(2):lower()
      a.truthy(text:find(title, 1, true) or text:find(k, 1, true), k .. ' title')
      a.contains('body of the alert', text)
    end
  end)

  QA.check(
    'R-052',
    'Each GitHub alert kind shows an icon before its title, and the five icons differ',
    function()
      local icons = {}
      for _, k in ipairs(KINDS) do
        local model = alert(k)
        local title = k:sub(1, 1) .. k:sub(2):lower()
        local ln = T.find(model, title) or T.find(model, k)
        a.not_nil(ln, k)
        local l = model.lines[ln]
        local before = l:sub(1, (l:find(title, 1, true) or l:find(k, 1, true)) - 1)
        a.truthy(
          T.has_pua(before) or T.has_non_ascii(before:gsub('[▎│┃▏▍▌|]', '')),
          k .. ' icon'
        )
        icons[k] = before
      end
      local seen = {}
      for k, v in pairs(icons) do
        a.is_nil(seen[v], k .. ' icon equals ' .. tostring(seen[v]))
        seen[v] = k
      end
    end
  )

  QA.check('R-052', 'Each GitHub alert kind shows its title in a distinct color', function()
    local colors = {}
    for _, k in ipairs(KINDS) do
      local model = alert(k)
      local title = k:sub(1, 1) .. k:sub(2):lower()
      local needle = T.find(model, title) and title or k
      local every = T.plugin_groups((T.groups_over(model, needle)))
      a.ge(1, #every, k .. ' title group')
      local fg
      for _, g in ipairs(every) do
        fg = fg or T.style(g).fg
      end
      a.not_nil(fg, k .. ' title color')
      a.is_nil(colors[fg], k .. ' shares its color with ' .. tostring(colors[fg]))
      colors[fg] = k
    end
  end)

  QA.check(
    'R-052',
    'A blockquote with an unknown alert word renders as a plain blockquote, not an alert',
    { severity = 'medium' },
    function()
      local quote = T.layout({ '> [!INFO]', '> body text' }, 80)
      local text = T.text(quote)
      a.contains('[!INFO]', text, 'unknown alert kind shown as written')
      a.not_contains('Info', text:gsub('%[!INFO%]', ''), 'no alert title')
    end
  )

  QA.check(
    'R-052',
    'An alert with several body paragraphs keeps every paragraph inside the alert',
    { severity = 'medium' },
    function()
      local model = T.layout({ '> [!WARNING]', '> first part', '>', '> second part' }, 80)
      local first = model.lines[T.find(model, 'first part')]
      local second = model.lines[T.find(model, 'second part')]
      a.eq(
        first:match('^(.-)first'),
        second:match('^(.-)second'),
        'same alert prefix on both paragraphs'
      )
    end
  )
end)

QA.describe('text: thematic breaks', function()
  QA.before_each(function()
    QA.reset_plugin()
    T.ensure_theme()
  end)

  local function rule_line(model)
    for i, l in ipairs(model.lines) do
      local t = vim.trim(l)
      if
        t ~= ''
        and not t:find('[%w]')
        and (t:find('─', 1, true) or t:find('━', 1, true) or t:find('—', 1, true))
      then
        return i, t
      end
    end
  end

  for _, w in ipairs({ 30, 60, 104, 160 }) do
    QA.check(
      'R-055',
      'A thematic break is a horizontal rule exactly as wide as the text width',
      function()
        local model = T.layout({ 'above', '', '---', '', 'below' }, w)
        local _, t = rule_line(model)
        a.not_nil(t, 'a rule line')
        a.eq(T.text_width(w), T.dw(t), 'rule width at window width ' .. w)
      end
    )
  end

  QA.check('R-055', 'Every thematic break marker style renders the same rule', function()
    local ref = T.layout({ 'a', '', '---', '', 'b' }, 60).lines
    for _, m in ipairs({ '***', '___', '- - -', '* * * *', '_____' }) do
      a.eq(ref, T.layout({ 'a', '', m, '', 'b' }, 60).lines, m)
    end
  end)

  QA.check('R-055', 'A thematic break uses the muted color, the same as HTML text', function()
    local model = T.layout({ 'a', '', '---', '', 'b' }, 60)
    local ln = rule_line(model)
    a.not_nil(ln)
    local rule_fgs = T.fgs(T.groups_on_line(model, ln))
    local html = T.layout({ '<div>x</div>' }, 60)
    local html_fgs = T.fgs((T.groups_over(html, '<div>')))
    a.ge(1, #vim.tbl_keys(html_fgs), 'HTML has a color')
    a.ge(1, #vim.tbl_keys(rule_fgs), 'rule has a color')
    for fg in pairs(rule_fgs) do
      a.not_nil(html_fgs[fg], ('rule color %06x is not the muted color of HTML text'):format(fg))
    end
  end)
end)

QA.describe('text: front matter', function()
  QA.before_each(function()
    QA.reset_plugin()
  end)

  QA.check('R-059', 'YAML front matter at the start of the document is hidden', function()
    local model = T.layout(
      { '---', 'title: Hidden Title', 'tags: [a, b]', '---', '', '# Visible', '', 'text' },
      80
    )
    local text = T.text(model)
    a.not_contains('Hidden Title', text)
    a.not_contains('tags:', text)
    a.matches('Visible', model.lines[1], 'the first line is the heading')
  end)

  QA.check(
    'R-059',
    'Front matter followed directly by content leaves no blank line at the top',
    function()
      local model = T.layout({ '---', 'k: v', '---', 'First paragraph.' }, 80)
      a.eq('First paragraph.', vim.trim(model.lines[1]))
    end
  )

  QA.check(
    'R-059',
    'A thematic break later in the document is not treated as front matter',
    function()
      local model = T.layout({ 'intro', '', '---', '', 'middle', '', '---', '', 'end' }, 60)
      local text = T.text(model)
      a.contains('intro', text)
      a.contains('middle', text)
      a.contains('end', text)
    end
  )

  QA.check(
    'R-059',
    'A document that is only front matter renders no front matter text',
    { severity = 'medium' },
    function()
      local model = T.layout({ '---', 'k: onlyvalue', '---' }, 80)
      a.not_contains('onlyvalue', T.text(model))
    end
  )
end)

QA.describe('text: HTML', function()
  QA.before_each(function()
    QA.reset_plugin()
    T.ensure_theme()
  end)

  QA.check('R-060', 'An HTML block is shown as its plain source text in the muted color', function()
    local model =
      T.layout({ 'para', '', '<div align="center">', '  <b>inside</b>', '</div>', '', 'after' }, 80)
    local text = T.text(model)
    a.contains('<div align="center">', text)
    a.contains('<b>inside</b>', text)
    a.contains('</div>', text)
    local every = T.groups_over(model, '<b>inside</b>')
    a.ge(1, #vim.tbl_keys(T.fgs(every)), 'HTML block text has a color')
    local para = T.groups_over(model, 'para')
    a.falsy(T.any_attr(every, 'bold'), 'HTML tags are not interpreted')
    for fg, g in pairs(T.fgs(every)) do
      a.is_nil(T.fgs(para)[fg], 'HTML color equals plain text color ' .. g)
    end
  end)

  QA.check(
    'R-060',
    'Inline HTML tags are shown as plain text in the muted color, not interpreted',
    function()
      local model = T.layout({ 'a <kbd>Ctrl</kbd> and <em>x</em> b' }, 80)
      local text = T.nonempty(model)[1]
      a.eq('a <kbd>Ctrl</kbd> and <em>x</em> b', text)
      local html = T.groups_over(T.layout({ '<div>x</div>' }, 60), '<div>')
      local muted = T.fgs(html)
      local tag = T.fgs((T.groups_over(model, '<kbd>')))
      local found = false
      for fg in pairs(tag) do
        if muted[fg] then
          found = true
        end
      end
      a.truthy(found, 'inline tag in the muted color')
      a.falsy(T.any_attr(T.groups_over(model, 'x</em>'), 'italic'), '<em> is not interpreted')
    end
  )

  QA.check('R-060', 'An inline img tag is not shown as raw HTML text', function()
    local model = T.layout(
      { 'before <img src="pic.png" alt="a picture"> after' },
      80,
      { images = false }
    )
    a.not_contains('<img', T.text(model))
  end)

  QA.check('R-060', 'An img HTML block is not shown as raw HTML text', function()
    local model = T.layout(
      { 'x', '', '<img src="pic.png" alt="a picture" width="200">', '', 'y' },
      80,
      { images = false }
    )
    a.not_contains('<img', T.text(model))
  end)
end)

QA.describe('text: block spacing', function()
  QA.before_each(function()
    QA.reset_plugin()
  end)

  local function no_double_blank(lines)
    for i = 2, #lines do
      if lines[i] == '' and lines[i - 1] == '' then
        return false, i
      end
    end
    return true
  end

  QA.check(
    'R-061',
    'Blocks are separated by exactly one blank line and the buffer starts with content',
    function()
      local model = T.layout({
        '',
        '',
        '# Head',
        'para one',
        '',
        '',
        '',
        'para two',
        '- item',
        '',
        '> quote',
        '```',
        'code',
        '```',
        '---',
        'end',
      }, 80)
      local lines = QA.lines(model)
      a.ne('', lines[1], 'blank first line')
      local ok, at = no_double_blank(lines)
      a.truthy(ok, 'two blank lines in a row at line ' .. tostring(at))
      local order = { 'Head', 'para one', 'para two', 'item', 'quote', 'code' }
      local prev
      for _, w in ipairs(order) do
        local ln = T.find(model, w, prev and prev + 1 or 1)
        a.not_nil(ln, w)
        if prev then
          local blanks = 0
          for i = prev + 1, ln - 1 do
            if lines[i] == '' then
              blanks = blanks + 1
            end
          end
          a.eq(1, blanks, 'blank lines before ' .. w)
        end
        prev = ln
      end
    end
  )

  QA.check(
    'R-061',
    'A heading directly followed by a paragraph still gets one blank line between them',
    function()
      local model = T.layout({ '# Head', 'para' }, 80)
      a.eq(3, #QA.lines(model))
      a.eq('', QA.lines(model)[2])
    end
  )

  QA.check(
    'R-061',
    'Hidden front matter and hidden definitions leave no blank line at the top',
    function()
      local model = T.layout({ '[r]: http://x.y/', '', '', 'text [l][r]' }, 80)
      a.ne('', QA.lines(model)[1])
    end
  )
end)
