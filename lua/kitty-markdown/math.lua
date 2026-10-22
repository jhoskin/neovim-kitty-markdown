-- TeX to Unicode approximation for math spans (package L-C).
-- Signatures: AGENTS.md, Conventions, Lua module contracts.
--
-- `render(tex)` gets the content of a `$...$` or `$$...$$` span without the
-- delimiters. It returns the Unicode text when the whole span is
-- approximable and nil otherwise, so the layout can fall back to the TeX
-- source (SPEC.md R-063, R-069). The renderer is a recursive descent over
-- a token list; the mapping tables sit at the top so a new symbol is a one
-- line change.

local M = {}

-- R-064. Each base command maps to the Unicode GREEK SMALL/CAPITAL LETTER of
-- the same name, and each `\var` command to the Unicode symbol variant. TeX
-- draws `\epsilon` and `\phi` with the lunate and closed shapes that Unicode
-- files under the symbol code points, so the two glyph shapes are swapped
-- against TeX here. The plain letters are the ones readers expect in text
-- and the ones with subscript forms (R-066), so the letter wins. Every
-- capital is listed although TeX only defines the eleven that differ from
-- Latin capitals; the extra entries cost nothing.
local GREEK = {
  alpha = 'α',
  beta = 'β',
  gamma = 'γ',
  delta = 'δ',
  epsilon = 'ε',
  zeta = 'ζ',
  eta = 'η',
  theta = 'θ',
  iota = 'ι',
  kappa = 'κ',
  lambda = 'λ',
  mu = 'μ',
  nu = 'ν',
  xi = 'ξ',
  omicron = 'ο',
  pi = 'π',
  rho = 'ρ',
  sigma = 'σ',
  tau = 'τ',
  upsilon = 'υ',
  phi = 'φ',
  chi = 'χ',
  psi = 'ψ',
  omega = 'ω',
  varepsilon = 'ϵ',
  vartheta = 'ϑ',
  varpi = 'ϖ',
  varrho = 'ϱ',
  varsigma = 'ς',
  varphi = 'ϕ',
  Alpha = 'Α',
  Beta = 'Β',
  Gamma = 'Γ',
  Delta = 'Δ',
  Epsilon = 'Ε',
  Zeta = 'Ζ',
  Eta = 'Η',
  Theta = 'Θ',
  Iota = 'Ι',
  Kappa = 'Κ',
  Lambda = 'Λ',
  Mu = 'Μ',
  Nu = 'Ν',
  Xi = 'Ξ',
  Omicron = 'Ο',
  Pi = 'Π',
  Rho = 'Ρ',
  Sigma = 'Σ',
  Tau = 'Τ',
  Upsilon = 'Υ',
  Phi = 'Φ',
  Chi = 'Χ',
  Psi = 'Ψ',
  Omega = 'Ω',
}

