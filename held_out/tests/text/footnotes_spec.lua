local QA = require('qa')
local T = require('text_util')
local a = QA.assert

local SUP = {
  ['⁰'] = 0,
  ['¹'] = 1,
  ['²'] = 2,
  ['³'] = 3,
  ['⁴'] = 4,
  ['⁵'] = 5,
  ['⁶'] = 6,
  ['⁷'] = 7,
  ['⁸'] = 8,
  ['⁹'] = 9,
}
local SUPS = { '⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹' }

local function sup(n)
  local out = {}
  for d in tostring(n):gmatch('%d') do
    out[#out + 1] = SUPS[tonumber(d) + 1]
  end
  return table.concat(out)
end

-- The line index where the definition text `needle` is shown, and the
-- number shown at its start (superscript or plain digits), or nil.
local function def_number(model, needle)
  local ln = T.find(model, needle)
  if not ln then
    return nil
  end
  local l = vim.trim(model.lines[ln])
  local head = l:sub(1, l:find(needle, 1, true) - 1)
  local digits = head:match('(%d+)')
  if digits then
    return ln, tonumber(digits)
  end
  local n = ''
  for _, ch in ipairs(vim.fn.split(head, '\\zs')) do
    if SUP[ch] then
      n = n .. SUP[ch]
    end
  end
  return ln, tonumber(n)
end

QA.describe('text: footnotes', function()
  QA.before_each(function()
    QA.reset_plugin()
  end)

  QA.check(
    'R-053',
    'A footnote reference is shown as a superscript number in place of its label',
    function()
      local model = T.layout({ 'Claim here[^src] and more.', '', '[^src]: The source.' }, 80)
      local line = vim.trim(model.lines[T.find(model, 'Claim')])
      a.eq('Claim here¹ and more.', line)
    end
  )

  QA.check(
    'R-053',
    'Footnote numbers follow the order of first reference, not the label text',
    function()
      local model = T.layout({
        'A[^zeta] B[^alpha] C[^9] D[^zeta]',
        '',
        '[^alpha]: alpha def',
        '[^9]: nine def',
        '[^zeta]: zeta def',
      }, 80)
      local line = vim.trim(model.lines[T.find(model, 'A')])
      a.eq('A¹ B² C³ D¹', line)
    end
  )

  QA.check(
    'R-053',
    'Footnote numbers of any number of digits use a superscript digit for every digit',
    { severity = 'medium' },
    function()
      local src, defs = {}, {}
      for i = 1, 11 do
        src[#src + 1] = 'w' .. i .. '[^n' .. i .. ']'
        defs[#defs + 1] = '[^n' .. i .. ']: def ' .. i
      end
      local model = T.layout(vim.list_extend({ table.concat(src, ' '), '' }, defs), 200)
      local text = T.text(model)
      a.contains('w10' .. sup(10), text)
      a.contains('w11' .. sup(11), text)
    end
  )

  QA.check(
    'R-053',
    'A footnote reference in a list item or heading is a superscript number too',
    { severity = 'medium' },
    function()
      local model = T.layout({ '# Title[^a]', '', '- item[^b]', '', '[^a]: da', '[^b]: db' }, 80)
      local text = T.text(model)
      a.contains('Title¹', text)
      a.contains('item²', text)
      a.not_contains('[^', text)
    end
  )

  QA.check(
    'R-053',
    'The footnote reference has its own highlight group',
    { severity = 'low' },
    function()
      local model = T.layout({ 'word[^x] tail', '', '[^x]: d' }, 80)
      local g = T.plugin_groups((T.groups_over(model, '¹')))
      a.ge(1, #g)
    end
  )

  QA.check(
    'R-054',
    'Footnote definitions are shown at the end of the document even when written earlier',
    function()
      local model = T.layout({
        'Intro[^a].',
        '',
        '[^a]: Early definition text.',
        '',
        '# Later heading',
        '',
        'Final paragraph.',
      }, 80)
      local def = T.find(model, 'Early definition text')
      local last = T.find(model, 'Final paragraph')
      a.not_nil(def, 'definition shown')
      a.truthy(def > last, 'definition after the last block')
    end
  )

  QA.check('R-054', 'Footnote definitions are numbered and ordered by first reference', function()
    local model = T.layout({
      'One[^b] two[^c] three[^a].',
      '',
      '[^a]: Text for a.',
      '[^c]: Text for c.',
      '[^b]: Text for b.',
    }, 80)
    local lb, nb = def_number(model, 'Text for b')
    local lc, nc = def_number(model, 'Text for c')
    local la, na = def_number(model, 'Text for a')
    a.eq(1, nb, 'number of the first referenced definition')
    a.eq(2, nc)
    a.eq(3, na)
    a.truthy(lb < lc and lc < la, 'definitions in reference order')
  end)

  QA.check(
    'R-054',
    'Footnote definition text keeps its inline styles and loses its label marker',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'x[^n]', '', '[^n]: A **bold** note with `code`.' }, 80)
      local ln = T.find(model, 'bold')
      a.not_nil(ln)
      local l = model.lines[ln]
      a.not_contains('[^n]', l)
      a.not_contains('**', l)
      a.not_contains('`', l)
    end
  )

  QA.check(
    'R-054',
    'A multi-paragraph footnote definition shows all of its paragraphs',
    { severity = 'medium' },
    function()
      local model = T.layout(
        { 'x[^long]', '', '[^long]: First para.', '', '    Second para of the note.', '', 'After.' },
        80
      )
      local f = T.find(model, 'First para')
      local s = T.find(model, 'Second para of the note')
      local after = T.find(model, 'After.')
      a.not_nil(f)
      a.not_nil(s)
      a.truthy(f > after and s > f, 'both paragraphs at the end')
    end
  )

  QA.check('R-218', 'A footnote definition with no reference is not shown', function()
    local model = T.layout(
      { 'Body[^used] text.', '', '[^used]: Used note.', '[^orphan]: Orphan note text.' },
      80
    )
    local text = T.text(model)
    a.contains('Used note', text)
    a.not_contains('Orphan note text', text)
    a.not_contains('orphan', text)
  end)

  QA.check(
    'R-218',
    'A document with only unreferenced footnote definitions shows none of them',
    function()
      local model = T.layout({ 'Body text.', '', '[^x]: Lonely one.', '[^y]: Lonely two.' }, 80)
      local text = T.text(model)
      a.not_contains('Lonely', text)
      a.eq({ 'Body text.' }, T.nonempty(model))
    end
  )

  QA.check(
    'R-218',
    'An unreferenced definition does not use up a footnote number',
    { severity = 'medium' },
    function()
      local model = T.layout({ 'A[^two].', '', '[^one]: Unused.', '[^two]: Used.' }, 80)
      a.eq('A¹.', vim.trim(model.lines[T.find(model, 'A')]))
    end
  )

  QA.check('R-219', 'A footnote reference with no definition is shown as written', function()
    local model = T.layout({ 'Dangling[^missing] ref.' }, 80)
    a.eq({ 'Dangling[^missing] ref.' }, T.nonempty(model))
  end)

  QA.check(
    'R-219',
    'An undefined footnote reference does not take a number from the defined ones',
    function()
      local model = T.layout({ 'A[^nodef] B[^def].', '', '[^def]: Defined.' }, 80)
      local line = vim.trim(model.lines[T.find(model, 'A')])
      a.eq('A[^nodef] B¹.', line)
      local _, n = def_number(model, 'Defined.')
      a.eq(1, n)
    end
  )
end)
