local math_ = require('kitty-markdown.math')
local render = math_.render

-- Every entry of the module's mapping tables appears once in these lists,
-- so a dropped or mistyped glyph fails here.
local GREEK = {
  { 'alpha', 'α' },
  { 'beta', 'β' },
  { 'gamma', 'γ' },
  { 'delta', 'δ' },
  { 'epsilon', 'ε' },
  { 'zeta', 'ζ' },
  { 'eta', 'η' },
  { 'theta', 'θ' },
  { 'iota', 'ι' },
  { 'kappa', 'κ' },
  { 'lambda', 'λ' },
  { 'mu', 'μ' },
  { 'nu', 'ν' },
  { 'xi', 'ξ' },
  { 'omicron', 'ο' },
  { 'pi', 'π' },
  { 'rho', 'ρ' },
  { 'sigma', 'σ' },
  { 'tau', 'τ' },
  { 'upsilon', 'υ' },
  { 'phi', 'φ' },
  { 'chi', 'χ' },
  { 'psi', 'ψ' },
  { 'omega', 'ω' },
  { 'varepsilon', 'ϵ' },
  { 'vartheta', 'ϑ' },
  { 'varpi', 'ϖ' },
  { 'varrho', 'ϱ' },
  { 'varsigma', 'ς' },
  { 'varphi', 'ϕ' },
  { 'Alpha', 'Α' },
  { 'Beta', 'Β' },
  { 'Gamma', 'Γ' },
  { 'Delta', 'Δ' },
  { 'Epsilon', 'Ε' },
  { 'Zeta', 'Ζ' },
  { 'Eta', 'Η' },
  { 'Theta', 'Θ' },
  { 'Iota', 'Ι' },
  { 'Kappa', 'Κ' },
  { 'Lambda', 'Λ' },
  { 'Mu', 'Μ' },
  { 'Nu', 'Ν' },
  { 'Xi', 'Ξ' },
  { 'Omicron', 'Ο' },
  { 'Pi', 'Π' },
  { 'Rho', 'Ρ' },
  { 'Sigma', 'Σ' },
  { 'Tau', 'Τ' },
  { 'Upsilon', 'Υ' },
  { 'Phi', 'Φ' },
  { 'Chi', 'Χ' },
  { 'Psi', 'Ψ' },
  { 'Omega', 'Ω' },
}

local SYMBOLS = {
  { 'times', '×' },
  { 'cdot', '·' },
  { 'pm', '±' },
  { 'mp', '∓' },
  { 'div', '÷' },
  { 'leq', '≤' },
  { 'le', '≤' },
  { 'geq', '≥' },
  { 'ge', '≥' },
  { 'neq', '≠' },
  { 'ne', '≠' },
  { 'approx', '≈' },
  { 'equiv', '≡' },
  { 'sim', '∼' },
  { 'simeq', '≃' },
  { 'cong', '≅' },
  { 'propto', '∝' },
  { 'll', '≪' },
  { 'gg', '≫' },
  { 'infty', '∞' },
  { 'sum', '∑' },
  { 'prod', '∏' },
  { 'int', '∫' },
  { 'oint', '∮' },
  { 'to', '→' },
  { 'rightarrow', '→' },
  { 'leftarrow', '←' },
  { 'gets', '←' },
  { 'leftrightarrow', '↔' },
  { 'Rightarrow', '⇒' },
  { 'Leftarrow', '⇐' },
  { 'Leftrightarrow', '⇔' },
  { 'implies', '⇒' },
  { 'iff', '⇔' },
  { 'mapsto', '↦' },
  { 'uparrow', '↑' },
  { 'downarrow', '↓' },
  { 'in', '∈' },
  { 'notin', '∉' },
  { 'ni', '∋' },
  { 'subset', '⊂' },
  { 'supset', '⊃' },
  { 'subseteq', '⊆' },
  { 'supseteq', '⊇' },
  { 'cup', '∪' },
  { 'cap', '∩' },
  { 'setminus', '∖' },
  { 'emptyset', '∅' },
  { 'varnothing', '∅' },
  { 'forall', '∀' },
  { 'exists', '∃' },
  { 'nexists', '∄' },
  { 'neg', '¬' },
  { 'lnot', '¬' },
  { 'land', '∧' },
  { 'wedge', '∧' },
  { 'lor', '∨' },
  { 'vee', '∨' },
  { 'oplus', '⊕' },
  { 'otimes', '⊗' },
  { 'partial', '∂' },
  { 'nabla', '∇' },
  { 'ldots', '…' },
  { 'dots', '…' },
  { 'cdots', '⋯' },
  { 'vdots', '⋮' },
  { 'ddots', '⋱' },
  { 'mid', '∣' },
  { 'parallel', '∥' },
  { 'perp', '⊥' },
  { 'ast', '∗' },
  { 'star', '⋆' },
  { 'circ', '∘' },
  { 'bullet', '•' },
  { 'degree', '°' },
  { 'angle', '∠' },
  { 'prime', '′' },
  { 'ell', 'ℓ' },
  { 'hbar', 'ℏ' },
  { 'aleph', 'ℵ' },
  { 'Re', 'ℜ' },
  { 'Im', 'ℑ' },
  { 'therefore', '∴' },
  { 'because', '∵' },
  { 'top', '⊤' },
  { 'bot', '⊥' },
  { 'vdash', '⊢' },
  { 'models', '⊨' },
  { 'langle', '⟨' },
  { 'rangle', '⟩' },
  { 'lfloor', '⌊' },
  { 'rfloor', '⌋' },
  { 'lceil', '⌈' },
  { 'rceil', '⌉' },
  { 'lbrace', '{' },
  { 'rbrace', '}' },
  { 'vert', '|' },
  { 'Vert', '‖' },
  { 'backslash', '\\' },
  { 'colon', ':' },
  { '%', '%' },
  { '{', '{' },
  { '}', '}' },
  { '_', '_' },
  { '&', '&' },
  { '#', '#' },
  { '$', '$' },
  { '|', '‖' },
  { ' ', '\226\128\137' },
  { ',', '\226\128\137' },
  { ';', '\226\128\133' },
  { ':', '\226\129\159' },
}