-- R-067 operators plus the extras that are one table entry each. Spacing
-- commands render as the Unicode space of about the same width; `\!` is a
-- negative space and renders as nothing.
local SYMBOLS = {
  times = '×',
  cdot = '·',
  pm = '±',
  mp = '∓',
  div = '÷',
  leq = '≤',
  le = '≤',
  geq = '≥',
  ge = '≥',
  neq = '≠',
  ne = '≠',
  approx = '≈',
  equiv = '≡',
  sim = '∼',
  simeq = '≃',
  cong = '≅',
  propto = '∝',
  ll = '≪',
  gg = '≫',
  infty = '∞',
  sum = '∑',
  prod = '∏',
  int = '∫',
  oint = '∮',
  to = '→',
  rightarrow = '→',
  leftarrow = '←',
  gets = '←',
  leftrightarrow = '↔',
  Rightarrow = '⇒',
  Leftarrow = '⇐',
  Leftrightarrow = '⇔',
  implies = '⇒',
  iff = '⇔',
  mapsto = '↦',
  uparrow = '↑',
  downarrow = '↓',
  ['in'] = '∈',
  notin = '∉',
  ni = '∋',
  subset = '⊂',
  supset = '⊃',
  subseteq = '⊆',
  supseteq = '⊇',
  cup = '∪',
  cap = '∩',
  setminus = '∖',
  emptyset = '∅',
  varnothing = '∅',
  forall = '∀',
  exists = '∃',
  nexists = '∄',
  neg = '¬',
  lnot = '¬',
  land = '∧',
  wedge = '∧',
  lor = '∨',
  vee = '∨',
  oplus = '⊕',
  otimes = '⊗',
  partial = '∂',
  nabla = '∇',
  ldots = '…',
  dots = '…',
  cdots = '⋯',
  vdots = '⋮',
  ddots = '⋱',
  mid = '∣',
  parallel = '∥',
  perp = '⊥',
  ast = '∗',
  star = '⋆',
  circ = '∘',
  bullet = '•',
  degree = '°',
  angle = '∠',
  prime = '′',
  ell = 'ℓ',
  hbar = 'ℏ',
  aleph = 'ℵ',
  Re = 'ℜ',
  Im = 'ℑ',
  therefore = '∴',
  because = '∵',
  top = '⊤',
  bot = '⊥',
  vdash = '⊢',
  models = '⊨',
  langle = '⟨',
  rangle = '⟩',
  lfloor = '⌊',
  rfloor = '⌋',
  lceil = '⌈',
  rceil = '⌉',
  lbrace = '{',
  rbrace = '}',
  vert = '|',
  Vert = '‖',
  backslash = '\\',
  colon = ':',
  ['%'] = '%',
  ['{'] = '{',
  ['}'] = '}',
  ['_'] = '_',
  ['&'] = '&',
  ['#'] = '#',
  ['$'] = '$',
  ['|'] = '‖',
  [' '] = '\226\128\137', -- U+2009 thin space
  [','] = '\226\128\137',
  [';'] = '\226\128\133', -- U+2005 four-per-em space
  [':'] = '\226\129\159', -- U+205F medium mathematical space
  ['!'] = '',
  quad = ' ',
  qquad = ' ',
  -- Function names render as their upright text.
  sin = 'sin',
  cos = 'cos',
  tan = 'tan',
  cot = 'cot',
  sec = 'sec',
  csc = 'csc',
  arcsin = 'arcsin',
  arccos = 'arccos',
  arctan = 'arctan',
  sinh = 'sinh',
  cosh = 'cosh',
  tanh = 'tanh',
  log = 'log',
  ln = 'ln',
  lg = 'lg',
  exp = 'exp',
  lim = 'lim',
  max = 'max',
  min = 'min',
  sup = 'sup',
  inf = 'inf',
  det = 'det',
  dim = 'dim',
  gcd = 'gcd',
  deg = 'deg',
  arg = 'arg',
  ker = 'ker',
  Pr = 'Pr',
}

