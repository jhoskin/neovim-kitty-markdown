-- End-to-end mermaid checks: headless nvim, the real helper, a faked kitty
-- terminal, and a saved markdown buffer.

local QA = require('qa')
local U = require('mermaid_util')
local a = QA.assert

local ctx

local function text_width_cells()
  local win
  for _, w in ipairs(vim.api.nvim_list_wins()) do
    if vim.api.nvim_win_get_buf(w) == ctx.pbuf then
      win = w
    end
  end
  QA.need(win ~= nil, 'a preview window')
  return math.min(vim.api.nvim_win_get_width(win) - 4, 100)
end

-- Opens a preview of one diagram, saves, and waits for the block to settle.
local function run(source, opts)
  ctx = U.open(U.doc(source), opts)
  QA.need(ctx.pbuf ~= nil, 'a preview buffer after :KittyMarkdown open')
  U.save(ctx)
  local rows = U.settle(ctx)
  return rows, U.rows_text(rows)
end

local function placed(text)
  return text:find(U.PLACEHOLDER, 1, true) ~= nil
end

local function last_png()
  local pngs = U.transmitted_pngs(ctx)
  a.ge(1, #pngs, 'transmitted images')
  return pngs[#pngs]
end

-- The body rows of the boxes, without borders and padding.
local function inner_rows(rows)
  local out = {}
  for _, r in ipairs(rows) do
    local inner = r:match('│%s?(.-)%s*│%s*$')
    if inner then
      out[#out + 1] = inner
    end
  end
  return out
end

-- The top border rows of the boxes.
local function title_rows(rows)
  local out = {}
  for _, r in ipairs(rows) do
    if r:find('╭', 1, true) then
      out[#out + 1] = r
    end
  end
  return table.concat(out, '\n')
end

local FLOW = U.source('types/flowchart')
local PARSE_FIX =
  'mermaid parses the diagram text with one newline appended (see Diagram.fromText in the pinned mermaid), which sets the line, the excerpt, and the caret for errors at the end of the text'

QA.describe('mermaid: end to end', function()
  QA.before_each(function()
    QA.reset_plugin()
    ctx = nil
  end)
  QA.after_each(function()
    U.close(ctx)
    ctx = nil
  end)

  QA.check(
    'R-097',
    'A fenced code block with the info string mermaid is a mermaid pixel block',
    function()
      local model = QA.layout(U.doc(FLOW), { width = 80 })
      a.eq(1, #model.blocks, 'one pixel block')
      a.matches('[Mm]ermaid', tostring(model.blocks[1].kind), 'block kind')
      a.contains('Kettle', tostring(model.blocks[1].source), 'block source is the fence content')
    end
  )

  QA.check('R-097', 'A fence with another info string is not a mermaid block', function()
    local lines = U.doc(FLOW)
    lines[3] = '```text'
    local model = QA.layout(lines, { width = 80 })
    for _, b in ipairs(model.blocks or {}) do
      a.no_match('[Mm]ermaid', tostring(b.kind), 'non-mermaid fence kind')
    end
  end)

  QA.check(
    'R-097',
    'A mermaid fence with a tilde fence is also a mermaid block',
    { severity = 'medium' },
    function()
      local lines = U.doc(FLOW)
      lines[3] = '~~~mermaid'
      for i, l in ipairs(lines) do
        if l == '```' then
          lines[i] = '~~~'
        end
      end
      local model = QA.layout(lines, { width = 80 })
      a.eq(1, #model.blocks, 'one pixel block')
      a.matches('[Mm]ermaid', tostring(model.blocks[1].kind))
    end
  )

  QA.check(
    'R-098',
    'A mermaid block wider than the text width is placed as a PNG at the target width',
    function()
      local _, text = run(U.source('types/journey'))
      a.truthy(placed(text), 'the diagram is placed with placeholder cells')
      local png = last_png()
      local w = QA.png_size(png)
      a.not_nil(w, 'the transmitted image is a PNG')
      a.near(text_width_cells() * U.CELL_W, w, 1, 'PNG width equals the target width')
    end
  )

  QA.check(
    { 'R-098', 'R-180' },
    'A mermaid diagram whose natural width fits the target width is placed at its natural size',
    function()
      local _, text = run(FLOW)
      a.truthy(placed(text), 'the diagram is placed')
      local w, h = QA.png_size(last_png())
      local nat = U.oracle_size('types/flowchart')
      a.near(nat[1], w, 2, 'natural width')
      a.near(nat[2], h, 2, 'natural height')
    end
  )

  QA.check('R-229', 'The plugin gets mermaid PNGs with a transparent background', function()
    local _, text = run(FLOW)
    a.truthy(placed(text), 'the diagram is placed')
    local info = U.png_info(last_png())
    for i, c in ipairs(info.corners) do
      a.eq(0, c[4], 'corner ' .. i .. ' alpha')
    end
    a.ge(1, info.transparent, 'transparent pixels')
  end)

  QA.check(
    'R-099',
    'On the dark palette a mermaid diagram without a theme renders with the dark theme',
    function()
      local _, text = run(FLOW, { setup = { theme = 'dark' } })
      a.truthy(placed(text), 'the diagram is placed')
      local got = last_png()
      local dark = U.cli(FLOW, text_width_cells() * U.CELL_W, 'dark', 'transparent')
      local default = U.cli(FLOW, text_width_cells() * U.CELL_W, 'default', 'transparent')
      a.ge(0.999, U.same_fraction(got, dark), 'same pixels as the dark theme')
      a.le(0.99, U.same_fraction(got, default), 'differs from the default theme')
    end
  )

  QA.check(
    'R-100',
    'On the light palette a mermaid diagram without a theme renders with the default theme',
    function()
      local _, text = run(FLOW, { setup = { theme = 'light' } })
      a.truthy(placed(text), 'the diagram is placed')
      local got = last_png()
      local default = U.cli(FLOW, text_width_cells() * U.CELL_W, 'default', 'transparent')
      a.ge(0.999, U.same_fraction(got, default), 'same pixels as the default theme')
    end
  )

  QA.check(
    { 'R-099', 'R-174' },
    'On the dark palette a theme set in the diagram wins over the dark theme',
    function()
      local src = '%%{init: {"theme": "forest"}}%%\n' .. FLOW
      local _, text = run(src, { setup = { theme = 'dark' } })
      a.truthy(placed(text), 'the diagram is placed')
      local forest = U.cli(FLOW, text_width_cells() * U.CELL_W, 'forest', 'transparent')
      a.ge(0.999, U.same_fraction(last_png(), forest), 'same pixels as the forest theme')
    end
  )

  QA.check(
    'R-100',
    'On the light palette a theme set in front matter wins over the default theme',
    function()
      local src = '---\nconfig:\n  theme: neutral\n---\n' .. FLOW
      local _, text = run(src, { setup = { theme = 'light' } })
      a.truthy(placed(text), 'the diagram is placed')
      local neutral = U.cli(FLOW, text_width_cells() * U.CELL_W, 'neutral', 'transparent')
      a.ge(0.999, U.same_fraction(last_png(), neutral), 'same pixels as the neutral theme')
    end
  )

  QA.check(
    'R-101',
    'The mermaid theme option selects the theme regardless of the palette',
    function()
      local _, text = run(FLOW, { setup = { theme = 'dark', mermaid = { theme = 'neutral' } } })
      a.truthy(placed(text), 'the diagram is placed')
      local neutral = U.cli(FLOW, text_width_cells() * U.CELL_W, 'neutral', 'transparent')
      a.ge(0.999, U.same_fraction(last_png(), neutral), 'same pixels as the neutral theme')
    end
  )

  QA.check('R-101', 'The mermaid theme option auto follows the palette', function()
    local _, text = run(FLOW, { setup = { theme = 'light', mermaid = { theme = 'auto' } } })
    a.truthy(placed(text), 'the diagram is placed')
    local default = U.cli(FLOW, text_width_cells() * U.CELL_W, 'default', 'transparent')
    a.ge(0.999, U.same_fraction(last_png(), default), 'same pixels as the default theme')
  end)

  QA.check('R-101', 'A theme set in the diagram wins over the mermaid theme option', function()
    local src = '%%{init: {"theme": "base"}}%%\n' .. FLOW
    local _, text = run(src, { setup = { theme = 'light', mermaid = { theme = 'forest' } } })
    a.truthy(placed(text), 'the diagram is placed')
    local base = U.cli(FLOW, text_width_cells() * U.CELL_W, 'base', 'transparent')
    a.ge(0.999, U.same_fraction(last_png(), base), 'same pixels as the base theme')
  end)

  QA.check(
    'R-234',
    'An unknown theme name in the diagram gives the theme of the base config in the preview',
    function()
      local src = '%%{init: {"theme": "midnight-plum"}}%%\n' .. FLOW
      local _, text = run(src, { setup = { theme = 'light', mermaid = { theme = 'forest' } } })
      a.truthy(placed(text), 'the diagram is placed, not a failure box')
      local forest = U.cli(FLOW, text_width_cells() * U.CELL_W, 'forest', 'transparent')
      a.ge(0.999, U.same_fraction(last_png(), forest), 'the configured theme still applies')
    end
  )

  QA.check(
    { 'R-111', 'R-177' },
    'A diagram that does not parse shows a failure box with the diagram type and the oracle parse error line',
    function()
      local src = U.source('parse/sequence_missing_actor')
      local _, n = U.oracle_error('sequence_missing_actor')
      local _, text = run(src)
      a.falsy(placed(text), 'no image is placed')
      a.contains('Parse error on line ' .. n, text, 'failure box reason')
      a.matches('[Ss]equence', text, 'the failure box names the diagram type')
    end
  )

  for _, c in ipairs({
    { 'parse/sequence_missing_actor', 'sequenceDiagram', '' },
    { 'parse/state_missing_target', 'stateDiagram-v2', '' },
    { 'parse/flowchart_double_arrow', 'flowchart', '%%{init: {"theme": "forest"}}%%\n' },
    { 'parse/er_missing_colon', 'erDiagram', '---\ntitle: Books\n---\n' },
  }) do
    QA.check(
      { 'R-111', 'R-300' },
      'The parse failure box of a '
        .. c[2]
        .. ' diagram names its first keyword, after any front matter or directive',
      function()
        local rows = run(c[3] .. U.source(c[1]))
        local titles = title_rows(rows)
        a.matches('[ ─]' .. vim.pesc(c[2]) .. ' ', titles, 'the first keyword in the box title')
      end
    )
  end

  QA.check(
    { 'R-111', 'R-177' },
    'A flowchart that does not parse at the end of the text shows the oracle parse error line in its failure box',
    { fix = PARSE_FIX },
    function()
      local name = 'flowchart_unclosed_pipe'
      local _, n = U.oracle_error(name)
      local _, text = run(U.source('parse/' .. name))
      a.falsy(placed(text), 'no image is placed')
      a.contains('Parse error on line ' .. n, text, 'failure box reason')
      a.matches('[Ff]lowchart', text, 'the failure box names the diagram type')
    end
  )

  QA.check(
    'R-177',
    'The failure box reason is exactly the parse error line with no extra text',
    { severity = 'medium' },
    function()
      local src = U.source('parse/state_missing_target')
      local _, n = U.oracle_error('state_missing_target')
      local rows = run(src)
      local found = false
      for _, r in ipairs(rows) do
        local inner = r:gsub('^%s*│%s*', ''):gsub('%s*│%s*$', '')
        if inner:find('Parse error on line', 1, true) then
          found = true
          a.eq('Parse error on line ' .. n, inner, 'reason row')
        end
      end
      a.truthy(found, 'a reason row')
    end
  )

  QA.check(
    'R-178',
    'A parse error writes the full oracle message with the caret line and the expected tokens to :messages',
    function()
      local name = 'sequence_missing_actor'
      local oracle = U.oracle_error(name)
      run(U.source('parse/' .. name))
      local msgs = U.messages(ctx)
      a.contains(oracle[1], msgs, 'the parse error line with its colon')
      a.contains(oracle[3], msgs, 'the caret line')
      a.contains(oracle[4], msgs, 'the expected tokens')
    end
  )

  QA.check(
    'R-178',
    'The parse error in :messages matches the oracle text for a class diagram',
    { severity = 'medium', fix = PARSE_FIX },
    function()
      local name = 'class_unclosed_body'
      local oracle = U.oracle_error(name)
      run(U.source('parse/' .. name))
      local msgs = U.messages(ctx)
      a.contains(table.concat(oracle, '\n'), msgs, 'the full oracle message')
    end
  )

  QA.check('R-178', 'A diagram that renders writes no parse error to :messages', function()
    run(FLOW)
    a.not_contains('Parse error', U.messages(ctx))
  end)

  QA.check(
    'R-230',
    'A diagram that matches no diagram type shows a failure box with Parse error on line 1',
    function()
      local _, text = run('teapots are lovely\nmore words here')
      a.falsy(placed(text), 'no image is placed')
      a.contains('Parse error on line 1', text)
    end
  )

  QA.check(
    { 'R-231', 'R-297' },
    'The no-diagram-type message in :messages keeps the line breaks of the text, one for a removed directive line, and none for a comment',
    function()
      run(
        '%%{init: {"theme": "dark"}}%%\nteapots are lovely\n%% a remark about tea\n\nand so are kettles'
      )
      local msgs = U.messages(ctx)
      a.matches(
        'No diagram type detected matching given configuration for text: \nteapots are lovely\n\nand so are kettles',
        msgs
      )
      a.not_contains('remark', msgs)
    end
  )

  QA.check(
    { 'R-231', 'R-297' },
    'The no-diagram-type message in :messages is not shortened for a long text',
    function()
      local lines = {}
      for i = 1, 30 do
        lines[i] = 'teapot line ' .. i .. ' with some words to make it long'
      end
      local text = table.concat(lines, '\n')
      run(text)
      a.contains('for text: ' .. text, U.messages(ctx))
    end
  )

  QA.check(
    'R-231',
    'A diagram that matches no diagram type writes the no-diagram-type message with the text to :messages',
    function()
      run('teapots are lovely')
      local msgs = U.messages(ctx)
      a.contains(
        'No diagram type detected matching given configuration for text: teapots are lovely',
        msgs
      )
    end
  )

  QA.check(
    'R-232',
    'Front matter that does not close shows a failure box with Parse error on line 1',
    function()
      local _, text = run('---\ntitle: Tea\nflowchart LR\n  A --> B')
      a.falsy(placed(text), 'no image is placed')
      a.contains('Parse error on line 1', text)
    end
  )

  QA.check(
    'R-233',
    'Front matter that is not valid YAML shows a failure box with Parse error on line 1',
    function()
      local _, text = run('---\nconfig: {theme: [forest\n---\nflowchart LR\n  A --> B')
      a.falsy(placed(text), 'no image is placed')
      a.contains('Parse error on line 1', text)
    end
  )

  local unsupported = {
    {
      'R-112',
      'The handDrawn look in an init directive',
      '%%{init: {"look": "handDrawn"}}%%\n' .. FLOW,
      'look: handDrawn',
    },
    {
      'R-175',
      'The handDrawn look in front matter',
      '---\nconfig:\n  look: handDrawn\n---\n' .. FLOW,
      'look: handDrawn',
    },
    {
      'R-112',
      'ELK layout in front matter',
      '---\nconfig:\n  layout: elk\n---\n' .. FLOW,
      'layout: elk',
    },
    {
      'R-175',
      'ELK layout in an init directive',
      '%%{init: {"layout": "elk"}}%%\n' .. FLOW,
      'layout: elk',
    },
    { 'R-112', 'The ELK flowchart header', 'flowchart-elk LR\n  A --> B', 'layout: elk' },
    {
      'R-112',
      'An icon pack icon on a node',
      'flowchart LR\n  A@{ icon: "fa:cog", form: "square", label: "Gear" } --> B',
      'icon pack',
    },
    {
      'R-112',
      'An image node',
      'flowchart LR\n  A@{ img: "https://example.invalid/a.png", label: "Pic", pos: "t" } --> B',
      'image node',
    },
    {
      'R-175',
      'The neo look in an init directive',
      '%%{init: {"look": "neo"}}%%\n' .. FLOW,
      'look: neo',
    },
    {
      'R-175',
      'The neo look in front matter',
      '---\nconfig:\n  look: neo\n---\n' .. FLOW,
      'look: neo',
    },
    {
      'R-294',
      'The neo-dark theme in an init directive',
      '%%{init: {"theme": "neo-dark"}}%%\n' .. FLOW,
      'theme: neo-dark',
    },
    {
      'R-294',
      'The redux-color theme in front matter',
      '---\nconfig:\n  theme: redux-color\n---\n' .. FLOW,
      'theme: redux-color',
    },
  }
  for _, u in ipairs(unsupported) do
    QA.check(
      { u[1], 'R-296' },
      u[2] .. ' gives a failure box with the exact reason unsupported: <feature>',
      function()
        local rows, text = run(u[3])
        a.falsy(placed(text), 'no image is placed')
        a.contains('unsupported: ' .. u[4], inner_rows(rows), 'the reason row')
      end
    )
  end

  for _, t in ipairs({
    { 'zenuml', 'zenuml\n  title Tea\n  Kettle.boil()' },
    { 'info', 'info' },
    { 'agentflow', 'agentflow-beta\n  a --> b' },
    { 'railroad', 'railroad-beta\n  tea = "cup" ;' },
    { 'railroad EBNF', 'railroad-ebnf-beta\n  tea = "cup" ;', 'R-298' },
    { 'railroad ABNF', 'railroad-abnf-beta\n  tea = "cup"', 'R-298' },
    { 'railroad PEG', 'railroad-peg-beta\n  tea <- "cup"', 'R-298' },
  }) do
    QA.check(
      { 'R-113', t[3] or 'R-176' },
      'The ' .. t[1] .. ' diagram type shows a failure box with the reason unsupported diagram type',
      function()
        local rows, text = run(t[2])
        a.falsy(placed(text), 'no image is placed')
        a.contains('unsupported diagram type', inner_rows(rows), 'the exact reason row')
        a.not_contains('Parse error', text)
      end
    )
  end

  QA.check(
    'R-113',
    'A diagram type outside the delivered phases shows unsupported diagram type',
    function()
      local phase = QA.delivered_phase()
      QA.need(phase < 5, 'a diagram phase that is not delivered yet')
      local name = U.phases[phase + 1][1]
      local _, text = run(U.source('types/' .. name))
      a.contains('unsupported diagram type', text)
    end
  )

  QA.check(
    'R-116',
    'Saving again without a change to the diagram does not render it again',
    { severity = 'medium' },
    function()
      local _, text = run(FLOW)
      a.truthy(placed(text), 'the diagram is placed')
      local before = #U.transmitted_pngs(ctx)
      local files = vim.fn.globpath(ctx.dir .. '/cache', '**/*.png', false, true)
      U.save(ctx)
      U.settle(ctx)
      local after_files = vim.fn.globpath(ctx.dir .. '/cache', '**/*.png', false, true)
      a.eq(#files, #after_files, 'no new cache entry')
      a.le(before, #U.transmitted_pngs(ctx), 'no second transmit')
    end
  )

  QA.check(
    'R-116',
    'A mermaid render is cached in the mermaid subdirectory of the cache',
    function()
      local _, text = run(FLOW)
      a.truthy(placed(text), 'the diagram is placed')
      local files = vim.fn.globpath(ctx.dir .. '/cache/mermaid', '**/*.png', false, true)
      a.ge(1, #files, 'a PNG under cache/mermaid')
    end
  )

  QA.check('R-116', 'Changing the diagram source and saving renders a new cache entry', function()
    local _, text = run(FLOW)
    a.truthy(placed(text), 'the diagram is placed')
    local lines = U.doc(FLOW:gsub('Pour the water', 'Pour the tea'))
    U.save(ctx, lines)
    QA.wait(15000, function()
      return #vim.fn.globpath(ctx.dir .. '/cache/mermaid', '**/*.png', false, true) >= 2
    end, 20)
    a.eq(
      2,
      #vim.fn.globpath(ctx.dir .. '/cache/mermaid', '**/*.png', false, true),
      'two cache entries'
    )
  end)

  local function key_of(source, width, theme)
    return vim.fn.sha256(vim.fn.sha256(source) .. '|' .. width .. '|' .. theme .. '|12.2.0')
  end

  local function cache_names()
    return table.concat(vim.fn.globpath(ctx.dir .. '/cache/mermaid', '**/*.png', false, true), '\n')
  end

  QA.check(
    { 'R-213', 'R-330', 'R-331' },
    'The plugin keys a mermaid render by the R-213 key string over the fence body joined by line feeds',
    function()
      local _, text = run(FLOW, { setup = { theme = 'light' } })
      a.truthy(placed(text), 'the diagram is placed')
      local body = table.concat(vim.split((FLOW:gsub('\n+$', '')), '\n', { plain = true }), '\n')
      a.contains(key_of(body, text_width_cells() * U.CELL_W, 'default'), cache_names())
    end
  )

  QA.check(
    { 'R-213', 'R-330' },
    'The key of a fence body with blank and indented lines covers those lines without a final line feed',
    function()
      local body = 'flowchart TD\n\n    Kettle --> Cup\n\n  Cup --> Saucer'
      local _, text = run(body, { setup = { theme = 'dark' } })
      a.truthy(placed(text), 'the diagram is placed')
      local names = cache_names()
      a.contains(
        key_of(body, text_width_cells() * U.CELL_W, 'dark'),
        names,
        'key without a final line feed'
      )
      a.not_contains(key_of(body .. '\n', text_width_cells() * U.CELL_W, 'dark'), names)
    end
  )

  QA.check(
    'R-169',
    'While a mermaid diagram renders, a loading box shows the diagram type and the word loading',
    { severity = 'medium' },
    function()
      ctx = U.open(U.doc(U.source('types/c4')))
      QA.need(ctx.pbuf ~= nil, 'a preview buffer')
      local seen
      QA.wait(5000, function()
        local t = U.rows_text(U.rows(ctx))
        if t:find('loading', 1, true) then
          seen = t
          return true
        end
        return t:find(U.PLACEHOLDER, 1, true) ~= nil
      end, 1)
      QA.need(seen ~= nil, 'a loading state that lasts long enough to observe')
      a.matches('[Cc]4', seen, 'the loading box names the diagram type')
    end
  )
end)