local FUNCTIONS = {
  'sin',
  'cos',
  'tan',
  'cot',
  'sec',
  'csc',
  'arcsin',
  'arccos',
  'arctan',
  'sinh',
  'cosh',
  'tanh',
  'log',
  'ln',
  'lg',
  'exp',
  'lim',
  'max',
  'min',
  'sup',
  'inf',
  'det',
  'dim',
  'gcd',
  'deg',
  'arg',
  'ker',
  'Pr',
}

-- Source characters with a superscript form and the expected glyphs, in the
-- same order.
local SUPERSCRIPT_SRC = '0123456789+-=()abcdefghijklmnoprstuvwxyzABCDEFGHIJKLMNOPQRTUVW'
local SUPERSCRIPT_OUT =
  '⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ᵃᵇᶜᵈᵉᶠᵍʰⁱʲᵏˡᵐⁿᵒᵖʳˢᵗᵘᵛʷˣʸᶻᴬᴮꟲᴰᴱꟳᴳᴴᴵᴶᴷᴸᴹᴺᴼᴾꟴᴿᵀᵁⱽᵂ'
local SUBSCRIPT_SRC = '0123456789+-=()aehijklmnoprstuvx'
local SUBSCRIPT_OUT =
  '₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎ₐₑₕᵢⱼₖₗₘₙₒₚᵣₛₜᵤᵥₓ'