-- R-065. Keys are the characters of a rendered superscript argument. Sources:
-- Latin-1 Supplement for ¹ ² ³; Superscripts and Subscripts (U+2070..U+207F)
-- for the other digits, the signs, i, and n; Phonetic Extensions
-- (U+1D2C..U+1D61) for most letters and the Greek letters; Phonetic
-- Extensions Supplement for c (U+1D9C), f (U+1DA0), z (U+1DBB), ι (U+1DA5)
-- and θ (U+1DBF); Spacing Modifier Letters for h j r w y (U+02B0..U+02B8)
-- and l s x (U+02E1..U+02E3); Latin Extended-C for V (U+2C7D); Latin
-- Extended-D for C F Q (U+A7F2..U+A7F4). Unicode has no superscript q and
-- no superscript S, X, Y, or Z.
local SUPERSCRIPT = {
  ['0'] = '⁰',
  ['1'] = '¹',
  ['2'] = '²',
  ['3'] = '³',
  ['4'] = '⁴',
  ['5'] = '⁵',
  ['6'] = '⁶',
  ['7'] = '⁷',
  ['8'] = '⁸',
  ['9'] = '⁹',
  ['+'] = '⁺',
  ['-'] = '⁻',
  ['='] = '⁼',
  ['('] = '⁽',
  [')'] = '⁾',
  a = 'ᵃ',
  b = 'ᵇ',
  c = 'ᶜ',
  d = 'ᵈ',
  e = 'ᵉ',
  f = 'ᶠ',
  g = 'ᵍ',
  h = 'ʰ',
  i = 'ⁱ',
  j = 'ʲ',
  k = 'ᵏ',
  l = 'ˡ',
  m = 'ᵐ',
  n = 'ⁿ',
  o = 'ᵒ',
  p = 'ᵖ',
  r = 'ʳ',
  s = 'ˢ',
  t = 'ᵗ',
  u = 'ᵘ',
  v = 'ᵛ',
  w = 'ʷ',
  x = 'ˣ',
  y = 'ʸ',
  z = 'ᶻ',
  A = 'ᴬ',
  B = 'ᴮ',
  C = 'ꟲ',
  D = 'ᴰ',
  E = 'ᴱ',
  F = 'ꟳ',
  G = 'ᴳ',
  H = 'ᴴ',
  I = 'ᴵ',
  J = 'ᴶ',
  K = 'ᴷ',
  L = 'ᴸ',
  M = 'ᴹ',
  N = 'ᴺ',
  O = 'ᴼ',
  P = 'ᴾ',
  Q = 'ꟴ',
  R = 'ᴿ',
  T = 'ᵀ',
  U = 'ᵁ',
  V = 'ⱽ',
  W = 'ᵂ',
  ['α'] = 'ᵅ',
  ['β'] = 'ᵝ',
  ['γ'] = 'ᵞ',
  ['δ'] = 'ᵟ',
  ['θ'] = 'ᶿ',
  ['ι'] = 'ᶥ',
  ['φ'] = 'ᵠ',
  ['ϕ'] = 'ᵠ',
  ['χ'] = 'ᵡ',
}

-- R-066. Sources: Superscripts and Subscripts (U+2080..U+209C) for the
-- digits, the signs, and a e o x h k l m n p s t; Phonetic Extensions
-- (U+1D62..U+1D6A) for i r u v and the Greek letters β γ ρ φ χ; Latin
-- Extended-C for j (U+2C7C). Unicode has no other subscript letters.
local SUBSCRIPT = {
  ['0'] = '₀',
  ['1'] = '₁',
  ['2'] = '₂',
  ['3'] = '₃',
  ['4'] = '₄',
  ['5'] = '₅',
  ['6'] = '₆',
  ['7'] = '₇',
  ['8'] = '₈',
  ['9'] = '₉',
  ['+'] = '₊',
  ['-'] = '₋',
  ['='] = '₌',
  ['('] = '₍',
  [')'] = '₎',
  a = 'ₐ',
  e = 'ₑ',
  h = 'ₕ',
  i = 'ᵢ',
  j = 'ⱼ',
  k = 'ₖ',
  l = 'ₗ',
  m = 'ₘ',
  n = 'ₙ',
  o = 'ₒ',
  p = 'ₚ',
  r = 'ᵣ',
  s = 'ₛ',
  t = 'ₜ',
  u = 'ᵤ',
  v = 'ᵥ',
  x = 'ₓ',
  ['β'] = 'ᵦ',
  ['γ'] = 'ᵧ',
  ['ρ'] = 'ᵨ',
  ['φ'] = 'ᵩ',
  ['ϕ'] = 'ᵩ',
  ['χ'] = 'ᵪ',
}

-- Radical signs by index for `\sqrt[n]{x}`. Other indices are not
-- approximable.
local ROOTS = { ['2'] = '√', ['3'] = '∛', ['4'] = '∜' }

