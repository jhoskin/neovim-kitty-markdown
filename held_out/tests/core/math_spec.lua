local QA = require('qa')
local U = require('core_util')
local a = QA.assert
local u = U.u

local function render(tex)
  local inner = U.inline(tex)
  a.not_nil(inner, 'math span not found between the marker words')
  return inner
end

QA.describe('core: math spans', function()
  QA.before_each(function()
    QA.reset_plugin()
  end)

  -- R-062 ------------------------------------------------------------------

  QA.check(
    'R-062',
    'Single dollar delimiters make inline math: the markers disappear and the content gets the math group',
    function()
      local inner, model = U.inline('\\alpha')
      a.eq(u(0x03B1), inner)
      a.no_match('%$', U.text(model))
      a.truthy(U.covered_by(model, u(0x03B1), U.is_math_group), 'math group over the span')
    end
  )

  QA.check('R-062', 'Inline math stays inside the paragraph line, between its neighbors', function()
    local model = QA.layout({ 'before $x^2$ after' }, { width = 80 })
    local lines = QA.lines(model)
    a.eq(1, #vim.tbl_filter(function(l)
      return l ~= ''
    end, lines), 'one text line')
    a.matches('before x' .. u(0x00B2) .. ' after', lines[1])
  end)

  QA.check(
    'R-062',
    'Double dollar delimiters make display math with the markers removed',
    function()
      local model = QA.layout(
        { 'intro', '', '$$', '\\alpha + \\beta', '$$', '', 'outro' },
        { width = 60 }
      )
      local text = U.text(model)
      a.no_match('%$', text)
      a.contains(u(0x03B1) .. ' + ' .. u(0x03B2), text)
      a.truthy(
        U.covered_by(model, u(0x03B1) .. ' + ' .. u(0x03B2), U.is_math_group),
        'math group over display math'
      )
    end
  )

  QA.check(
    'R-062',
    'Display math written on one line with double dollars is display math',
    function()
      local model = QA.layout({ 'intro', '', '$$\\gamma$$', '', 'outro' }, { width = 60 })
      local lines = QA.lines(model)
      local found = false
      for _, l in ipairs(lines) do
        if vim.trim(l) == u(0x03B3) then
          found = true
        end
      end
      a.truthy(found, 'display math alone on a line: ' .. vim.inspect(lines))
    end
  )

  QA.check('R-062', 'Two inline math spans on one line render separately', function()
    local inner = render('\\alpha$ and $\\beta')
    a.eq(u(0x03B1) .. ' and ' .. u(0x03B2), inner)
  end)

  QA.check(
    { 'R-062', 'R-058', 'R-353' },
    'An escaped dollar sign is literal text and does not open a math span',
    { severity = 'medium' },
    function()
      local model = QA.layout({ 'costs \\$5 or \\$6 today' }, { width = 80 })
      a.contains('costs $5 or $6 today', U.text(model))
      a.falsy(U.covered_by(model, '5 or', U.is_math_group), 'math group over escaped dollars')
    end
  )

  QA.check(
    'R-062',
    'Dollar signs inside a code span stay literal code, not math',
    { severity = 'medium' },
    function()
      local model = QA.layout({ 'run `echo $x$` now' }, { width = 80 })
      a.contains('echo $x$', U.text(model))
    end
  )

  QA.check(
    'R-062',
    'Dollar signs inside a fenced code block stay literal',
    { severity = 'medium' },
    function()
      local model = QA.layout(
        { '```', 'a $\\alpha$ b', '$$', '\\beta', '$$', '```' },
        { width = 80 }
      )
      local text = U.text(model)
      a.contains('a $\\alpha$ b', text)
      a.contains('\\beta', text)
    end
  )

  -- R-063 ------------------------------------------------------------------

  QA.check(
    'R-063',
    'A span of only Greek letters, scripts, common operators, and a simple fraction becomes Unicode text with no TeX syntax left',
    function()
      local inner = render('\\alpha^2 + \\beta_1 \\leq \\frac{1}{2} \\cdot x')
      a.no_match('[\\%^_{}%$]', inner, 'TeX syntax left in ' .. inner)
      a.contains(u(0x03B1, 0x00B2), inner)
      a.contains(u(0x03B2, 0x2081), inner)
      a.contains(u(0x2264), inner)
      a.contains('1/2', inner)
    end
  )

  QA.check(
    'R-063',
    'Plain letters, digits, and ASCII operators in math pass through as text',
    function()
      a.eq('a + b = c', render('a + b = c'))
      a.eq('x - 1', render('x - 1'))
    end
  )

  -- R-064 ------------------------------------------------------------------

  QA.check('R-064', 'Every lowercase Greek letter command maps to its Unicode letter', function()
    local map = {
      alpha = 0x03B1,
      beta = 0x03B2,
      gamma = 0x03B3,
      delta = 0x03B4,
      zeta = 0x03B6,
      eta = 0x03B7,
      theta = 0x03B8,
      iota = 0x03B9,
      kappa = 0x03BA,
      lambda = 0x03BB,
      mu = 0x03BC,
      nu = 0x03BD,
      xi = 0x03BE,
      pi = 0x03C0,
      rho = 0x03C1,
      sigma = 0x03C3,
      tau = 0x03C4,
      upsilon = 0x03C5,
      chi = 0x03C7,
      psi = 0x03C8,
      omega = 0x03C9,
    }
    local bad = {}
    for name, cp in pairs(map) do
      local got = render('\\' .. name)
      if got ~= u(cp) then
        bad[#bad + 1] = name .. '=' .. tostring(got)
      end
    end
    table.sort(bad)
    a.eq({}, bad)
  end)

  QA.check('R-064', 'Every uppercase Greek letter command maps to its Unicode capital', function()
    local map = {
      Gamma = 0x0393,
      Delta = 0x0394,
      Theta = 0x0398,
      Lambda = 0x039B,
      Xi = 0x039E,
      Pi = 0x03A0,
      Sigma = 0x03A3,
      Upsilon = 0x03A5,
      Phi = 0x03A6,
      Psi = 0x03A8,
      Omega = 0x03A9,
    }
    local bad = {}
    for name, cp in pairs(map) do
      local got = render('\\' .. name)
      if got ~= u(cp) then
        bad[#bad + 1] = name .. '=' .. tostring(got)
      end
    end
    table.sort(bad)
    a.eq({}, bad)
  end)

  QA.check(
    'R-064',
    'The variant Greek letter commands map to their Unicode variant letters',
    { severity = 'medium' },
    function()
      local map = { vartheta = 0x03D1, varpi = 0x03D6, varrho = 0x03F1, varsigma = 0x03C2 }
      local bad = {}
      for name, cp in pairs(map) do
        local got = render('\\' .. name)
        if got ~= u(cp) then
          bad[#bad + 1] = name .. '=' .. tostring(got)
        end
      end
      table.sort(bad)
      a.eq({}, bad)
    end
  )

  QA.check(
    'R-064',
    'Greek letters next to each other and next to Latin letters map in place',
    function()
      a.eq(u(0x03B1, 0x03B2), render('\\alpha\\beta'))
      a.eq('2' .. u(0x03C0) .. 'r', (render('2\\pi r'):gsub(' ', '')))
    end
  )

  QA.check(
    'R-064',
    'A command that only starts with a Greek letter name is not mapped as that letter',
    { severity = 'medium' },
    function()
      local inner = render('\\alphabet')
      a.ne(u(0x03B1) .. 'bet', inner)
    end
  )

  -- R-065 ------------------------------------------------------------------

  QA.check('R-065', 'A single digit superscript maps to the superscript digit', function()
    local sup = { 0x2070, 0x00B9, 0x00B2, 0x00B3, 0x2074, 0x2075, 0x2076, 0x2077, 0x2078, 0x2079 }
    for d = 0, 9 do
      a.eq('x' .. u(sup[d + 1]), render('x^' .. d), 'digit ' .. d)
    end
  end)

  QA.check(
    'R-065',
    'A braced superscript of several digits and signs maps every character',
    function()
      a.eq('x' .. u(0x00B9, 0x2070), render('x^{10}'))
      a.eq('x' .. u(0x207B, 0x00B9), render('x^{-1}'))
      a.eq('x' .. u(0x207A), render('x^{+}'))
    end
  )

  QA.check(
    'R-065',
    'A superscript of Latin letters that have superscript forms maps to those forms',
    function()
      a.eq('x' .. u(0x207F), render('x^n'))
      a.eq('e' .. u(0x2071), render('e^i'))
      a.eq('x' .. u(0x207F, 0x207B, 0x00B9), render('x^{n-1}'))
    end
  )

  QA.check('R-065', 'A superscript on a Greek letter maps the same way', function()
    a.eq(u(0x03C3, 0x00B2), render('\\sigma^2'))
  end)

  -- R-066 ------------------------------------------------------------------

  QA.check('R-066', 'A single digit subscript maps to the subscript digit', function()
    for d = 0, 9 do
      a.eq('a' .. u(0x2080 + d), render('a_' .. d), 'digit ' .. d)
    end
  end)

  QA.check('R-066', 'A braced subscript of digits and signs maps every character', function()
    a.eq('a' .. u(0x2081, 0x2082), render('a_{12}'))
    a.eq('a' .. u(0x208B, 0x2081), render('a_{-1}'))
    a.eq('a' .. u(0x208A), render('a_{+}'))
  end)

  QA.check(
    'R-066',
    'A subscript of Latin letters that have subscript forms maps to those forms',
    function()
      a.eq('x' .. u(0x1D62), render('x_i'))
      a.eq('x' .. u(0x2099), render('x_n'))
      a.eq('x' .. u(0x1D62, 0x2C7C), render('x_{ij}'))
      a.eq('x' .. u(0x2099, 0x208B, 0x2081), render('x_{n-1}'))
    end
  )

  QA.check(
    { 'R-065', 'R-066' },
    'A token with both a subscript and a superscript maps both',
    function()
      local inner = render('x_1^2')
      a.no_match('[%^_{}]', inner)
      a.contains(u(0x2081), inner)
      a.contains(u(0x00B2), inner)
    end
  )

  -- R-067 ------------------------------------------------------------------

  QA.check('R-067', 'Each listed operator command maps to its Unicode symbol', function()
    local map = {
      times = { 0x00D7 },
      pm = { 0x00B1 },
      leq = { 0x2264 },
      geq = { 0x2265 },
      neq = { 0x2260 },
      approx = { 0x2248 },
      infty = { 0x221E },
      sum = { 0x2211 },
      int = { 0x222B },
      to = { 0x2192 },
      cdot = { 0x22C5, 0x00B7 },
    }
    local bad = {}
    for name, cps in pairs(map) do
      local got = render('a \\' .. name .. ' b')
      local ok = false
      for _, cp in ipairs(cps) do
        if got == 'a ' .. u(cp) .. ' b' then
          ok = true
        end
      end
      if not ok then
        bad[#bad + 1] = name .. '=' .. tostring(got)
      end
    end
    table.sort(bad)
    a.eq({}, bad)
  end)

  QA.check('R-067', 'The square root command maps to the radical sign', function()
    a.contains(u(0x221A), render('\\sqrt{2}'))
  end)

  QA.check('R-067', 'Operators without operands still map to their symbols', function()
    a.eq(u(0x221E), render('\\infty'))
    a.eq(u(0x2211), render('\\sum'))
  end)

  QA.check(
    'R-067',
    'An operator directly followed by a letter maps without eating the letter',
    function()
      a.eq('n ' .. u(0x2192) .. ' ' .. u(0x221E), render('n \\to \\infty'))
      a.eq('2' .. u(0x00D7) .. '3', (render('2\\times3'):gsub(' ', '')))
    end
  )

  -- R-068 ------------------------------------------------------------------

  QA.check('R-068', 'A fraction of single tokens shows as numerator slash denominator', function()
    a.eq('a/b', render('\\frac{a}{b}'))
    a.eq('1/2', render('\\frac{1}{2}'))
  end)

  QA.check(
    'R-068',
    'A number of several digits is one token and gets no parentheses in a fraction',
    function()
      a.eq('12/7', render('\\frac{12}{7}'))
    end
  )

  QA.check('R-068', 'A token with its superscript counts as one token in a fraction', function()
    a.eq('x' .. u(0x00B2) .. '/y', render('\\frac{x^2}{y}'))
    a.eq(u(0x03C0) .. '/4', render('\\frac{\\pi}{4}'))
  end)

  QA.check('R-068', 'A numerator of several tokens gets parentheses', function()
    a.eq('(a + b)/c', render('\\frac{a + b}{c}'))
    a.eq('(a+b)/c', (render('\\frac{a+b}{c}'):gsub(' ', '')))
  end)

  QA.check('R-068', 'A denominator of several tokens gets parentheses', function()
    a.eq('1/(n + 1)', render('\\frac{1}{n + 1}'))
  end)

  QA.check(
    'R-068',
    'A fraction with several tokens on both sides gets parentheses on both sides',
    function()
      a.eq('(a - b)/(c + d)', render('\\frac{a - b}{c + d}'))
    end
  )

  QA.check('R-068', 'A fraction inside a longer span keeps the text around it', function()
    a.eq('y = 1/2', render('y = \\frac{1}{2}'))
  end)

  -- R-069 / R-220 ------------------------------------------------------------

  QA.check(
    { 'R-069', 'R-220' },
    'A span with TeX that cannot be approximated shows its source without dollar markers in the math group',
    function()
      local tex = '\\begin{pmatrix} a & b \\end{pmatrix}'
      local inner, model = U.inline(tex)
      a.eq(tex, inner)
      a.truthy(U.covered_by(model, tex, U.is_math_group), 'math group over the source')
    end
  )

  QA.check('R-069', 'An unknown command makes the span fall back to the source', function()
    local tex = '\\qaunknowncmd{x}'
    local inner, model = U.inline(tex)
    a.eq(tex, inner)
    a.truthy(U.covered_by(model, tex, U.is_math_group), 'math group over the source')
  end)

  QA.check(
    'R-069',
    'A span that mixes known and unapproximable TeX shows the whole source, not a partial conversion',
    function()
      local tex = '\\alpha + \\overbrace{x}'
      local inner = render(tex)
      a.eq(tex, inner)
    end
  )

  QA.check(
    { 'R-069', 'R-220' },
    'Display math that cannot be approximated shows its source without double dollar markers in the math group',
    function()
      local model = QA.layout(
        { 'intro', '', '$$', '\\begin{cases} a \\end{cases}', '$$', '', 'outro' },
        { width = 80 }
      )
      local text = U.text(model)
      a.no_match('%$', text)
      a.contains('\\begin{cases} a \\end{cases}', text)
      a.truthy(U.covered_by(model, '\\begin{cases}', U.is_math_group), 'math group over the source')
    end
  )

  QA.check(
    'R-220',
    'Source fallback shows no dollar markers even when the span is the only content of the paragraph',
    function()
      local model = QA.layout({ '$\\qaunknown$' }, { width = 80 })
      local text = U.text(model)
      a.eq('\\qaunknown', text)
    end
  )

  QA.check(
    'R-069',
    'Approximated math is also in the math group',
    { severity = 'medium' },
    function()
      local _, model = U.inline('x^2')
      a.truthy(U.covered_by(model, 'x' .. u(0x00B2), U.is_math_group))
    end
  )

  -- R-070 ------------------------------------------------------------------

  local function display_line(model, needle)
    for i, l in ipairs(model.lines) do
      if l:find(needle, 1, true) then
        return l, i
      end
    end
  end

  local function check_centered(width, needle, lines)
    local model = QA.layout(lines, { width = width })
    local l = display_line(model, needle)
    a.not_nil(l, 'display line')
    a.eq(needle, vim.trim(l), 'display math alone on its line')
    local margin = U.margin(width)
    local lead = U.lead(l) - margin
    local w = U.width(needle)
    local ok = false
    local tried = {}
    -- opts.width is the window width (R-332); the text width is width - 4,
    -- capped at 100
    for _, tw in ipairs({ math.min(width - 4, 100) }) do
      local free = tw - w
      tried[#tried + 1] = ('tw=%d lead=%d free=%d'):format(tw, lead, free)
      if lead == math.floor(free / 2) or lead == math.ceil(free / 2) then
        ok = true
      end
    end
    a.truthy(ok, 'centered: ' .. table.concat(tried, '; '))
  end

  QA.check('R-070', 'Display math sits on its own line, centered in the text width', function()
    check_centered(
      40,
      u(0x03B1) .. ' + ' .. u(0x03B2),
      { 'intro', '', '$$', '\\alpha + \\beta', '$$', '', 'outro' }
    )
  end)

  QA.check('R-070', 'Display math centering follows the window width', function()
    check_centered(81, 'E = mc' .. u(0x00B2), { '$$', 'E = mc^2', '$$' })
    check_centered(30, 'E = mc' .. u(0x00B2), { '$$', 'E = mc^2', '$$' })
  end)

  QA.check(
    'R-070',
    'Display math centers in the capped text width on a very wide window',
    function()
      check_centered(240, 'a/b', { '$$\\frac{a}{b}$$' })
    end
  )

  QA.check(
    'R-070',
    'Display math in the middle of a paragraph breaks out onto its own line',
    function()
      local model = QA.layout({ 'before $$\\delta$$ after' }, { width = 60 })
      local l = display_line(model, u(0x03B4))
      a.not_nil(l)
      a.eq(u(0x03B4), vim.trim(l))
      local text = U.text(model)
      a.contains('before', text)
      a.contains('after', text)
    end
  )

  QA.check('R-070', 'Display math that falls back to source is still on its own line', function()
    local model = QA.layout(
      { 'intro', '', '$$', '\\qaunknown{z}', '$$', '', 'outro' },
      { width = 60 }
    )
    local l = display_line(model, '\\qaunknown{z}')
    a.not_nil(l)
    a.eq('\\qaunknown{z}', vim.trim(l))
  end)

  -- R-221 ------------------------------------------------------------------

  QA.check('R-221', 'Runs of whitespace between math tokens collapse to one space', function()
    a.eq('x = 1', render('x   =    1'))
    a.eq(u(0x03B1) .. ' ' .. u(0x03B2), render('\\alpha    \\beta'))
  end)

  QA.check(
    'R-221',
    'Whitespace at the ends of display math written on one line is omitted',
    function()
      local model = QA.layout({ 'intro', '', '$$   \\beta + 1   $$', '', 'outro' }, { width = 60 })
      local l = display_line(model, u(0x03B2))
      a.not_nil(l)
      a.eq(u(0x03B2) .. ' + 1', vim.trim(l))
      check_centered(60, u(0x03B2) .. ' + 1', { '$$   \\beta + 1   $$' })
    end
  )

  QA.check('R-221', 'A line break inside display math shows as one space between tokens', function()
    local model = QA.layout({ '$$', '\\alpha +', '  \\beta', '$$' }, { width = 60 })
    local text = U.text(model)
    a.contains(u(0x03B1) .. ' + ' .. u(0x03B2), text)
  end)

  QA.check('R-221', 'Whitespace at the ends of display math is omitted before centering', function()
    check_centered(50, u(0x03B3), { '$$', '   \\gamma    ', '$$' })
  end)

  -- R-222 ------------------------------------------------------------------

  QA.check('R-222', 'A square root of one token shows as the radical sign and the token', function()
    a.eq(u(0x221A) .. 'x', render('\\sqrt{x}'))
    a.eq(u(0x221A) .. '2', render('\\sqrt{2}'))
  end)

  QA.check(
    'R-222',
    'A square root of a multi-digit number or a token with a superscript needs no parentheses',
    function()
      a.eq(u(0x221A) .. '12', render('\\sqrt{12}'))
      a.eq(u(0x221A) .. 'x' .. u(0x00B2), render('\\sqrt{x^2}'))
    end
  )

  QA.check(
    'R-222',
    'A square root of several tokens puts parentheses around the radicand',
    function()
      a.eq(u(0x221A) .. '(x + 1)', render('\\sqrt{x + 1}'))
      a.eq(
        u(0x221A) .. '(a' .. u(0x00B2) .. ' + b' .. u(0x00B2) .. ')',
        render('\\sqrt{a^2 + b^2}')
      )
    end
  )

  QA.check('R-222', 'A square root inside a longer span keeps the text around it', function()
    a.eq('r = ' .. u(0x221A) .. '2', render('r = \\sqrt{2}'))
  end)

  -- R-223 ------------------------------------------------------------------

  QA.check('R-223', 'The epsilon and phi commands map to U+03B5 and U+03C6', function()
    a.eq(u(0x03B5), render('\\epsilon'))
    a.eq(u(0x03C6), render('\\phi'))
  end)

  QA.check('R-223', 'The varepsilon and varphi commands map to U+03F5 and U+03D5', function()
    a.eq(u(0x03F5), render('\\varepsilon'))
    a.eq(u(0x03D5), render('\\varphi'))
  end)

  -- R-276 ------------------------------------------------------------------

  QA.check(
    'R-276',
    'A fraction in the numerator follows the fraction rule at each level',
    function()
      a.eq('(a/b)/c', render('\\frac{\\frac{a}{b}}{c}'))
    end
  )

  QA.check(
    'R-276',
    'A fraction in the denominator follows the fraction rule at each level',
    function()
      a.eq('1/(2/3)', render('\\frac{1}{\\frac{2}{3}}'))
    end
  )

  QA.check(
    'R-276',
    'An inner fraction with several tokens gets its own parentheses inside the outer ones',
    function()
      a.eq('((a + b)/c)/d', render('\\frac{\\frac{a + b}{c}}{d}'))
    end
  )

  QA.check('R-276', 'A nested fraction never falls back to the source', function()
    local inner = render('\\frac{x}{\\frac{y}{\\frac{1}{2}}}')
    a.no_match('\\', inner)
    a.eq('x/(y/(1/2))', inner)
  end)

  -- R-277 ------------------------------------------------------------------

  QA.check(
    { 'R-277', 'R-069' },
    'A subscript letter with no Unicode subscript form makes the whole span show as source',
    function()
      local tex = 'y = x_b + \\alpha'
      local inner, model = U.inline(tex)
      a.eq(tex, inner)
      a.truthy(U.covered_by(model, tex, U.is_math_group), 'math group over the source')
    end
  )

  QA.check(
    'R-277',
    'One unmappable character in a braced subscript makes the whole span show as source',
    function()
      local tex = 'a_{1w} + 2'
      a.eq(tex, render(tex))
    end
  )

  QA.check(
    'R-277',
    'A superscript character with no Unicode superscript form makes the whole span show as source',
    function()
      local tex = 'x^{2Z}'
      a.eq(tex, render(tex))
    end
  )

  QA.check('R-277', 'A span whose scripts all have Unicode forms is still converted', function()
    a.eq('x' .. u(0x2099) .. ' + y' .. u(0x00B2), render('x_n + y^2'))
  end)

  -- R-278 ------------------------------------------------------------------

  QA.check(
    'R-278',
    'Inline source fallback collapses runs of whitespace between tokens to one space',
    function()
      a.eq('\\qaunknown x + y', render('\\qaunknown   x  +    y'))
    end
  )

  QA.check('R-278', 'Inline source fallback across a soft line break shows one space', function()
    local model = QA.layout({ 'AAA $\\qaunknown', 'x$ ZZZ' }, { width = 200 })
    a.contains('AAA \\qaunknown x ZZZ', (U.text(model):gsub('\n', ' ')))
  end)

  -- R-279 ------------------------------------------------------------------

  QA.check(
    'R-279',
    'Display math source fallback shows each source line on its own preview line',
    function()
      local model = QA.layout({
        'intro',
        '',
        '$$',
        '\\begin{cases}',
        'a & x > 0 \\\\',
        'b & x \\le 0',
        '\\end{cases}',
        '$$',
        '',
        'outro',
      }, { width = 80 })
      local want = { '\\begin{cases}', 'a & x > 0 \\\\', 'b & x \\le 0', '\\end{cases}' }
      local idx = {}
      for k, w in ipairs(want) do
        local _, i = display_line(model, w)
        a.not_nil(i, 'preview line for source line ' .. k)
        a.eq(w, vim.trim(model.lines[i]), 'source line ' .. k .. ' alone on its preview line')
        idx[k] = i
      end
      for k = 2, #want do
        a.eq(idx[k - 1] + 1, idx[k], 'consecutive preview lines')
      end
      a.no_match('%$', U.text(model))
    end
  )

  QA.check(
    'R-279',
    'Display math source fallback lines share one left edge and the block is centered in the text width',
    function()
      local width = 80
      local want = { '\\begin{cases}', 'a & x > 0 \\\\', 'b & x \\le 0', '\\end{cases}' }
      local model = QA.layout(
        vim.list_extend(
          vim.list_extend({ 'intro', '', '$$' }, vim.deepcopy(want)),
          { '$$', '', 'outro' }
        ),
        { width = width }
      )
      local leads, widest = {}, 0
      for k, w in ipairs(want) do
        local l = display_line(model, w)
        a.not_nil(l, 'preview line for source line ' .. k)
        leads[k] = U.lead(l)
        widest = math.max(widest, U.width(w))
      end
      for k = 2, #want do
        a.eq(leads[1], leads[k], 'common left edge for source line ' .. k)
      end
      local free = math.min(width - 4, 100) - widest
      local lead = leads[1] - U.margin(width)
      a.truthy(
        lead == math.floor(free / 2) or lead == math.ceil(free / 2),
        ('block centered: lead %d, free %d'):format(lead, free)
      )
    end
  )

  -- R-352 ------------------------------------------------------------------

  QA.check(
    'R-352',
    'Display math source fallback collapses whitespace runs and trims the ends of each source line',
    function()
      local model = QA.layout({
        '$$',
        '   \\begin{qa}   ',
        'a   &    b',
        '  \\end{qa}',
        '$$',
      }, { width = 80 })
      local text = U.text(model)
      a.contains('\\begin{qa}', text)
      local l = display_line(model, 'a &')
      a.not_nil(l, 'a line with the collapsed source: ' .. vim.inspect(model.lines))
      a.eq('a & b', vim.trim(l))
      local lb = display_line(model, '\\begin{qa}')
      local le = display_line(model, '\\end{qa}')
      a.eq(U.lead(lb), U.lead(le), 'leading whitespace of a source line does not move it')
    end
  )

  QA.check(
    'R-279',
    'Each display fallback line is in the math group',
    { severity = 'medium' },
    function()
      local model = QA.layout({ '$$', '\\begin{qa}', 'z', '\\end{qa}', '$$' }, { width = 80 })
      a.truthy(U.covered_by(model, '\\begin{qa}', U.is_math_group))
      a.truthy(U.covered_by(model, '\\end{qa}', U.is_math_group))
    end
  )

  -- R-353 ------------------------------------------------------------------

  QA.check('R-353', 'A dollar sign after a backslash does not open inline math', function()
    local model = QA.layout({ 'AAA \\$x$ ZZZ' }, { width = 200 })
    a.contains('AAA $x$ ZZZ', U.text(model))
    a.falsy(U.covered_by(model, 'x', U.is_math_group))
  end)

  QA.check('R-353', 'A dollar sign after a backslash does not close inline math', function()
    local model = QA.layout({ 'AAA $a\\$b$ ZZZ' }, { width = 200 })
    local text = U.text(model)
    a.contains('AAA ', text)
    a.contains(' ZZZ', text)
    a.truthy(U.covered_by(model, 'b', U.is_math_group), 'the span runs past the escaped dollar')
    a.falsy(U.covered_by(model, 'ZZZ', U.is_math_group))
  end)

  -- R-280 ------------------------------------------------------------------

  QA.check('R-280', 'A dollar sign followed by a space does not open inline math', function()
    local model = QA.layout({ 'AAA $ x + y$ ZZZ' }, { width = 200 })
    a.contains('AAA $ x + y$ ZZZ', U.text(model))
    a.falsy(U.covered_by(model, 'x + y', U.is_math_group))
  end)

  QA.check('R-280', 'A dollar sign with a space before it does not close inline math', function()
    local model = QA.layout({ 'AAA $x + y $ ZZZ' }, { width = 200 })
    a.contains('AAA $x + y $ ZZZ', U.text(model))
  end)

  QA.check('R-280', 'Two prices on one line stay literal text', function()
    local model = QA.layout({ 'it costs $5 and $10 today' }, { width = 200 })
    a.contains('it costs $5 and $10 today', U.text(model))
    a.falsy(U.covered_by(model, '5 and', U.is_math_group))
  end)

  QA.check('R-280', 'A dollar sign alone between spaces is literal text', function()
    local model = QA.layout({ 'a $ b' }, { width = 80 })
    a.contains('a $ b', U.text(model))
  end)
end)
