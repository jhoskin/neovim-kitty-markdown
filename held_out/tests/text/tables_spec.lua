local QA = require('qa')
local T = require('text_util')
local a = QA.assert

-- Table rows (lines with vertical borders) and border lines of a model.
local function table_parts(model)
  local rows, borders = {}, {}
  for _, i in ipairs(T.table_lines(model)) do
    local t = vim.trim(model.lines[i])
    if t:find('^│') then
      rows[#rows + 1] = i
    else
      borders[#borders + 1] = i
    end
  end
  return rows, borders
end

local BASIC = {
  '| Name | Count | Note |',
  '|------|-------|------|',
  '| a | 1 | x |',
  '| bb | 22 | yy |',
  '| ccc | 333 | zzz |',
}

QA.describe('text: tables', function()
  QA.before_each(function()
    QA.reset_plugin()
    T.ensure_theme()
  end)

  QA.check('R-043', 'A table has box drawing corners, junctions, and vertical borders', function()
    local model = T.layout(BASIC, 80)
    local text = T.text(model)
    for _, c in ipairs({
      '┌',
      '┐',
      '└',
      '┘',
      '┬',
      '┴',
      '├',
      '┤',
      '┼',
      '│',
      '─',
    }) do
      a.contains(c, text, 'box drawing ' .. c)
    end
    a.no_match('|', text, 'ASCII pipe shown')
    a.no_match('%-%-%-', text, 'delimiter row shown')
  end)

  QA.check('R-043', 'Every table row has a vertical border on both sides of every cell', function()
    local model = T.layout(BASIC, 80)
    local rows = table_parts(model)
    a.eq(4, #rows, 'header row and three body rows')
    for _, i in ipairs(rows) do
      local _, n = model.lines[i]:gsub('│', '')
      a.eq(4, n, 'vertical borders on row ' .. i)
    end
  end)

  QA.check(
    'R-043',
    'Every table cell has a horizontal border above and below it, between body rows too',
    { severity = 'medium' },
    function()
      local model = T.layout(BASIC, 80)
      local rows, borders = table_parts(model)
      a.eq(#rows + 1, #borders, 'one horizontal border above, below, and between each row')
      for k = 1, #rows - 1 do
        local between = false
        for _, b in ipairs(borders) do
          if b > rows[k] and b < rows[k + 1] then
            between = true
          end
        end
        a.truthy(between, 'a horizontal border between row ' .. k .. ' and row ' .. (k + 1))
      end
    end
  )

  QA.check(
    'R-043',
    'All lines of a table have the same display width and the borders line up',
    function()
      local model =
        T.layout({ '| 名前 | b |', '|---|---|', '| x | 😀 |', '| longer text | y |' }, 80)
      local idx = T.table_lines(model)
      a.ge(5, #idx)
      local w = T.dw((model.lines[idx[1]]:gsub('%s+$', '')))
      for _, i in ipairs(idx) do
        a.eq(w, T.dw((model.lines[i]:gsub('%s+$', ''))), 'width of table line ' .. i)
      end
      -- the inner junctions sit in the same display column as the row borders
      local function cols(line, chars)
        local out, c = {}, 0
        for _, ch in ipairs(vim.fn.split(line, '\\zs')) do
          if chars[ch] then
            out[#out + 1] = c
          end
          c = c + T.dw(ch)
        end
        return out
      end
      local ref = cols(model.lines[idx[1]], { ['┌'] = true, ['┬'] = true, ['┐'] = true })
      for _, i in ipairs(idx) do
        local got = cols(model.lines[i], {
          ['│'] = true,
          ['┌'] = true,
          ['┬'] = true,
          ['┐'] = true,
          ['├'] = true,
          ['┼'] = true,
          ['┤'] = true,
          ['└'] = true,
          ['┴'] = true,
          ['┘'] = true,
        })
        a.eq(ref, got, 'border columns on line ' .. i)
      end
    end
  )

  QA.check(
    'R-043',
    'A table with a single column and a header only still has full borders',
    { severity = 'medium' },
    function()
      local model = T.layout({ '| only |', '|---|' }, 80)
      local text = T.text(model)
      for _, c in ipairs({ '┌', '┐', '└', '┘', '│' }) do
        a.contains(c, text)
      end
      a.contains('only', text)
    end
  )

  QA.check(
    'R-043',
    'A row with fewer cells than the header is padded with empty bordered cells',
    { severity = 'medium' },
    function()
      local model = T.layout({ '| a | b | c |', '|---|---|---|', '| 1 |' }, 80)
      local rows = table_parts(model)
      for _, i in ipairs(rows) do
        local _, n = model.lines[i]:gsub('│', '')
        a.eq(4, n, 'borders on row ' .. i)
      end
    end
  )

  QA.check(
    'R-043',
    'An escaped pipe inside a cell is cell text, not a border',
    { severity = 'medium' },
    function()
      local model = T.layout({ '| a | b |', '|---|---|', '| x \\| y | z |' }, 80)
      a.contains('x | y', T.text(model))
    end
  )

  -- Cell text and its padding inside the borders, per row.
  local function cell(model, needle)
    local ln = T.find(model, needle)
    a.not_nil(ln, needle)
    for _, c in ipairs(T.cells(model.lines[ln])) do
      if c:find(needle, 1, true) then
        local s = c:find(needle, 1, true)
        local left = T.dw(c:sub(1, s - 1))
        local right = T.dw(c:sub(s + #needle))
        return left, right, c
      end
    end
  end

  local ALIGN = {
    '| Left | Center | Right | Plain |',
    '|:-----|:------:|------:|-------|',
    '| q1 | q2 | q3 | q4 |',
    '| longer left | longer center | longer right | longer plain |',
  }

  QA.check('R-044', 'A left-aligned column puts short cell text against the left border', function()
    local model = T.layout(ALIGN, 120)
    local left, right = cell(model, 'q1')
    local l2, r2 = cell(model, 'longer left')
    a.eq(l2, left, 'same left padding as the widest cell')
    a.truthy(right > r2, 'extra space goes to the right')
  end)

  QA.check(
    'R-044',
    'A right-aligned column puts short cell text against the right border',
    function()
      local model = T.layout(ALIGN, 120)
      local left, right = cell(model, 'q3')
      local l2, r2 = cell(model, 'longer right')
      a.eq(r2, right, 'same right padding as the widest cell')
      a.truthy(left > l2, 'extra space goes to the left')
    end
  )

  QA.check(
    'R-044',
    'A center-aligned column splits the extra space evenly, within one cell',
    function()
      local model = T.layout(ALIGN, 120)
      local left, right = cell(model, 'q2')
      a.le(1, math.abs(left - right), 'left and right padding differ by at most one')
      a.ge(3, math.min(left, right), 'short text is centered, not at an edge')
    end
  )

  QA.check('R-044', 'A column without alignment colons is left-aligned', function()
    local model = T.layout(ALIGN, 120)
    local left, right = cell(model, 'q4')
    local l2 = cell(model, 'longer plain')
    a.eq(l2, left)
    a.truthy(right > left)
  end)

  QA.check(
    'R-044',
    'Alignment applies to the header row as well as the body rows',
    { severity = 'medium' },
    function()
      local model = T.layout({ '| R |', '|--:|', '| wide body |' }, 80)
      local left, right = cell(model, 'R')
      local _, r2 = cell(model, 'wide body')
      a.eq(r2, right)
      a.truthy(left > right)
    end
  )

  QA.check('R-045', 'Each column is as wide as its widest cell plus a constant padding', function()
    local model = T.layout(BASIC, 80)
    local rows = table_parts(model)
    local widest = { 0, 0, 0 }
    local inner = {}
    for _, i in ipairs(rows) do
      local cells = T.cells(model.lines[i])
      for k, c in ipairs(cells) do
        widest[k] = math.max(widest[k], T.dw(vim.trim(c)))
        inner[k] = T.dw(c)
      end
    end
    local pad = inner[1] - widest[1]
    a.ge(0, pad)
    a.le(4, pad, 'padding of at most 2 cells each side')
    for k = 2, 3 do
      a.eq(pad, inner[k] - widest[k], 'padding of column ' .. k)
    end
    a.eq(5 + pad, inner[2], 'the Count column sized by its header')
  end)

  QA.check(
    'R-045',
    'Column widths count wide characters by display width, not bytes or code points',
    function()
      local model =
        T.layout({ '| 東京都 | x |', '|---|---|', '| ab | 😀😀 |', '| abcdefg | y |' }, 80)
      local rows = table_parts(model)
      local inner = {}
      for _, i in ipairs(rows) do
        for k, c in ipairs(T.cells(model.lines[i])) do
          inner[k] = inner[k] or T.dw(c)
          a.eq(inner[k], T.dw(c), 'cell display width in column ' .. k .. ' on line ' .. i)
        end
      end
      local plain = T.layout({ '| abcdefg | xxxx |', '|---|---|', '| a | b |' }, 80)
      local pc = T.cells(plain.lines[table_parts(plain)[1]])
      a.eq(T.dw(pc[1]), inner[1], 'a 7-cell column whatever the characters')
      a.eq(T.dw(pc[2]), inner[2], 'the emoji column is as wide as 4 narrow cells')
    end
  )

  QA.check('R-045', 'Column widths use the rendered cell text, without markdown markers', function()
    local model = T.layout({
      '| **ab** | `cd` | [ef](http://long.example/url) |',
      '|---|---|---|',
      '| abc | cde | efg |',
    }, 80)
    local rows = table_parts(model)
    for _, i in ipairs(rows) do
      local cells = T.cells(model.lines[i])
      a.eq(3, #cells)
      for k, c in ipairs(cells) do
        a.eq(T.dw(T.cells(model.lines[rows[2]])[k]), T.dw(c))
      end
    end
    local plain = T.layout({ '| abc | cde | efg |', '|---|---|---|', '| ab | cd | ef |' }, 80)
    local want = T.cells(plain.lines[table_parts(plain)[1]])
    local got = T.cells(model.lines[rows[1]])
    for k = 1, 3 do
      a.eq(T.dw(want[k]), T.dw(got[k]), 'column ' .. k .. ' sized by its widest rendered cell')
    end
    a.not_contains('**', T.text(model))
    a.not_contains('long.example', T.text(model))
  end)

  QA.check('R-046', 'Header cells are bold and body cells are not', function()
    local model = T.layout(BASIC, 80)
    for _, h in ipairs({ 'Name', 'Count', 'Note' }) do
      a.truthy(T.any_attr((T.groups_over(model, h)), 'bold'), 'header ' .. h .. ' bold')
    end
    for _, b in ipairs({ 'ccc', '333', 'zzz' }) do
      a.falsy(T.any_attr((T.groups_over(model, b)), 'bold'), 'body cell ' .. b .. ' bold')
    end
  end)

  QA.check(
    'R-046',
    'Inline styles in a header cell keep the header bold',
    { severity = 'medium' },
    function()
      local model = T.layout({ '| *it* head |', '|---|', '| x |' }, 80)
      a.truthy(T.any_attr((T.groups_over(model, 'it')), 'bold'), 'styled header text bold')
      a.truthy(T.any_attr((T.groups_over(model, 'it')), 'italic'), 'styled header text italic')
    end
  )
end)