-- Single characters that render as themselves. ASCII letters and digits are
-- handled by pattern. `'` becomes a prime and `~` a space.
local PASSTHROUGH = {}
for c in ('+-=<>()[]|,.;:!/*'):gmatch('.') do
  PASSTHROUGH[c] = c
end
PASSTHROUGH["'"] = '′'
PASSTHROUGH['~'] = ' '

-- Characters that count as part of the token they follow when the fraction
-- and radical rules decide whether to add parentheses.
local SCRIPT_CHARS = {}
for _, table_ in ipairs({ SUPERSCRIPT, SUBSCRIPT }) do
  for _, glyph in pairs(table_) do
    SCRIPT_CHARS[glyph] = true
  end
end

local UTF8_CHAR = '[%z\1-\127\194-\244][\128-\191]*'

local function chars(s)
  local out = {}
  for ch in s:gmatch(UTF8_CHAR) do
    out[#out + 1] = ch
  end
  return out
end

local function is_letter(ch)
  if ch:match('^%a$') then
    return true
  end
  for _, glyph in pairs(GREEK) do
    if glyph == ch then
      return true
    end
  end
  return false
end

-- R-068: a rendered string is one token when it is one glyph, one number,
-- or one identifier. Attached scripts do not split a token, so `x^2`
-- stays `x²` without parentheses.
local function is_single_token(s)
  local cs = chars(s)
  if #cs == 0 then
    return false
  end
  if #cs == 1 then
    return true
  end
  local base = {}
  for _, ch in ipairs(cs) do
    if not SCRIPT_CHARS[ch] then
      base[#base + 1] = ch
    end
  end
  if #base == 0 then
    return true
  end
  local text = table.concat(base)
  if text:match('^%d+%.?%d*$') then
    return true
  end
  for _, ch in ipairs(base) do
    if not is_letter(ch) then
      return false
    end
  end
  return true
end

local function parenthesize(s)
  if is_single_token(s) then
    return s
  end
  return '(' .. s .. ')'
end

-- Tokenizer. A group token holds its own token list, so an unbalanced brace
-- is caught here and the renderer never sees one.
local function tokenize(tex)
  local pos = 1
  local len = #tex

  local function run()
    local tokens = {}
    while pos <= len do
      local c = tex:sub(pos, pos)
      if c == '}' then
        return tokens
      elseif c == '{' then
        pos = pos + 1
        local inner = run()
        if inner == nil or pos > len or tex:sub(pos, pos) ~= '}' then
          return nil
        end
        pos = pos + 1
        tokens[#tokens + 1] = { kind = 'group', tokens = inner }
      elseif c == '\\' then
        pos = pos + 1
        if pos > len then
          return nil
        end
        -- TeX swallows the whitespace after a control word. The renderer
        -- keeps it instead, so `a \leq b` keeps the space after `≤`.
        local name = tex:match('^%a+', pos)
        if name then
          pos = pos + #name
        else
          name = tex:sub(pos, pos)
          pos = pos + 1
        end
        tokens[#tokens + 1] = { kind = 'cmd', name = name }
      elseif c == '^' or c == '_' then
        pos = pos + 1
        tokens[#tokens + 1] = { kind = c == '^' and 'sup' or 'sub' }
      elseif c:match('%s') then
        local ws = tex:match('^%s+', pos)
        pos = pos + #ws
        tokens[#tokens + 1] = { kind = 'space' }
      else
        pos = pos + 1
        tokens[#tokens + 1] = { kind = 'char', char = c }
      end
    end
    return tokens
  end

  local tokens = run()
  if tokens == nil or pos <= len then
    return nil
  end
  return tokens
end

local render_list

-- Returns the rendered text of the item at `i` and the index after it, or
-- nil when the item is not approximable. A command that takes arguments
-- consumes them here.
local render_item

local function skip_spaces(tokens, i)
  while tokens[i] and tokens[i].kind == 'space' do
    i = i + 1
  end
  return i
end

-- Reads one argument: the next item after optional whitespace.
local function read_arg(tokens, i)
  i = skip_spaces(tokens, i)
  if tokens[i] == nil then
    return nil
  end
  return render_item(tokens, i)
end

local function map_script(text, table_)
  local out = {}
  for _, ch in ipairs(chars(text)) do
    local mapped = table_[ch]
    if mapped == nil then
      return nil
    end
    out[#out + 1] = mapped
  end
  return table.concat(out)
end

local function render_sqrt(tokens, i)
  local sign = ROOTS['2']
  i = skip_spaces(tokens, i)
  if tokens[i] and tokens[i].kind == 'char' and tokens[i].char == '[' then
    local index = {}
    i = i + 1
    while tokens[i] and not (tokens[i].kind == 'char' and tokens[i].char == ']') do
      if tokens[i].kind ~= 'char' then
        return nil
      end
      index[#index + 1] = tokens[i].char
      i = i + 1
    end
    if tokens[i] == nil then
      return nil
    end
    sign = ROOTS[table.concat(index)]
    if sign == nil then
      return nil
    end
    i = i + 1
  end
  local radicand, after = read_arg(tokens, i)
  if radicand == nil then
    return nil
  end
  return sign .. parenthesize(radicand), after
end

local function render_frac(tokens, i)
  local num, after_num = read_arg(tokens, i)
  if num == nil then
    return nil
  end
  local den, after_den = read_arg(tokens, after_num)
  if den == nil then
    return nil
  end
  return parenthesize(num) .. '/' .. parenthesize(den), after_den
end

local function render_cmd(tokens, i)
  local name = tokens[i].name
  if name == 'frac' then
    return render_frac(tokens, i + 1)
  elseif name == 'sqrt' then
    return render_sqrt(tokens, i + 1)
  end
  local glyph = GREEK[name] or SYMBOLS[name]
  if glyph == nil then
    return nil
  end
  return glyph, i + 1
end

function render_item(tokens, i)
  local token = tokens[i]
  if token.kind == 'char' then
    local c = token.char
    if c:match('^[%a%d]$') then
      return c, i + 1
    end
    local mapped = PASSTHROUGH[c]
    if mapped == nil then
      return nil
    end
    return mapped, i + 1
  elseif token.kind == 'group' then
    local text = render_list(token.tokens)
    if text == nil then
      return nil
    end
    return text, i + 1
  elseif token.kind == 'cmd' then
    return render_cmd(tokens, i)
  elseif token.kind == 'sup' or token.kind == 'sub' then
    local arg, after = read_arg(tokens, i + 1)
    if arg == nil then
      return nil
    end
    local mapped = map_script(arg, token.kind == 'sup' and SUPERSCRIPT or SUBSCRIPT)
    if mapped == nil then
      return nil
    end
    return mapped, after
  elseif token.kind == 'space' then
    return ' ', i + 1
  end
  return nil
end

-- Whitespace between items collapses to one space and is dropped at both
-- ends, so `$ a + b $` reads as `a + b`.
function render_list(tokens)
  local parts = {}
  local i = 1
  local pending_space = false
  while tokens[i] do
    if tokens[i].kind == 'space' then
      pending_space = #parts > 0
      i = i + 1
    else
      local text, after = render_item(tokens, i)
      if text == nil then
        return nil
      end
      if text:match('^ +$') then
        -- `\quad` and friends collapse with the whitespace around them.
        pending_space = #parts > 0
      elseif text ~= '' then
        if pending_space then
          parts[#parts + 1] = ' '
          pending_space = false
        end
        parts[#parts + 1] = text
      end
      i = after
    end
  end
  return table.concat(parts)
end

--- Renders a TeX math span as Unicode text.
--- @param tex string the span content without the `$` delimiters
--- @return string|nil text nil when any part of the span is not approximable
function M.render(tex)
  if type(tex) ~= 'string' then
    return nil
  end
  local tokens = tokenize(tex)
  if tokens == nil then
    return nil
  end
  return render_list(tokens)
end

return M
