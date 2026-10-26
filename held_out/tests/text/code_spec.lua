local QA = require('qa')
local T = require('text_util')
local a = QA.assert

-- Open a real preview for a document and return the preview buffer.
local function open_preview(text)
  pcall(QA.capture_terminal)
  local buf = QA.buf(text)
  vim.api.nvim_set_current_buf(buf)
  if vim.fn.exists(':KittyMarkdown') == 2 then
    vim.cmd('KittyMarkdown open split')
  else
    require('kitty-markdown').open('split')
  end
  local pbuf
  QA.wait(2000, function()
    for _, w in ipairs(vim.api.nvim_list_wins()) do
      local b = vim.api.nvim_win_get_buf(w)
      if vim.bo[b].filetype == 'kitty-markdown' then
        pbuf = b
        return true
      end
    end
    return false
  end)
  QA.need(pbuf, 'a preview buffer with filetype kitty-markdown after opening a preview')
  return pbuf, buf
end

-- Highlight groups of the extmarks in the preview on 0-based rows [r0, r1].
local function marks_on_rows(pbuf, r0, r1)
  local out = {}
  local marks = vim.api.nvim_buf_get_extmarks(
    pbuf,
    -1,
    { r0, 0 },
    { r1, -1 },
    { details = true, overlap = true }
  )
  for _, m in ipairs(marks) do
    local g = m[4].hl_group
    if g then
      out[#out + 1] = g
    end
  end
  return out
end

local function row_of(pbuf, needle)
  for i, l in ipairs(vim.api.nvim_buf_get_lines(pbuf, 0, -1, false)) do
    if l:find(needle, 1, true) then
      return i - 1
    end
  end
end

QA.describe('text: fenced code blocks in the preview', function()
  QA.before_each(function()
    QA.reset_plugin()
  end)
  QA.after_each(function()
    QA.reset_plugin()
  end)

  QA.check(
    'R-047',
    'A fenced code block whose language has a tree-sitter parser gets tree-sitter capture highlights',
    function()
      QA.need(pcall(vim.treesitter.language.add, 'lua'), 'the bundled lua tree-sitter parser')
      local pbuf = open_preview({
        'text',
        '',
        '```lua',
        'local function greet(name)',
        '  return "hi " .. name',
        'end',
        '```',
      })
      local r0 = row_of(pbuf, 'local function')
      local r1 = row_of(pbuf, 'return')
      a.not_nil(r0, 'code line in the preview')
      local groups = marks_on_rows(pbuf, r0, r1)
      local ts = vim.tbl_filter(function(g)
        return g:sub(1, 1) == '@'
      end, groups)
      a.ge(2, #ts, 'tree-sitter capture groups on the code lines')
      local kw = false
      for _, g in ipairs(ts) do
        if g:match('^@keyword') then
          kw = true
        end
      end
      a.truthy(kw, 'a keyword capture')
    end
  )

  QA.check(
    'R-047',
    'Tree-sitter highlights land on the right code tokens',
    { severity = 'medium' },
    function()
      QA.need(pcall(vim.treesitter.language.add, 'lua'), 'the bundled lua tree-sitter parser')
      local pbuf = open_preview({ '```lua', 'local answer = "quoted"', '```' })
      local row = row_of(pbuf, 'quoted')
      local line = vim.api.nvim_buf_get_lines(pbuf, row, row + 1, false)[1]
      local col = line:find('"quoted"', 1, true) - 1
      local at = vim.api.nvim_buf_get_extmarks(
        pbuf,
        -1,
        { row, col + 2 },
        { row, col + 2 },
        { details = true, overlap = true }
      )
      local str = false
      for _, m in ipairs(at) do
        if (m[4].hl_group or ''):match('^@string') then
          str = true
        end
      end
      a.truthy(str, 'a string capture over the string literal')
      local kcol = line:find('local', 1, true) - 1
      local atk = vim.api.nvim_buf_get_extmarks(
        pbuf,
        -1,
        { row, kcol + 1 },
        { row, kcol + 1 },
        { details = true, overlap = true }
      )
      local kw = false
      for _, m in ipairs(atk) do
        if (m[4].hl_group or ''):match('^@keyword') then
          kw = true
        end
      end
      a.truthy(kw, 'a keyword capture over the keyword')
    end
  )

  QA.check(
    'R-047',
    'The language is read from the first word of the info string',
    { severity = 'medium' },
    function()
      QA.need(pcall(vim.treesitter.language.add, 'lua'), 'the bundled lua tree-sitter parser')
      local pbuf = open_preview({ '```lua {.numberLines title="x"}', 'local x = 1', '```' })
      local r = row_of(pbuf, 'local x')
      local ts = vim.tbl_filter(function(g)
        return g:sub(1, 1) == '@'
      end, marks_on_rows(pbuf, r, r))
      a.ge(1, #ts)
    end
  )

  QA.check(
    'R-047',
    'A tilde fence gets the same highlighting as a backtick fence',
    { severity = 'medium' },
    function()
      QA.need(pcall(vim.treesitter.language.add, 'lua'), 'the bundled lua tree-sitter parser')
      local pbuf = open_preview({ '~~~lua', 'local y = 2', '~~~' })
      local r = row_of(pbuf, 'local y')
      a.not_nil(r)
      local ts = vim.tbl_filter(function(g)
        return g:sub(1, 1) == '@'
      end, marks_on_rows(pbuf, r, r))
      a.ge(1, #ts)
    end
  )

  QA.check(
    'R-048',
    'A code block in a language with no parser uses only the plain code group',
    function()
      local pbuf = open_preview({
        '```zzznolang',
        'local function x() end',
        '```',
        '',
        '```',
        'no language here',
        '```',
      })
      for _, needle in ipairs({ 'local function', 'no language' }) do
        local r = row_of(pbuf, needle)
        a.not_nil(r, needle)
        local groups = marks_on_rows(pbuf, r, r)
        local plugin = T.plugin_groups(groups)
        a.ge(1, #plugin, 'a KittyMarkdown code group on ' .. needle)
        for _, g in ipairs(groups) do
          a.ne('@', g:sub(1, 1), 'tree-sitter capture ' .. g .. ' on ' .. needle)
        end
      end
    end
  )

  QA.check(
    'R-048',
    'Code without a parser and code with a parser share the same base code group',
    { severity = 'medium' },
    function()
      local m = T.layout({ '```zzznolang', 'aaa', '```', '', '```lua', 'bbb', '```' }, 60)
      local g1 = vim.fn.sort(T.plugin_groups((T.groups_over(m, 'aaa'))))
      local g2 = vim.fn.sort(T.plugin_groups((T.groups_over(m, 'bbb'))))
      a.ge(1, #g1)
      a.eq(g1, g2)
    end
  )
end)

QA.describe('text: fenced code blocks in the layout', function()
  QA.before_each(function()
    QA.reset_plugin()
    T.ensure_theme()
  end)

  QA.check('R-047', 'Markdown inside a code block stays literal', function()
    local model = T.layout(
      { '```text', '# not a heading', '**not bold** and `tick` [a](b)', '- not a list', '```' },
      80
    )
    local text = T.text(model)
    a.contains('# not a heading', text)
    a.contains('**not bold** and `tick` [a](b)', text)
    a.contains('- not a list', text)
  end)

  QA.check('R-047', 'Code block indentation is kept', function()
    local model = T.layout({ '```', 'def f():', '    return 1', '\tx', '```' }, 80)
    local l1 = model.lines[T.find(model, 'def f')]
    local l2 = model.lines[T.find(model, 'return 1')]
    a.eq(4, l2:find('return', 1, true) - l1:find('def', 1, true), 'relative indentation')
  end)

  QA.check('R-049', 'The language label is on the top border line of the code block', function()
    local model = T.layout({ 'para', '', '```python', 'print(1)', '```' }, 60)
    local code = T.find(model, 'print(1)')
    a.not_nil(code)
    local top = model.lines[code - 1]
    a.contains('python', top, 'label on the line above the code')
    a.truthy(top:find('[─━═┌╭]'), 'the label line is a border')
    a.ne(nil, T.plugin_groups((T.groups_over(model, 'python')))[1], 'the label has a group')
  end)

  QA.check(
    'R-049',
    'A code block without a language has a top border and no label text',
    { severity = 'medium' },
    function()
      local model = T.layout({ '```', 'body', '```' }, 60)
      local code = T.find(model, 'body')
      local top = model.lines[code - 1]
      a.truthy(top and top:find('[─━═┌╭]'), 'a top border')
      a.no_match('%w', top, 'no label text')
    end
  )

  QA.check(
    'R-049',
    'The language label border fits the text width',
    { severity = 'medium' },
    function()
      local w = 40
      local model = T.layout({ '```javascript', 'x', '```' }, w)
      local top = model.lines[T.find(model, 'javascript')]
      a.le(T.text_width(w) + T.MARGIN, T.dw((top:gsub('%s+$', ''))))
    end
  )

  QA.check('R-050', 'Fence markers are not shown for backtick and tilde fences', function()
    local model = T.layout({
      '```lua',
      'a = 1',
      '```',
      '',
      '~~~',
      'b = 2',
      '~~~',
      '',
      '````md',
      '```',
      'inner',
      '```',
      '````',
    }, 60)
    for _, l in ipairs(model.lines) do
      local t = vim.trim(l)
      if t ~= '```' then
        a.no_match('^%s*````', l)
        a.no_match('^%s*~~~', l)
        a.no_match('^%s*```%a', l)
      end
    end
    local backticks = 0
    for _, l in ipairs(model.lines) do
      if vim.trim(l) == '```' then
        backticks = backticks + 1
      end
    end
    a.eq(2, backticks, 'only the inner fence lines of the longer fence remain, as code')
  end)

  local function bg_spans(model, ln)
    local out = {}
    for _, h in ipairs(model.highlights) do
      local l, cs, ce, g = T.hl(h)
      if l == ln and T.style(g).bg then
        out[#out + 1] = { cs, ce, g }
      end
    end
    return out
  end

  for _, w in ipairs({ 40, 80, 130 }) do
    QA.check(
      'R-050',
      'Every code row, including blank rows and short rows, is filled with the code background across the text width',
      function()
        local model = T.layout({ '```', 'short', '', 'a much longer line of code', '```' }, w)
        local tw = T.text_width(w)
        local region = model.code_regions and model.code_regions[1]
        local first, last
        if region then
          first, last = region.line_start or region[1], region.line_end or region[2]
        else
          first, last = T.find(model, 'short'), T.find(model, 'a much longer')
        end
        a.eq(3, last - first + 1, 'three code rows')
        for ln = first, last do
          local line = model.lines[ln]
          local spans = bg_spans(model, ln)
          a.ge(1, #spans, 'a background span on code row ' .. ln)
          local covered = false
          for _, s in ipairs(spans) do
            local left = T.dw(line:sub(1, s[1]))
            local right = s[2] > #line and math.huge or T.dw(line:sub(1, s[2]))
            if left <= T.MARGIN and right >= tw + T.MARGIN then
              covered = true
            end
          end
          a.truthy(
            covered,
            ('code row %d background covers the text width (line %q)'):format(ln, line)
          )
        end
      end
    )
  end

  QA.check(
    'R-050',
    'The code background group sets a background color different from the editor background',
    function()
      local model = T.layout({ '```', 'x', '```' }, 60)
      local ln = T.find(model, 'x')
      local spans = bg_spans(model, ln)
      a.ge(1, #spans)
      local normal = vim.api.nvim_get_hl(0, { name = 'Normal', link = false }).bg
      a.ne(normal, T.style(spans[1][3]).bg)
    end
  )

  QA.check(
    'R-050',
    'A code block inside a list item keeps its code text and hides its fences',
    { severity = 'medium' },
    function()
      local model = T.layout({ '- item', '', '  ```lua', '  local z = 3', '  ```' }, 60)
      local text = T.text(model)
      a.contains('local z = 3', text)
      a.not_contains('```', text)
    end
  )
end)
