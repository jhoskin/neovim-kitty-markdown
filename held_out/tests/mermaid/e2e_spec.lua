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
    'R-180',
    'A mermaid diagram narrower than the target width is placed at its natural size',
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

  QA.check('R-234', 'An unknown theme name in the diagram is ignored in the preview', function()
    local src = '%%{init: {"theme": "midnight-plum"}}%%\n' .. FLOW
    local _, text = run(src, { setup = { theme = 'light', mermaid = { theme = 'forest' } } })
    a.truthy(placed(text), 'the diagram is placed, not a failure box')
    local forest = U.cli(FLOW, text_width_cells() * U.CELL_W, 'forest', 'transparent')
    a.ge(0.999, U.same_fraction(last_png(), forest), 'the configured theme still applies')
  end)

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
      'A diagram with the handDrawn look shows an unsupported failure box that names the feature',
      '%%{init: {"look": "handDrawn"}}%%\n' .. FLOW,
      'handdrawn',
    },
    {
      'R-112',
      'A diagram with ELK layout shows an unsupported failure box that names the feature',
      '---\nconfig:\n  layout: elk\n---\n' .. FLOW,
      'elk',
    },
    {
      'R-112',
      'A diagram with an icon pack shows an unsupported failure box that names the feature',
      'flowchart LR\n  A@{ icon: "fa:cog", form: "square", label: "Gear" } --> B',
      'icon',
    },
    {
      'R-112',
      'A diagram with an image node shows an unsupported failure box that names the feature',
      'flowchart LR\n  A@{ img: "https://example.invalid/a.png", label: "Pic", pos: "t" } --> B',
      'im',
    },
    {
      'R-175',
      'A diagram with the neo look shows an unsupported failure box that names the feature',
      '%%{init: {"look": "neo"}}%%\n' .. FLOW,
      'neo',
    },
  }
  for _, u in ipairs(unsupported) do
    QA.check(u[1], u[2], function()
      local _, text = run(u[3])
      a.falsy(placed(text), 'no image is placed')
      local line
      for l in text:gmatch('[^\n]+') do
        if l:find('unsupported: ', 1, true) then
          line = l
        end
      end
      a.not_nil(line, 'a reason that starts with "unsupported: "')
      a.contains(u[4], line:lower(), 'the reason names the feature')
    end)
  end

  for _, t in ipairs({
    { 'zenuml', 'zenuml\n  title Tea\n  Kettle.boil()' },
    { 'info', 'info' },
    { 'agentflow', 'agentflow-beta\n  a --> b' },
    { 'railroad', 'railroad-beta\n  tea = "cup" ;' },
  }) do
    QA.check(
      { 'R-113', 'R-176' },
      'The ' .. t[1] .. ' diagram type shows a failure box with the reason unsupported diagram type',
      function()
        local _, text = run(t[2])
        a.falsy(placed(text), 'no image is placed')
        a.contains('unsupported diagram type', text)
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
