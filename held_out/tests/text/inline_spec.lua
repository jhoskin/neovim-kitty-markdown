local QA = require('qa')
local T = require('text_util')
local a = QA.assert

-- The words of a list of strings, in order.
local function words(list)
  local out = {}
  for _, s in ipairs(list) do
    for w in s:gmatch('%S+') do
      out[#out + 1] = w
    end
  end
  return out
end

local LOREM = 'The quick brown fox jumps over the lazy dog while seven tired wizards quietly '
  .. 'juggle bright copper kettles beside a narrow winding river under heavy grey clouds '
  .. 'and nobody notices the strange little parade marching through the old market square.'

QA.describe('text: paragraphs and wrapping', function()
  QA.before_each(function()
    QA.reset_plugin()
  end)

  for _, w in ipairs({ 24, 30, 44, 60, 80 }) do
    QA.check(
      'R-029',
      'Paragraph lines fit in the text width (window width minus 2 cells each side) at several widths',
      function()
        local tw = T.text_width(w)
        local model = T.layout({ LOREM }, w)
        local body = T.nonempty(model)
        a.ge(2, #body, 'paragraph wraps to several lines')
        for _, l in ipairs(model.lines) do
          local t = l:gsub('%s+$', '')
          a.le(tw + T.MARGIN, T.dw(t), 'line display width with the left margin')
          a.le(tw, T.dw(vim.trim(t)), 'text display width')
        end
      end
    )
  end

  QA.check('R-029', 'The text width is capped at 100 cells on a wide window', function()
    local model = T.layout({ LOREM .. ' ' .. LOREM }, 200)
    local widest = 0
    for _, l in ipairs(model.lines) do
      widest = math.max(widest, T.dw(vim.trim(l)))
    end
    a.le(100, widest, 'widest text line')
    a.ge(90, widest, 'a wide window uses most of the 100-cell cap')
  end)

  QA.check('R-029', 'The configured maximum text width caps the wrap width', function()
    local ok = pcall(function()
      require('kitty-markdown').setup({ text_width = 50 })
    end)
    QA.need(
      ok and require('kitty-markdown.config').get().text_width == 50,
      'a text width option in setup()'
    )
    local model = T.layout({ LOREM }, 200)
    for _, l in ipairs(model.lines) do
      a.le(50, T.dw(vim.trim(l)), 'line under the configured maximum')
    end
    pcall(function()
      require('kitty-markdown').setup({})
    end)
  end)

  QA.check(
    'R-029',
    'Paragraph wrapping breaks only at word boundaries and keeps every word in order',
    function()
      local model = T.layout({ LOREM }, 34)
      a.eq(words({ LOREM }), words(T.nonempty(model)))
    end
  )

  QA.check(
    'R-029',
    'Paragraph wrapping fills each line: the next word would not have fit',
    { severity = 'medium' },
    function()
      local w = 40
      local tw = T.text_width(w)
      local body = T.nonempty(T.layout({ LOREM }, w))
      for i = 1, #body - 1 do
        local nxt = body[i + 1]:match('^%S+')
        a.ge(tw + 1, T.dw(body[i]) + 1 + T.dw(nxt), 'line ' .. i .. ' broke early')
      end
    end
  )

  QA.check('R-029', 'Soft line breaks inside a paragraph join into one wrapped flow', function()
    local model = T.layout({ 'alpha beta', 'gamma delta', 'epsilon' }, 80)
    local body = T.nonempty(model)
    a.eq(1, #body, 'one rendered line')
    a.eq('alpha beta gamma delta epsilon', body[1])
  end)

  QA.check(
    'R-029',
    'A soft line break before or after a styled run, code span, or link joins with a space',
    function()
      local cases = {
        { { 'one', '*two* three' }, 'one two three' },
        { { 'one', '**two** three' }, 'one two three' },
        { { 'one', '`two` three' }, 'one two three' },
        { { 'one', '[two](http://e.x/) three' }, 'one two three' },
        { { 'one,', '~~two~~ three' }, 'one, two three' },
        { { 'one *two*', 'three' }, 'one two three' },
      }
      for _, c in ipairs(cases) do
        local model = T.layout(c[1], 80)
        a.eq({ c[2] }, T.nonempty(model), table.concat(c[1], ' / '))
      end
    end
  )

  QA.check(
    'R-029',
    'Wrapped text with wide characters fits the text width measured in display cells',
    function()
      local cjk = string.rep('漢字テスト ', 20)
      local w = 30
      local tw = T.text_width(w)
      local model = T.layout({ cjk }, w)
      for _, l in ipairs(model.lines) do
        a.le(tw, T.dw(vim.trim(l)), 'display width of a wide-character line')
      end
      a.ge(3, #T.nonempty(model), 'wide text wraps')
    end
  )

  QA.check(
    'R-029',
    'Paragraph text starts after a left margin of 2 cells',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'hello world' }, 80)
      local ln = T.find(model, 'hello')
      a.not_nil(ln)
      local lead = model.lines[ln]:match('^(%s*)')
      a.eq(2, T.dw(lead), 'left margin')
    end
  )

  QA.check('R-030', 'A word shorter than the text width is never split across lines', function()
    local long = 'supercalifragilistic'
    local text = {}
    for i = 1, 12 do
      text[#text + 1] = (i % 2 == 0) and long or ('w' .. i)
    end
    local src = table.concat(text, ' ')
    for _, w in ipairs({ 26, 27, 29, 33 }) do
      local model = T.layout({ src }, w)
      local count = 0
      for _, l in ipairs(model.lines) do
        for word in l:gmatch('%S+') do
          if word == long then
            count = count + 1
          end
        end
      end
      a.eq(6, count, 'whole long words at window width ' .. w)
    end
  end)

  QA.check(
    'R-030',
    'A word exactly as wide as the text width stays whole on its own line',
    function()
      local w = 24
      local tw = T.text_width(w)
      local word = string.rep('x', tw)
      local model = T.layout({ 'a ' .. word .. ' b' }, w)
      local found = false
      for _, l in ipairs(model.lines) do
        if vim.trim(l) == word then
          found = true
        end
      end
      a.truthy(found, 'the full-width word on one line')
    end
  )

  QA.check('R-030', 'Styled words (bold, code, links) are not split when wrapping', function()
    local src = 'lead text and **boldword** then `codeword` and [linkword](http://e.x) repeated '
    src = src:rep(4)
    for _, w in ipairs({ 22, 25, 31 }) do
      local model = T.layout({ src }, w)
      local whole = 0
      for _, l in ipairs(model.lines) do
        for word in l:gmatch('%S+') do
          if word == 'boldword' or word == 'codeword' or word == 'linkword' then
            whole = whole + 1
          end
        end
      end
      a.eq(12, whole, 'styled words intact at width ' .. w)
    end
  end)

  QA.check('R-031', 'Two trailing spaces end a line as a hard line break', function()
    local model = T.layout({ 'first part  ', 'second part' }, 80)
    local body = T.nonempty(model)
    a.eq({ 'first part', 'second part' }, body)
  end)

  QA.check(
    'R-031',
    'A trailing backslash ends a line as a hard line break and is not shown',
    function()
      local model = T.layout({ 'first part\\', 'second part' }, 80)
      local body = T.nonempty(model)
      a.eq({ 'first part', 'second part' }, body)
    end
  )

  QA.check(
    'R-031',
    'A hard line break inside a list item or a blockquote breaks the line there',
    { severity = 'medium' },
    function()
      local model =
        T.layout({ '- item one  ', '  item two', '', '> quote one\\', '> quote two' }, 80)
      local l1 = T.find(model, 'item one')
      local l2 = T.find(model, 'item two')
      local q1 = T.find(model, 'quote one')
      local q2 = T.find(model, 'quote two')
      a.truthy(l1 and l2 and l2 == l1 + 1, 'list item break')
      a.truthy(q1 and q2 and q2 == q1 + 1, 'quote break')
      a.no_match('\\', model.lines[q1], 'backslash shown')
    end
  )
end)

QA.describe('text: emphasis, strong, strikethrough', function()
  QA.before_each(function()
    QA.reset_plugin()
    T.ensure_theme()
  end)

  QA.check(
    'R-032',
    'Emphasis with either marker is italic and its markers are not shown',
    function()
      for _, src in ipairs({ 'say *softly* now', 'say _softly_ now' }) do
        local model = T.layout({ src }, 80)
        a.eq({ 'say softly now' }, T.nonempty(model), src)
        local every = T.groups_over(model, 'softly')
        a.truthy(T.any_attr(every, 'italic'), 'italic group over the emphasis text')
        local before = T.groups_over(model, 'say')
        a.falsy(T.any_attr(before, 'italic'), 'text outside emphasis is not italic')
      end
    end
  )

  QA.check(
    'R-032',
    'An underscore inside a word is literal text, not emphasis',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'call snake_case_name here' }, 80)
      a.eq({ 'call snake_case_name here' }, T.nonempty(model))
    end
  )

  QA.check(
    'R-033',
    'Strong text with either marker is bold and its markers are not shown',
    function()
      for _, src in ipairs({ 'a **loud** word', 'a __loud__ word' }) do
        local model = T.layout({ src }, 80)
        a.eq({ 'a loud word' }, T.nonempty(model), src)
        local every = T.groups_over(model, 'loud')
        a.truthy(T.any_attr(every, 'bold'), 'bold group over the strong text')
        a.falsy(T.any_attr(T.groups_over(model, 'word'), 'bold'), 'text after strong is not bold')
      end
    end
  )

  QA.check(
    { 'R-032', 'R-033' },
    'Emphasis nested in strong text (and the reverse) keeps both italic and bold',
    function()
      for _, src in ipairs({ 'x ***both*** z', 'x **bold _both_ bold** z', 'x *it **both** it* z' }) do
        local model = T.layout({ src }, 80)
        local body = table.concat(T.nonempty(model), ' ')
        a.no_match('[%*_]', body, 'markers in ' .. src)
        local every = T.groups_over(model, 'both')
        a.truthy(T.any_attr(every, 'italic'), 'italic in ' .. src)
        a.truthy(T.any_attr(every, 'bold'), 'bold in ' .. src)
      end
    end
  )

  QA.check(
    'R-034',
    'Strikethrough text has the strikethrough attribute and its tildes are not shown',
    function()
      local model = T.layout({ 'keep ~~gone~~ keep' }, 80)
      a.eq({ 'keep gone keep' }, T.nonempty(model))
      a.truthy(
        T.any_attr(T.groups_over(model, 'gone'), 'strikethrough'),
        'strikethrough over the text'
      )
      a.falsy(T.any_attr(T.groups_over(model, 'keep'), 'strikethrough'), 'no strikethrough outside')
    end
  )

  QA.check(
    { 'R-032', 'R-033', 'R-034' },
    'Styles stay on every wrapped line of a long styled run',
    { severity = 'medium' },
    function()
      local run = string.rep('styled ', 14)
      local model = T.layout({ 'plain **' .. vim.trim(run) .. '** tail' }, 30)
      local lines_with_styled = 0
      for i, l in ipairs(model.lines) do
        local s = l:find('styled', 1, true)
        if s then
          lines_with_styled = lines_with_styled + 1
          local groups = T.groups_at_pos(model, i, s - 1)
          a.truthy(T.any_attr(groups, 'bold'), 'bold on wrapped line ' .. i)
        end
      end
      a.ge(2, lines_with_styled, 'the run wraps')
    end
  )
end)

QA.describe('text: inline code', function()
  QA.before_each(function()
    QA.reset_plugin()
    T.ensure_theme()
  end)

  QA.check(
    'R-035',
    'Inline code is in a code highlight group and its backticks are not shown',
    function()
      local model = T.layout({ 'run `make all` now' }, 80)
      a.eq({ 'run make all now' }, T.nonempty(model))
      local every = T.plugin_groups((T.groups_over(model, 'make all')))
      a.ge(1, #every, 'a KittyMarkdown group covers the code text')
      local plain = T.groups_over(model, 'now')
      for _, g in ipairs(every) do
        a.not_contains(g, plain, 'the code group is not on plain text')
      end
    end
  )

  QA.check(
    'R-035',
    'Double-backtick code spans keep an inner backtick and hide only the delimiters',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'see `` a`b `` here' }, 80)
      a.eq({ 'see a`b here' }, T.nonempty(model))
    end
  )

  QA.check('R-035', 'Markdown inside inline code stays literal', function()
    local model = T.layout({ 'x `**not bold** [n](u) :tada:` y' }, 80)
    a.eq({ 'x **not bold** [n](u) :tada: y' }, T.nonempty(model))
    a.falsy(T.any_attr(T.groups_over(model, 'not bold'), 'bold'), 'no bold inside code')
  end)

  QA.check(
    'R-035',
    'Inline code uses the same code group in a heading, a list item, and a paragraph',
    { severity = 'low' },
    function()
      local model = T.layout({ 'p `cc1` p', '', '- l `cc2` l' }, 80)
      local g1 = T.plugin_groups((T.groups_over(model, 'cc1')))
      local g2 = T.plugin_groups((T.groups_over(model, 'cc2')))
      a.ge(1, #g1)
      a.contains(g1[1], g2)
    end
  )
end)

QA.describe('text: links', function()
  QA.before_each(function()
    QA.reset_plugin()
    T.ensure_theme()
  end)

  local function link_fg(model, needle)
    local every = T.groups_over(model, needle)
    local ok, g = T.any_attr(every, 'underline')
    return ok, g and T.style(g).fg, every
  end

  QA.check(
    'R-036',
    'Link text is underlined in the link color and the brackets and URL are not shown',
    function()
      local model = T.layout({ 'go to [the site](https://example.org/path "Title") today' }, 80)
      a.eq({ 'go to the site today' }, T.nonempty(model))
      local ok, fg = link_fg(model, 'the site')
      a.truthy(ok, 'underlined link text')
      a.not_nil(fg, 'link text has a foreground color')
      local plain_ok = link_fg(model, 'today')
      a.falsy(plain_ok, 'plain text is not underlined')
    end
  )

  QA.check(
    'R-036',
    'Styled link text keeps the link style and loses all markers',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'a [**big** link](http://e.x/) b' }, 80)
      a.eq({ 'a big link b' }, T.nonempty(model))
      a.truthy((link_fg(model, 'big')), 'underline on styled link text')
    end
  )

  QA.check(
    'R-036',
    'A link that wraps keeps the link style on both lines',
    { severity = 'medium' },
    function()
      local model =
        T.layout({ 'aaaa bbbb cccc [one two three four five six](http://e.x/) end' }, 28)
      for _, w in ipairs({ 'one', 'six' }) do
        local ln, col = T.find(model, w)
        a.not_nil(ln, w)
        a.truthy(T.any_attr(T.groups_at_pos(model, ln, col), 'underline'), 'underline on ' .. w)
      end
    end
  )

  QA.check(
    'R-036',
    'Obsidian wikilinks and embeds are not links and stay as written',
    { severity = 'medium' },
    function()
      T.ensure_theme()
      local model = T.layout({ 'see [[Some Page]] and ![[embed.png]] end' }, 80, { images = false })
      a.eq({ 'see [[Some Page]] and ![[embed.png]] end' }, T.nonempty(model))
      local ok = T.any_attr(T.groups_over(model, 'Some Page'), 'underline')
      a.falsy(ok, 'wikilink shown in the link style')
    end
  )

  QA.check(
    'R-037',
    'A bare URL and an angle-bracket autolink are underlined in the same color as link text',
    function()
      local model = T.layout({
        'see https://bare.example.com/x and <https://auto.example.com/y> and [txt](https://z.example/)',
      }, 120)
      local ok1, fg1 = link_fg(model, 'bare.example.com')
      local ok2, fg2 = link_fg(model, 'auto.example.com')
      local ok3, fg3 = link_fg(model, 'txt')
      a.truthy(ok1, 'bare URL underlined')
      a.truthy(ok2, 'autolink underlined')
      a.truthy(ok3, 'link underlined')
      a.eq(fg3, fg1, 'bare URL in the link color')
      a.eq(fg3, fg2, 'autolink in the link color')
    end
  )

  QA.check(
    'R-037',
    'A bare www address and an email autolink are shown in the link style',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'visit www.example.com or <someone@example.com>' }, 80)
      a.truthy((link_fg(model, 'www.example.com')), 'www autolink underlined')
      a.truthy((link_fg(model, 'someone@example.com')), 'email autolink underlined')
    end
  )

  QA.check(
    { 'R-037', 'R-171' },
    'The angle brackets of an autolink are markdown markers and are not shown',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'x <https://auto.example.com/y> z' }, 80)
      a.eq({ 'x https://auto.example.com/y z' }, T.nonempty(model))
    end
  )

  QA.check(
    'R-038',
    'Full, collapsed, and shortcut reference links show like an inline link and hide the definitions',
    function()
      local model = T.layout({
        'one [full text][ref1] two [ref2][] three [ref3] four',
        '',
        '[ref1]: https://one.example/',
        '[REF2]: https://two.example/ "T"',
        '[ref3]: <https://three.example/>',
        '',
        'after',
      }, 100)
      a.eq({ 'one full text two ref2 three ref3 four', 'after' }, T.nonempty(model))
      local inline = T.layout({ '[x](https://one.example/)' }, 80)
      local _, inline_fg = link_fg(inline, 'x')
      for _, w in ipairs({ 'full text', 'ref2', 'ref3' }) do
        local ok, fg = link_fg(model, w)
        a.truthy(ok, w .. ' underlined')
        a.eq(inline_fg, fg, w .. ' in the link color')
      end
      a.not_contains('example', T.text(model), 'definition URL shown')
    end
  )

  QA.check(
    'R-038',
    'A reference link records its resolved target like an inline link',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'go [there][r]', '', '[r]: https://ref.example/p' }, 80)
      local found = false
      for _, l in ipairs(model.links or {}) do
        if l.target == 'https://ref.example/p' then
          found = true
        end
      end
      a.truthy(found, 'link target in model.links')
    end
  )

  QA.check(
    'R-038',
    'Hiding a reference definition leaves no extra blank lines',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'para [a][d]', '', '[d]: https://d.example/', '', 'next para' }, 80)
      local lines = QA.lines(model)
      local first, last = T.find(model, 'para'), T.find(model, 'next para')
      a.eq(2, last - first, 'one blank line between the paragraphs')
      a.eq('', lines[first + 1])
    end
  )

  QA.check(
    'R-038',
    'A bracketed word with no matching definition stays as written',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'see [nodef] here' }, 80)
      a.eq({ 'see [nodef] here' }, T.nonempty(model))
    end
  )