describe('math.render', function()
  describe('R-064 Greek letters', function()
    it('maps every Greek letter command', function()
      for _, pair in ipairs(GREEK) do
        assert.are.equal(pair[2], render('\\' .. pair[1]), pair[1])
      end
    end)

    it('renders adjacent letters without a separator', function()
      assert.are.equal('αβ', render('\\alpha\\beta'))
      assert.are.equal('Ωx', render('\\Omega{}x'))
    end)
  end)

  describe('R-065 superscripts', function()
    it('maps a single digit', function()
      assert.are.equal('x²', render('x^2'))
    end)

    it('maps a group of digits and signs', function()
      assert.are.equal('x¹⁰', render('x^{10}'))
      assert.are.equal('e⁻¹', render('e^{-1}'))
      assert.are.equal('x⁽ⁱ⁺¹⁾', render('x^{(i+1)}'))
      assert.are.equal('a⁼', render('a^='))
    end)

    it('maps every character with a superscript form', function()
      for i = 1, #SUPERSCRIPT_SRC do
        local c = SUPERSCRIPT_SRC:sub(i, i)
        local out = render('^' .. c)
        assert.is_not_nil(out, c)
        assert.is_true(SUPERSCRIPT_OUT:find(out, 1, true) ~= nil, c)
      end
    end)

    it('maps the Greek letters with superscript forms', function()
      assert.are.equal(
        'xᵅᵝᵞᵟᶿᶥᵠᵡ',
        render('x^{\\alpha\\beta\\gamma\\delta\\theta\\iota\\phi\\chi}')
      )
      assert.are.equal('xᵠ', render('x^\\varphi'))
    end)

    it('is all or nothing', function()
      assert.is_nil(render('x^q'))
      assert.is_nil(render('x^{2q}'))
      assert.is_nil(render('x^S'))
      assert.is_nil(render('e^{i\\pi}'))
      assert.is_nil(render('x^\\infty'))
    end)

    it('rejects a missing or doubled argument', function()
      assert.is_nil(render('x^'))
      assert.is_nil(render('x^^2'))
    end)
  end)

  describe('R-066 subscripts', function()
    it('maps a single digit', function()
      assert.are.equal('x₁', render('x_1'))
    end)

    it('maps a group of digits, signs, and letters', function()
      assert.are.equal('a₁₂', render('a_{12}'))
      assert.are.equal('xᵢ₊₁', render('x_{i+1}'))
      assert.are.equal('x₍ₙ₎', render('x_{(n)}'))
      assert.are.equal('a₌', render('a_='))
    end)

    it('maps every character with a subscript form', function()
      for i = 1, #SUBSCRIPT_SRC do
        local c = SUBSCRIPT_SRC:sub(i, i)
        local out = render('_' .. c)
        assert.is_not_nil(out, c)
        assert.is_true(SUBSCRIPT_OUT:find(out, 1, true) ~= nil, c)
      end
    end)

    it('maps the Greek letters with subscript forms', function()
      assert.are.equal('xᵦᵧᵨᵩᵪ', render('x_{\\beta\\gamma\\rho\\phi\\chi}'))
      assert.are.equal('xᵩ', render('x_\\varphi'))
    end)

    it('is all or nothing', function()
      assert.is_nil(render('x_q'))
      assert.is_nil(render('x_{1q}'))
      assert.is_nil(render('x_A'))
      assert.is_nil(render('x_\\alpha'))
    end)

    it('rejects a missing argument', function()
      assert.is_nil(render('x_'))
    end)

    it('combines a subscript and a superscript on one base', function()
      assert.are.equal('∑ᵢ₌₁ⁿ xᵢ', render('\\sum_{i=1}^{n} x_i'))
      assert.are.equal('x₁²', render('x_1^2'))
    end)
  end)

  describe('R-067 operators', function()
    it('maps the operators the spec names', function()
      assert.are.equal('a × b', render('a \\times b'))
      assert.are.equal('a · b', render('a \\cdot b'))
      assert.are.equal('a ± b', render('a \\pm b'))
      assert.are.equal('a ≤ b', render('a \\leq b'))
      assert.are.equal('a ≥ b', render('a \\geq b'))
      assert.are.equal('a ≠ b', render('a \\neq b'))
      assert.are.equal('a ≈ b', render('a \\approx b'))
      assert.are.equal('∞', render('\\infty'))
      assert.are.equal('∑ x', render('\\sum x'))
      assert.are.equal('∫ x', render('\\int x'))
      assert.are.equal('√x', render('\\sqrt{x}'))
      assert.are.equal('a → b', render('a \\to b'))
    end)

    it('maps every symbol command', function()
      for _, pair in ipairs(SYMBOLS) do
        assert.are.equal(pair[2], render('\\' .. pair[1]), pair[1])
      end
    end)

    it('renders function names as text', function()
      for _, name in ipairs(FUNCTIONS) do
        assert.are.equal(name, render('\\' .. name), name)
      end
      assert.are.equal('sin² x', render('\\sin^2 x'))
    end)

    it('renders the negative thin space as nothing', function()
      assert.are.equal('ab', render('a\\!b'))
    end)

    it('collapses quad with the whitespace around it', function()
      assert.are.equal('a b', render('a\\quad b'))
      assert.are.equal('a b', render('a \\qquad b'))
    end)
  end)

  describe('R-067 square roots', function()
    it('renders a single token without parentheses', function()
      assert.are.equal('√x', render('\\sqrt{x}'))
      assert.are.equal('√x', render('\\sqrt x'))
      assert.are.equal('√2', render('\\sqrt{2}'))
      assert.are.equal('√x²', render('\\sqrt{x^2}'))
    end)

    it('wraps many tokens in parentheses', function()
      assert.are.equal('√(a+b)', render('\\sqrt{a+b}'))
      assert.are.equal('√(a + b)', render('\\sqrt{a + b}'))
    end)

    it('renders cube and fourth roots', function()
      assert.are.equal('∛8', render('\\sqrt[3]{8}'))
      assert.are.equal('∜(x+1)', render('\\sqrt[4]{x+1}'))
    end)

    it('rejects an index without a radical sign', function()
      assert.is_nil(render('\\sqrt[5]{8}'))
      assert.is_nil(render('\\sqrt[n]{8}'))
    end)

    it('rejects a missing radicand', function()
      assert.is_nil(render('\\sqrt'))
    end)
  end)

  describe('R-068 fractions', function()
    it('renders single tokens without parentheses', function()
      assert.are.equal('a/b', render('\\frac{a}{b}'))
      assert.are.equal('a/b', render('\\frac ab'))
      assert.are.equal('12/α', render('\\frac{12}{\\alpha}'))
      assert.are.equal('1.5/2', render('\\frac{1.5}{2}'))
      assert.are.equal('x²/2', render('\\frac{x^2}{2}'))
      assert.are.equal('ab/2', render('\\frac{ab}{2}'))
    end)

    it('wraps many tokens in parentheses', function()
      assert.are.equal('(a+b)/2', render('\\frac{a+b}{2}'))
      assert.are.equal('1/(a+b)', render('\\frac{1}{a+b}'))
      assert.are.equal('(a+b)/(c-d)', render('\\frac{a+b}{c-d}'))
      assert.are.equal('(-1)/2', render('\\frac{-1}{2}'))
    end)

    it('nests without parentheses around single tokens', function()
      assert.are.equal('(a/b)/c', render('\\frac{\\frac{a}{b}}{c}'))
      assert.are.equal('1/(a/b)', render('\\frac{1}{\\frac{a}{b}}'))
    end)

    it('nests with parentheses on both levels', function()
      assert.are.equal('((a+b)/c)/(d+e)', render('\\frac{\\frac{a+b}{c}}{d+e}'))
    end)

    it('rejects a missing argument', function()
      assert.is_nil(render('\\frac{a}'))
      assert.is_nil(render('\\frac'))
    end)
  end)

  describe('R-063 mixed spans', function()
    it('renders the README example', function()
      assert.are.equal('α² + β₁', render('\\alpha^2 + \\beta_1'))
    end)

    it('passes ASCII letters, digits, and operators through', function()
      assert.are.equal('2x + 3y = 7', render('2x + 3y = 7'))
      assert.are.equal('a < b > c', render('a < b > c'))
      assert.are.equal('f(x) = [a, b]; |x|: y! z/w * 2.', render('f(x) = [a, b]; |x|: y! z/w * 2.'))
    end)

    it('maps a prime and a tie', function()
      assert.are.equal('f′(x)', render("f'(x)"))
      assert.are.equal('a b', render('a~b'))
    end)

    it('renders a group as its content', function()
      assert.are.equal('a+b', render('{a+b}'))
      assert.are.equal('', render('{}'))
      assert.are.equal('x', render('x^{}'))
    end)
  end)

  describe('whitespace', function()
    it('collapses runs to one space and trims the ends', function()
      assert.are.equal('a + b', render('  a  +  b  '))
    end)

    it('keeps a space after a command', function()
      assert.are.equal('α x', render('\\alpha x'))
      assert.are.equal('x α', render('x \\alpha'))
    end)

    it('ignores whitespace before an argument', function()
      assert.are.equal('x²', render('x^ 2'))
      assert.are.equal('a/b', render('\\frac {a} {b}'))
    end)
  end)

  describe('R-069 not approximable', function()
    it('returns nil for an environment', function()
      assert.is_nil(render('\\begin{matrix} a & b \\end{matrix}'))
    end)

    it('returns nil for left and right delimiters', function()
      assert.is_nil(render('\\left( x \\right)'))
    end)

    it('returns nil for text', function()
      assert.is_nil(render('\\text{hi}'))
    end)

    it('returns nil for an unknown command', function()
      assert.is_nil(render('\\foo'))
      assert.is_nil(render('\\mathbb{R}'))
    end)

    it('returns nil for unbalanced braces', function()
      assert.is_nil(render('{a'))
      assert.is_nil(render('a}'))
      assert.is_nil(render('\\frac{a}{b'))
    end)

    it('returns nil for a trailing backslash and a bare special', function()
      assert.is_nil(render('\\'))
      assert.is_nil(render('a & b'))
      assert.is_nil(render('100%'))
      assert.is_nil(render('a\\\\b'))
    end)

    it('returns nil when one part of the span fails', function()
      assert.is_nil(render('\\alpha + \\foo'))
      assert.is_nil(render('\\int_0^\\infty e^{-x} dx'))
    end)

    it('returns nil for a non-string', function()
      assert.is_nil(render(nil))
    end)
  end)
end)