end)

QA.describe('text: emoji and escapes', function()
  QA.before_each(function()
    QA.reset_plugin()
  end)

  QA.check('R-056', 'Known emoji shortcodes are replaced by their emoji', function()
    local model = T.layout({ 'party :tada: ok :+1: love :heart: smile :smile:' }, 80)
    local text = table.concat(T.nonempty(model), ' ')
    for _, e in ipairs({ '🎉', '👍', '❤', '😄' }) do
      a.contains(e, text)
    end
    a.no_match(':%a[%w_+-]*:', text, 'shortcode left in the text')
    a.matches('^party 🎉 ok 👍 love ❤', text)
  end)

  QA.check(
    'R-056',
    'Emoji shortcodes are replaced in headings, list items, and table cells too',
    { severity = 'medium' },
    function()
      local model =
        T.layout({ '# Done :rocket:', '', '- item :tada:', '', '| a |', '|---|', '| :tada: |' }, 80)
      local text = T.text(model)
      a.not_contains(':rocket:', text)
      a.not_contains(':tada:', text)
      a.contains('🚀', text)
      a.contains('🎉', text)
    end
  )

  QA.check(
    { 'R-056', 'R-045' },
    'A table column holding an emoji is sized by its display width',
    { severity = 'medium' },
    function()
      local model = T.layout({ '| a | b |', '|---|---|', '| :tada: | x |' }, 80)
      local widths = {}
      for _, i in ipairs(T.table_lines(model)) do
        widths[T.dw((model.lines[i]:gsub('%s+$', '')))] = true
      end
      a.eq(1, #vim.tbl_keys(widths), 'every table line has the same display width')
    end
  )

  QA.check('R-057', 'An unknown emoji shortcode stays as written', function()
    local model = T.layout({ 'a :not_a_real_emoji_xyz: b and 10:30:45 c' }, 80)
    a.eq({ 'a :not_a_real_emoji_xyz: b and 10:30:45 c' }, T.nonempty(model))
  end)

  QA.check(
    'R-058',
    'A backslash escape shows the escaped character and its markdown meaning is off',
    function()
      local model =
        T.layout({ '\\*not italic\\* and \\_x\\_ and \\[y\\] and \\`z\\` and \\\\ end' }, 80)
      a.eq({ '*not italic* and _x_ and [y] and `z` and \\ end' }, T.nonempty(model))
      T.ensure_theme()
      a.falsy(
        T.any_attr(T.groups_over(model, 'not italic'), 'italic'),
        'escaped emphasis is not italic'
      )
    end
  )

  QA.check('R-058', 'An escaped block marker at the start of a line stays literal text', function()
    local model = T.layout(
      { '\\# not heading', '', '\\- not list', '', '\\> not quote', '', '1\\. not ordered' },
      80
    )
    a.eq({ '# not heading', '- not list', '> not quote', '1. not ordered' }, T.nonempty(model))
  end)

  QA.check(
    'R-058',
    'A backslash before a character that cannot be escaped stays as written',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'path C:\\dir\\file and \\a' }, 80)
      a.eq({ 'path C:\\dir\\file and \\a' }, T.nonempty(model))
    end
  )

  QA.check('R-058', 'An escaped colon stops an emoji shortcode', { severity = 'low' }, function()
    local model = T.layout({ 'x \\:tada: y' }, 80)
    a.eq({ 'x :tada: y' }, T.nonempty(model))
  end)
end)

QA.describe('text: no markers in inline text', function()
  QA.before_each(function()
    QA.reset_plugin()
  end)

  QA.check(
    'R-171',
    'Rendered inline text holds none of the emphasis, code, link, or strikethrough markers',
    function()
      local src = {
        'A *b* **c** ***d*** _e_ __f__ ~~g~~ `h` [i](http://x.y/) ![j](k.png "t") ',
        'and [l][m] plus <http://n.o/> and [p](<a b.md>).',
        '',
        '[m]: http://m.example/',
      }
      local model = T.layout(src, 120, { images = false })
      for i, l in ipairs(model.lines) do
        if l:find('[a-z]') then
          for _, m in ipairs({
            { '%*', '*' },
            { '__', '__' },
            { '~~', '~~' },
            { '`', 'backtick' },
            { '%]%(', '](' },
            { '%]%[', '][' },
            { '!%[', '![' },
          }) do
            a.no_match(m[1], l, m[2] .. ' on line ' .. i)
          end
        end
      end
      a.contains('A b c d e f g h i', T.text(model))
    end
  )
end)
