-- Helper-level mermaid checks: mermaid.render over a JSON-lines session and
-- `kitty-markdown-helper render --mermaid` on the command line.

local QA = require('qa')
local U = require('mermaid_util')
local a = QA.assert

local FLOW = U.source('types/flowchart')
local WIDE = U.source('config/flow_plain')
local PARSE_FIX =
  'mermaid parses the diagram text with one newline appended (see Diagram.fromText in the pinned mermaid), which sets the line, the excerpt, and the caret for errors at the end of the text'

local function ok_render(source, width, theme, background)
  local res, err = U.render(source, width, theme, background)
  a.is_nil(err, 'no error for a valid diagram' .. (err and (': ' .. tostring(err.message)) or ''))
  a.not_nil(res, 'a result')
  return res
end

-- Renders through the CLI and requires success. Returns the PNG path.
local function ok_cli(source, width, theme)
  local out, res = U.cli(source, width, theme, 'transparent')
  a.eq(0, res.code, 'CLI exit code (' .. tostring(res.stderr) .. ')')
  a.not_nil(QA.png_size(out), 'the CLI wrote a PNG')
  return out
end

-- Renders through the CLI and requires failure. Returns the stderr text.
local function failing_cli(source)
  local out, res = U.cli(source, 960)
  a.ne(0, res.code, 'CLI exit code for a diagram that does not render')
  a.is_nil(QA.png_size(out), 'no PNG for a diagram that does not render')
  return (res.stderr or '') .. (res.stdout or '')
end

local function tol(v)
  return math.max(2, v * 0.01)
end

local function err_text(err)
  return tostring(err and err.message or '') .. ' ' .. tostring(err and err.code or '')
end

QA.describe('mermaid: helper', function()
  QA.after_each(function()
    U.close_session()
  end)

  -- Every delivered type renders (R-105 to R-109) --------------------------------
  for phase, names in ipairs(U.phases) do
    local req = U.requirement_of_phase[phase]
    for _, name in ipairs(names) do
      QA.check(
        req,
        'The helper renders a ' .. name .. ' diagram to a PNG file',
        { phase = phase },
        function()
          local res = ok_render(U.source('types/' .. name), 960, 'default')
          a.eq(1, vim.fn.filereadable(res.path), 'the result path is a readable file')
          local w, h = QA.png_size(res.path)
          a.not_nil(w, 'the file is a PNG')
          a.eq(res.width_px, w, 'reported width matches the PNG')
          a.eq(res.height_px, h, 'reported height matches the PNG')
          a.ge(1, w)
          a.ge(1, h)
          a.le(960, w, 'not wider than the target width')
        end
      )

      QA.check(
        req,
        'The CLI renders a ' .. name .. ' diagram and exits 0',
        { phase = phase, severity = 'medium' },
        function()
          ok_cli(U.source('types/' .. name), 960, 'dark')
        end
      )

      if name ~= 'gantt' then
        QA.check(
          { 'R-098', 'R-180', req },
          'A '
            .. name
            .. ' diagram whose natural width fits the target width renders at its natural size',
          { phase = phase },
          function()
            local nat = U.oracle_size('types/' .. name)
            local res = ok_render(U.source('types/' .. name), 2400, 'default')
            a.near(nat[1], res.width_px, tol(nat[1]), 'natural width')
            a.near(nat[2], res.height_px, tol(nat[2]), 'natural height')
          end
        )
      end
    end
  end

  -- Target width (R-098) ------------------------------------------------------
  for _, case in ipairs({
    { 'config/flow_plain', 400 },
    { 'types/journey', 600 },
    { 'types/c4', 500 },
    { 'types/sequence', 333 },
  }) do
    local kind = case[1]:match('/(%a+)')
    if kind == 'flow' then
      kind = 'flowchart'
    end
    QA.check(
      'R-098',
      'A '
        .. kind
        .. ' diagram wider than the target width renders at the target width with its aspect ratio',
      function()
        local nat = U.oracle_size(case[1])
        local res = ok_render(U.source(case[1]), case[2], 'default')
        a.near(case[2], res.width_px, 1, 'width equals the target width')
        a.near(nat[2] * case[2] / nat[1], res.height_px, 2, 'height keeps the aspect ratio')
      end
    )
  end

  QA.check('R-098', 'The CLI renders a wide diagram at the requested width', function()
    local out = ok_cli(WIDE, 300, 'default')
    local w = QA.png_size(out)
    a.near(300, w, 1)
  end)

  QA.check(
    'R-098',
    'The same diagram at two target widths below its natural width gives two widths',
    { severity = 'medium' },
    function()
      local r1 = ok_render(WIDE, 320)
      local r2 = ok_render(WIDE, 480)
      a.near(320, r1.width_px, 1)
      a.near(480, r2.width_px, 1)
    end
  )

  -- Diagram width (R-114) -------------------------------------------------------
  for _, case in ipairs({ 'config/xychart_width', 'config/quadrant_width', 'config/sankey_width' }) do
    QA.check(
      'R-114',
      'A '
        .. case:match('/(%a+)')
        .. ' diagram that sets its own width smaller than the target width renders at that width',
      function()
        local nat = U.oracle_size(case)
        local res = ok_render(U.source(case), 960)
        a.near(nat[1], res.width_px, 2, 'the diagram width')
        a.near(nat[2], res.height_px, 2, 'the diagram height')
      end
    )
  end

  QA.check(
    'R-114',
    'A diagram that sets its own width wider than the target width still fits the target width',
    { severity = 'medium' },
    function()
      local res = ok_render(U.source('config/xychart_width'), 300)
      a.near(300, res.width_px, 1)
    end
  )

  -- Transparent background (R-229) -------------------------------------------
  QA.check(
    'R-229',
    'A transparent mermaid render has transparent pixels around the diagram',
    function()
      local res = ok_render(FLOW, 960, 'dark', 'transparent')
      local info = U.png_info(res.path)
      for i, c in ipairs(info.corners) do
        a.eq(0, c[4], 'corner ' .. i .. ' alpha')
      end
      a.ge(1, info.opaque, 'the diagram itself is drawn')
    end
  )

  -- Themes (R-104) --------------------------------------------------------------
  local themes = { 'default', 'dark', 'neutral', 'forest', 'base' }
  QA.check(
    'R-104',
    'The helper renders each of the five built-in themes with distinct colors',
    function()
      local paths = {}
      for _, t in ipairs(themes) do
        paths[t] = ok_render(FLOW, 960, t).path
      end
      for i = 1, #themes do
        for j = i + 1, #themes do
          a.le(
            0.99,
            U.same_fraction(paths[themes[i]], paths[themes[j]]),
            themes[i] .. ' and ' .. themes[j] .. ' differ'
          )
        end
      end
    end
  )

  for _, t in ipairs(themes) do
    QA.check(
      'R-104',
      'The '
        .. t
        .. ' theme named in the diagram gives the same picture as the '
        .. t
        .. ' theme selected by the plugin',
      function()
        local direct = ok_render(FLOW, 960, t).path
        local other = t == 'default' and 'dark' or 'default'
        local via = ok_render('%%{init: {"theme": "' .. t .. '"}}%%\n' .. FLOW, 960, other).path
        a.ge(0.999, U.same_fraction(direct, via), t .. ' through the directive')
      end
    )
  end

  -- Directives and front matter (R-102, R-103, R-174) ----------------------------
  QA.check(
    { 'R-102', 'R-174' },
    'An init directive theme overrides the theme the plugin selects',
    function()
      local via = ok_render('%%{init: {"theme": "forest"}}%%\n' .. FLOW, 960, 'dark').path
      local forest = ok_render(FLOW, 960, 'forest').path
      a.ge(0.999, U.same_fraction(forest, via))
    end
  )

  QA.check(
    { 'R-103', 'R-174' },
    'A front matter theme overrides the theme the plugin selects',
    function()
      local via = ok_render('---\nconfig:\n  theme: forest\n---\n' .. FLOW, 960, 'dark').path
      local forest = ok_render(FLOW, 960, 'forest').path
      a.ge(0.999, U.same_fraction(forest, via))
    end
  )

  QA.check('R-102', 'An init directive with themeVariables changes the colors it names', function()
    local res = ok_render(
      '%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#ff0000"}}}%%\n' .. FLOW,
      960,
      'default'
    )
    a.ge(5000, U.count_near(res.path, '#ff0000', 12), 'red node fill pixels')
  end)

  QA.check('R-103', 'Front matter themeVariables change the colors they name', function()
    local src = '---\nconfig:\n  theme: base\n  themeVariables:\n    primaryColor: "#00ff00"\n---\n'
      .. FLOW
    local res = ok_render(src, 960, 'default')
    a.ge(5000, U.count_near(res.path, '#00ff00', 12), 'green node fill pixels')
  end)

  QA.check('R-102', 'An init directive fontFamily changes the font of the labels', function()
    local nat = U.oracle_size('config/flow_font_verdana')
    local res = ok_render(U.source('config/flow_font_verdana'), 2400)
    local plain = ok_render(WIDE, 2400)
    a.near(nat[1], res.width_px, 2, 'width with the requested font')
    a.ne(plain.width_px, res.width_px, 'the label widths change with the font')
  end)

  QA.check(
    'R-102',
    'An init directive with the classic look and dagre layout renders like the default',
    function()
      local res = ok_render(U.source('config/flow_classic_explicit'), 2400)
      local plain = ok_render(WIDE, 2400)
      a.ge(0.999, U.same_fraction(plain.path, res.path))
    end
  )

  for _, case in ipairs({
    { 'config/seq_mirror_init', 'R-102' },
    { 'config/flow_spacing_init', 'R-102' },
    { 'config/seq_mirror_front', 'R-103' },
    { 'config/flow_spacing_front', 'R-103' },
  }) do
    local how = case[2] == 'R-102' and 'An init directive' or 'Front matter config'
    QA.check(
      case[2],
      how
        .. ' with a '
        .. (case[1]:find('/seq_') and 'sequence' or 'flowchart')
        .. ' diagram setting changes the layout as the oracle does',
      function()
        local nat = U.oracle_size(case[1])
        local res = ok_render(U.source(case[1]), 2400)
        a.near(nat[1], res.width_px, 2, 'width')
        a.near(nat[2], res.height_px, 2, 'height')
      end
    )
  end

  QA.check('R-103', 'Front matter with a title and no config renders with the title', function()
    local nat = U.oracle_size('config/flow_front_title')
    local res = ok_render(U.source('config/flow_front_title'), 2400)
    a.near(nat[2], res.height_px, 2, 'height includes the title')
  end)

  QA.check(
    'R-103',
    'Front matter config with the handDrawn look is refused like the init directive',
    function()
      local _, err = U.render('---\nconfig:\n  look: handDrawn\n---\n' .. FLOW, 960)
      a.not_nil(err, 'an error')
      a.contains('handdrawn', err_text(err):lower())
    end
  )

  QA.check(
    'R-174',
    'A diagram without look or layout settings renders with the classic dagre base config',
    function()
      local res, err = U.render(WIDE, 2400)
      a.is_nil(err, 'the mermaid 12 defaults (ELK, neo) are not applied')
      local nat = U.oracle_size('config/flow_plain')
      a.near(nat[1], res.width_px, 2, 'the dagre layout size')
    end
  )

  QA.check(
    'R-174',
    'Front matter config applies over the base theme for every theme the plugin can select',
    { severity = 'medium' },
    function()
      local src = '---\nconfig:\n  theme: neutral\n---\n' .. FLOW
      local neutral = ok_render(FLOW, 960, 'neutral').path
      for _, t in ipairs({ 'default', 'dark', 'forest', 'base' }) do
        a.ge(0.999, U.same_fraction(neutral, ok_render(src, 960, t).path), 'base theme ' .. t)
      end
    end
  )

  -- Unknown theme (R-234) and themeVariables merge (R-235) ---------------------------
  QA.check(
    'R-234',
    'An unknown theme name in an init directive is ignored and the selected theme applies',
    function()
      local res, err = U.render('%%{init: {"theme": "midnight-plum"}}%%\n' .. FLOW, 960, 'dark')
      a.is_nil(err, 'no error for an unknown theme name')
      a.ge(
        0.999,
        U.same_fraction(ok_render(FLOW, 960, 'dark').path, res.path),
        'the dark theme still applies'
      )
    end
  )

  QA.check(
    'R-234',
    'An unknown theme name in front matter is ignored and the selected theme applies',
    function()
      local res, err =
        U.render('---\nconfig:\n  theme: midnight-plum\n---\n' .. FLOW, 960, 'neutral')
      a.is_nil(err, 'no error for an unknown theme name')
      a.ge(
        0.999,
        U.same_fraction(ok_render(FLOW, 960, 'neutral').path, res.path),
        'the neutral theme still applies'
      )
    end
  )

  QA.check(
    'R-234',
    'An unknown theme name does not stop the other directive settings from applying',
    { severity = 'medium' },
    function()
      local src = '%%{init: {"theme": "midnight-plum", "sequence": {"mirrorActors": false}}}%%\n'
        .. U.source('config/seq_plain')
      local res, err = U.render(src, 2400, 'default')
      a.is_nil(err, 'no error')
      a.near(
        U.oracle_size('config/seq_mirror_init')[2],
        res.height_px,
        2,
        'the sequence setting applies'
      )
    end
  )

  QA.check(
    'R-235',
    'themeVariables without a theme merge over the current theme and leave its other colors',
    function()
      local base = ok_render(FLOW, 960, 'dark').path
      local res =
        ok_render('%%{init: {"themeVariables": {"mainBkg": "#ff0000"}}}%%\n' .. FLOW, 960, 'dark')
      local m = U.masked(base, res.path, '#ff0000', 10, 3)
      a.truthy(m.same_size, 'same size')
      a.ge(5000, m.mask, 'the node fill takes the new color')
      a.ge(0.9 * m.mask, m.mask_top_base, 'the new color replaces one color of the theme')
      a.ge(0.98 * m.outside, m.outside_same, 'the rest of the dark theme stays')
    end
  )

  QA.check(
    { 'R-235', 'R-299' },
    'themeVariables without a theme do not recompute the colors derived from them',
    function()
      local base = ok_render(FLOW, 960, 'default').path
      local res = ok_render(
        '%%{init: {"themeVariables": {"primaryColor": "#ff0000"}}}%%\n' .. FLOW,
        960,
        'default'
      )
      a.ge(0.999, U.same_fraction(base, res.path), 'same picture as the theme without the override')
    end
  )

  QA.check(
    'R-299',
    'On the dark theme a merged theme variable leaves the colors derived from other variables',
    function()
      local base = ok_render(FLOW, 960, 'dark').path
      local res = ok_render(
        '%%{init: {"themeVariables": {"primaryColor": "#ff0000"}}}%%\n' .. FLOW,
        960,
        'dark'
      )
      a.ge(0.999, U.same_fraction(base, res.path), 'same picture as the dark theme')
    end
  )

  -- Init directive over front matter (R-295) -----------------------------------
  QA.check('R-295', 'An init directive theme wins over a front matter theme', function()
    local src = '---\nconfig:\n  theme: forest\n---\n%%{init: {"theme": "neutral"}}%%\n' .. FLOW
    local got = ok_render(src, 960, 'dark').path
    a.ge(0.999, U.same_fraction(ok_render(FLOW, 960, 'neutral').path, got), 'the neutral theme')
  end)

  QA.check(
    'R-295',
    'An init directive diagram setting wins over the same setting in front matter',
    function()
      local src = '---\nconfig:\n  sequence:\n    mirrorActors: true\n---\n'
        .. '%%{init: {"sequence": {"mirrorActors": false}}}%%\n'
        .. U.source('config/seq_plain')
      local res = ok_render(src, 2400)
      a.near(
        U.oracle_size('config/seq_mirror_init')[2],
        res.height_px,
        2,
        'height without mirrored actors'
      )
    end
  )

  QA.check(
    'R-295',
    'Front matter keys that the init directive does not set still apply',
    { severity = 'medium' },
    function()
      local src = '---\nconfig:\n  theme: forest\n  sequence:\n    mirrorActors: false\n---\n'
        .. '%%{init: {"themeVariables": {}}}%%\n'
        .. U.source('config/seq_plain')
      local res = ok_render(src, 2400)
      a.near(
        U.oracle_size('config/seq_mirror_init')[2],
        res.height_px,
        2,
        'the front matter setting'
      )
    end
  )

  -- Gantt width (R-293) -----------------------------------------------------------
  local GANTT = U.source('types/gantt')
  for _, w in ipairs({ 2400, 700 }) do
    QA.check(
      { 'R-293', 'R-106' },
      'A gantt diagram without useWidth renders at the target width ('
        .. (w > 960 and 'a wide target' or 'a narrow target')
        .. ')',
      { phase = 2 },
      function()
        local res = ok_render(GANTT, w)
        a.eq(w, res.width_px, 'gantt width')
      end
    )
  end

  QA.check(
    { 'R-293', 'R-098' },
    'A gantt diagram with a useWidth below the target width renders at useWidth',
    { phase = 2 },
    function()
      local res = ok_render('%%{init: {"gantt": {"useWidth": 500}}}%%\n' .. GANTT, 960)
      a.eq(500, res.width_px)
    end
  )

  QA.check(
    { 'R-293', 'R-098' },
    'A gantt diagram with a useWidth above the target width renders at the target width',
    { phase = 2 },
    function()
      local res = ok_render('%%{init: {"gantt": {"useWidth": 1200}}}%%\n' .. GANTT, 600)
      a.eq(600, res.width_px)
    end
  )

  -- Fonts (R-115, R-179) --------------------------------------------------------
  local function have(path)
    return vim.uv.fs_stat(path) ~= nil
  end
  local trebuchet = have('/System/Library/Fonts/Supplemental/Trebuchet MS.ttf')
    or have('/Library/Fonts/Trebuchet MS.ttf')
  local verdana = have('/System/Library/Fonts/Supplemental/Verdana.ttf')
    or have('/Library/Fonts/Verdana.ttf')

  QA.check(
    'R-115',
    'The default font chain gives the same picture as the chain written out explicitly',
    function()
      local plain = ok_render(WIDE, 2400).path
      local chain = ok_render(U.source('config/flow_font_chain'), 2400).path
      a.ge(0.999, U.same_fraction(plain, chain))
    end
  )

  QA.check(
    'R-115',
    'The default font chain resolves to the first installed family, not a later one',
    function()
      QA.need(trebuchet and verdana, 'Trebuchet MS and Verdana installed')
      local plain = ok_render(WIDE, 2400).path
      local verd = ok_render(U.source('config/flow_font_verdana'), 2400).path
      a.le(0.99, U.same_fraction(plain, verd), 'trebuchet ms is used before verdana')
      local nat = U.oracle_size('config/flow_plain')
      local w = QA.png_size(plain)
      a.near(nat[1], w, 2, 'the label widths of the first family')
    end
  )

  QA.check(
    'R-115',
    'A missing family in the font chain falls through to the next family in order',
    function()
      QA.need(verdana, 'Verdana installed')
      local src = '%%{init: {"fontFamily": "no-such-family-qa, verdana, arial, sans-serif"}}%%\n'
        .. WIDE
      local fell = ok_render(src, 2400).path
      local verd = ok_render(U.source('config/flow_font_verdana'), 2400).path
      a.ge(0.999, U.same_fraction(verd, fell), 'the chain resolves to verdana')
    end
  )

  local function system_open_sans()
    local res = QA.sh({ 'fc-list', ':family=Open Sans', 'file' })
    if res.code == 0 and vim.trim(res.stdout or '') ~= '' then
      return true
    end
    for _, dir in ipairs({
      vim.env.HOME .. '/Library/Fonts',
      '/Library/Fonts',
      '/System/Library/Fonts',
      '/usr/share/fonts',
      vim.env.HOME .. '/.local/share/fonts',
    }) do
      if #vim.fn.globpath(dir, '**/OpenSans*', false, true) > 0 then
        return true
      end
    end
    return false
  end

  QA.check(
    { 'R-179', 'R-345' },
    'The bundled Open Sans faces are used for the Open Sans family also when the system has an Open Sans font',
    function()
      QA.need(
        vim.fn.executable('fc-list') == 1 or vim.fn.has('mac') == 1,
        'a way to list system fonts'
      )
      QA.need(system_open_sans(), 'a system Open Sans font (R-345)')
      local nat = U.oracle_size('config/flow_font_opensans')
      local os_path = ok_render(U.source('config/flow_font_opensans'), 2400).path
      local missing =
        ok_render('%%{init: {"fontFamily": "no-such-family-qa"}}%%\n' .. WIDE, 2400).path
      local plain = ok_render(WIDE, 2400).path
      a.le(0.99, U.same_fraction(os_path, missing), 'Open Sans is not the fallback font')
      a.le(0.99, U.same_fraction(os_path, plain), 'Open Sans is not the default chain')
      local w = QA.png_size(os_path)
      a.near(nat[1], w, 1, 'the label widths of the oracle with Open Sans')
    end
  )

  -- Parse errors (R-111, R-177, R-178) -------------------------------------------
  local phase_of = { er = 2 }
  local what = {
    class_unclosed_body = 'a class diagram error at the end of the text',
    er_missing_colon = 'an ER diagram error at a missing token',
    flowchart_double_arrow = 'a flowchart error at an unexpected token inside the text',
    flowchart_unclosed_label = 'a flowchart node shape left open at the end of the text',
    flowchart_unclosed_pipe = 'a flowchart link label left open at the end of the text',
    sequence_blank_lines = 'a sequence diagram error on a line that follows blank lines',
    sequence_missing_actor = 'a sequence diagram error at the end of the text',
    state_missing_target = 'a state diagram error at a missing token',
  }
  for _, name in ipairs(U.parse_cases()) do
    local kind = name:match('^(%a+)_')
    local phase = phase_of[kind] or 1
    local desc = what[name] or ('a ' .. kind .. ' diagram')
    QA.check(
      { 'R-111', 'R-177' },
      'The parse error for ' .. desc .. ' reports the line the oracle reports',
      { phase = phase, fix = PARSE_FIX },
      function()
        local oracle, n = U.oracle_error(name)
        local res, err = U.render(U.source('parse/' .. name), 960)
        a.is_nil(res, 'no image')
        a.not_nil(err, 'an error')
        local msg = tostring(err.message)
        a.eq(oracle[1], (msg:match('^[^\n]*')), 'first line of the error')
        if type(err.data) == 'table' and err.data.line ~= nil then
          a.eq(n, tonumber(err.data.line), 'the line in the error data')
        end
      end
    )

    QA.check(
      'R-178',
      'The parse error message for '
        .. desc
        .. ' has the oracle excerpt, caret line, and expected tokens',
      { phase = phase, severity = 'medium', fix = PARSE_FIX },
      function()
        local oracle = U.oracle_error(name)
        local _, err = U.render(U.source('parse/' .. name), 960)
        a.not_nil(err, 'an error')
        a.eq(
          table.concat(oracle, '\n'),
          (tostring(err.message):gsub('\n+$', '')),
          'the oracle message'
        )
      end
    )
  end

  QA.check(
    'R-177',
    'The CLI reports a parse error with the oracle line and a nonzero exit',
    function()
      local oracle = U.oracle_error('sequence_missing_actor')
      local text = failing_cli(U.source('parse/sequence_missing_actor'))
      a.contains(oracle[1], text)
    end
  )

  -- No diagram type and bad front matter (R-230 to R-233) ---------------------------
  QA.check('R-230', 'Text that matches no diagram type is an error on line 1', function()
    local res, err = U.render('teapots are lovely', 960)
    a.is_nil(res)
    a.not_nil(err)
    if type(err.data) == 'table' and err.data.line ~= nil then
      a.eq(1, tonumber(err.data.line), 'the line in the error data')
    end
  end)

  QA.check(
    'R-230',
    'A diagram keyword that is only a prefix of a real type matches no diagram type',
    { severity = 'medium' },
    function()
      local _, err = U.render('flow LR\n  A --> B', 960)
      a.not_nil(err)
      a.contains('No diagram type detected', tostring(err.message))
    end
  )

  QA.check(
    'R-231',
    'The no-diagram-type error carries the mermaid message with the diagram text',
    function()
      local _, err = U.render('teapots are lovely\nand so are kettles', 960)
      a.not_nil(err)
      a.contains(
        'No diagram type detected matching given configuration for text: teapots are lovely',
        tostring(err.message)
      )
    end
  )

  QA.check(
    'R-231',
    'The CLI prints the no-diagram-type message and exits nonzero',
    { severity = 'medium' },
    function()
      local text = failing_cli('teapots are lovely')
      a.contains(
        'No diagram type detected matching given configuration for text: teapots are lovely',
        text
      )
    end
  )

  QA.check('R-232', 'Front matter that does not close is an error on line 1', function()
    local res, err = U.render('---\ntitle: Tea\nflowchart LR\n  A --> B', 960)
    a.is_nil(res)
    a.not_nil(err)
    if type(err.data) == 'table' and err.data.line ~= nil then
      a.eq(1, tonumber(err.data.line), 'the line in the error data')
    end
  end)

  QA.check(
    'R-232',
    'Front matter that never closes at the end of the diagram is an error',
    { severity = 'medium' },
    function()
      local res, err = U.render('---\nconfig:\n  theme: dark', 960)
      a.is_nil(res)
      a.not_nil(err)
    end
  )

  QA.check('R-233', 'Front matter that is not valid YAML is an error on line 1', function()
    local res, err = U.render('---\nconfig: {theme: [forest\n---\nflowchart LR\n  A --> B', 960)
    a.is_nil(res)
    a.not_nil(err)
    if type(err.data) == 'table' and err.data.line ~= nil then
      a.eq(1, tonumber(err.data.line), 'the line in the error data')
    end
  end)

  QA.check(
    'R-233',
    'Front matter with a tab-indented key is not valid YAML and is an error',
    { severity = 'medium' },
    function()
      local res, err = U.render('---\nconfig:\n\ttheme: dark\n---\nflowchart LR\n  A --> B', 960)
      a.is_nil(res)
      a.not_nil(err)
    end
  )

  -- Unsupported features (R-112, R-175) ------------------------------------------------
  local features = {
    {
      'R-112',
      'The handDrawn look in an init directive is refused as unsupported',
      '%%{init: {"look": "handDrawn"}}%%\n' .. FLOW,
      'look: handDrawn',
    },
    {
      'R-112',
      'ELK layout in an init directive is refused as unsupported',
      '%%{init: {"layout": "elk"}}%%\n' .. FLOW,
      'layout: elk',
    },
    {
      'R-112',
      'ELK layout in front matter is refused as unsupported',
      '---\nconfig:\n  layout: elk\n---\n' .. FLOW,
      'layout: elk',
    },
    {
      'R-112',
      'The flowchart-elk diagram header is refused as unsupported ELK layout',
      'flowchart-elk TD\n  A --> B',
      'layout: elk',
    },
    {
      'R-112',
      'A node with an icon from an icon pack is refused as unsupported',
      'flowchart TD\n  A@{ icon: "fa:mug-hot", form: "circle", label: "Tea" } --> B',
      'icon pack',
    },
    {
      'R-112',
      'An architecture service with an icon pack icon is refused as unsupported',
      'architecture-beta\n  service fn(logos:aws-lambda)[Function]',
      'icon pack',
    },
    {
      'R-112',
      'An image node is refused as unsupported',
      'flowchart TD\n  A@{ img: "https://example.invalid/tea.png", label: "Tea", pos: "b", w: 60, h: 60 } --> B',
      'image node',
    },
    {
      'R-175',
      'The neo look in an init directive is refused as unsupported',
      '%%{init: {"look": "neo"}}%%\n' .. FLOW,
      'look: neo',
    },
    {
      'R-175',
      'The neo look in front matter is refused as unsupported',
      '---\nconfig:\n  look: neo\n---\n' .. FLOW,
      'look: neo',
    },
    {
      'R-175',
      'The handDrawn look in front matter is refused as unsupported',
      '---\nconfig:\n  look: handDrawn\n---\n' .. FLOW,
      'look: handDrawn',
    },
  }
  for i, name in ipairs({
    'neo',
    'neo-dark',
    'redux',
    'redux-dark',
    'redux-color',
    'redux-dark-color',
  }) do
    local src = i % 2 == 1 and ('%%{init: {"theme": "' .. name .. '"}}%%\n' .. FLOW)
      or ('---\nconfig:\n  theme: ' .. name .. '\n---\n' .. FLOW)
    features[#features + 1] = {
      'R-294',
      'The mermaid 12 theme ' .. name .. ' is refused as unsupported with its theme name',
      src,
      'theme: ' .. name,
    }
  end
  for _, f in ipairs(features) do
    QA.check({ f[1], 'R-296' }, f[2], function()
      local res, err = U.render(f[3], 960)
      a.is_nil(res, 'no image')
      a.not_nil(err, 'an error')
      local t = err_text(err)
      a.contains('unsupported', t:lower())
      a.contains(f[4], t, 'the error names the feature in the form of R-296')
      a.not_contains('parse error', t:lower(), 'not a parse error')
    end)
  end

  -- When the helper refuses one of these, the feature has the R-296 form.
  local conditional = {
    {
      'architecture randomize',
      '---\nconfig:\n  architecture:\n    randomize: true\n---\narchitecture-beta\n  service a(server)[A]',
      '^randomize$',
    },
    {
      'a sequence participant type',
      'sequenceDiagram\n  participant Ann@{ "type": "boundary" }\n  Ann->>Ann: hi',
      '^participant type boundary$',
    },
    {
      'a top-down timeline',
      'timeline TD\n  title Week\n  Monday : Bread',
      '^timeline direction TD$',
    },
  }
  for _, c in ipairs(conditional) do
    QA.check(
      'R-296',
      'If the helper refuses ' .. c[1] .. ', the unsupported feature has the form R-296 gives',
      { severity = 'medium' },
      function()
        local _, err = U.render(c[2], 960)
        if not err then
          return
        end
        local msg = tostring(err.message)
        local feature = type(err.data) == 'table' and err.data.feature
          or msg:match('unsupported[^:]*: (.*)$')
        a.matches(c[3], tostring(feature), 'feature of ' .. msg)
      end
    )
  end

  QA.check(
    'R-112',
    'Architecture services with the built-in icons are not refused as an icon pack',
    { phase = 4 },
    function()
      ok_render(U.source('types/architecture'), 960)
    end
  )

  QA.check(
    'R-175',
    'The classic look and the dagre layout named in front matter are accepted',
    function()
      ok_render('---\nconfig:\n  look: classic\n  layout: dagre\n---\n' .. FLOW, 960)
    end
  )

  -- Types outside the delivered phases (R-113, R-176) ---------------------------------
  local outside = {
    { 'zenuml', 'zenuml\n  title Tea\n  Kettle.boil()' },
    { 'info', 'info' },
    { 'agentflow', 'agentflow-beta\n  a --> b' },
    { 'railroad', 'railroad-beta\n  tea = "cup" ;' },
    { 'railroad EBNF', 'railroad-ebnf-beta\n  tea = "cup" ;', 'R-298' },
    { 'railroad ABNF', 'railroad-abnf-beta\n  tea = "cup"', 'R-298' },
    { 'railroad PEG', 'railroad-peg-beta\n  tea <- "cup"', 'R-298' },
  }
  for _, o in ipairs(outside) do
    QA.check(
      { 'R-113', o[3] or 'R-176' },
      'The ' .. o[1] .. ' type is reported as an unsupported diagram type, not a parse error',
      function()
        local res, err = U.render(o[2], 960)
        a.is_nil(res, 'no image')
        a.not_nil(err, 'an error')
        local t = err_text(err):lower()
        a.truthy(
          t:find('unsupported diagram type', 1, true) or t:find('unsupported_diagram_type', 1, true),
          'unsupported diagram type: ' .. t
        )
        a.not_contains('parse error', t)
        a.not_contains('no diagram type detected', t)
      end
    )
  end

  QA.check(
    'R-113',
    'Every type of a phase after the delivered one is reported as unsupported',
    function()
      local phase = QA.delivered_phase()
      QA.need(phase < 5, 'a diagram phase that is not delivered yet')
      for p = phase + 1, 5 do
        for _, name in ipairs(U.phases[p]) do
          local _, err = U.render(U.source('types/' .. name), 960)
          a.not_nil(err, name)
          a.contains('unsupported', err_text(err):lower(), name)
        end
      end
    end
  )

  -- Cache (R-116) --------------------------------------------------------------
  QA.check(
    'R-116',
    'The same diagram, width, and theme render once and then come from the cache',
    function()
      local r1 = ok_render(FLOW, 700, 'dark')
      local r2 = ok_render(FLOW, 700, 'dark')
      a.eq(r1.path, r2.path, 'the same cache entry')
      if r2.cached ~= nil then
        a.truthy(r2.cached, 'the second render is a cache hit')
      end
    end
  )

  QA.check('R-116', 'A change in source, width, or theme gives a separate cache entry', function()
    local base = ok_render(FLOW, 700, 'dark').path
    local seen = { [base] = 'base' }
    local variants = {
      source = ok_render(FLOW .. '\n', 700, 'dark').path,
      width = ok_render(FLOW, 701, 'dark').path,
      theme = ok_render(FLOW, 700, 'forest').path,
    }
    for kind, p in pairs(variants) do
      a.is_nil(seen[p], kind .. ' shares an entry with ' .. tostring(seen[p]))
      seen[p] = kind
    end
  end)

  QA.check(
    'R-116',
    'Two target widths above the natural width still give separate cache entries',
    { severity = 'medium' },
    function()
      local p1 = ok_render(FLOW, 1500).path
      local p2 = ok_render(FLOW, 1600).path
      a.ne(p1, p2)
    end
  )

  QA.check(
    'R-116',
    'The mermaid cache entry lives in the mermaid subdirectory of the cache',
    function()
      local res = ok_render(FLOW, 700, 'dark')
      local dir = U.session().cache_dir
      a.eq(
        1,
        vim.startswith(vim.fs.normalize(res.path), vim.fs.normalize(dir) .. '/mermaid/') and 1 or 0,
        'path ' .. res.path
      )
    end
  )

  QA.check('R-116', 'A cached render survives a helper restart', { severity = 'medium' }, function()
    local s1 = QA.helper_session()
    local r1 = s1:request(
      'mermaid.render',
      { source = FLOW, width_px = 650, theme = 'neutral', background = 'transparent' }
    )
    s1:close()
    local s2 = QA.helper_session({ 'serve', '--cache-dir', s1.cache_dir })
    local r2 = s2:request(
      'mermaid.render',
      { source = FLOW, width_px = 650, theme = 'neutral', background = 'transparent' }
    )
    s2:close()
    a.not_nil(r1 and r1.result, 'first render')
    a.not_nil(r2 and r2.result, 'second render')
    a.eq(r1.result.path, r2.result.path)
    if r2.result.cached ~= nil then
      a.truthy(r2.result.cached, 'served from the cache')
    end
  end)
  -- Key string (R-213, R-330, R-331) and the R-231 text (R-297) ------------------
  local function key_of(source, width, theme, version)
    return vim.fn.sha256(vim.fn.sha256(source) .. '|' .. width .. '|' .. theme .. '|' .. version)
  end

  QA.check(
    { 'R-213', 'R-331' },
    'The mermaid cache entry is keyed by the SHA-256 of the R-213 key string with mermaid 12.2.0',
    function()
      for _, c in ipairs({
        { FLOW, 700, 'dark' },
        { WIDE, 333, 'forest' },
        { U.source('types/pie'), 960, 'default' },
      }) do
        local res = ok_render(c[1], c[2], c[3])
        local key = key_of(c[1], c[2], c[3], '12.2.0')
        a.contains(
          key,
          vim.fs.basename(res.path),
          'cache entry name for ' .. c[3] .. ' at ' .. c[2]
        )
      end
    end
  )

  QA.check(
    'R-331',
    'The mermaid version in the key is the version the probe reports',
    { severity = 'medium' },
    function()
      local probe =
        U.session()
          :request('probe', { plugin_version = '0.0.0', cache_dir = U.session().cache_dir })
      a.not_nil(probe and probe.result, 'a probe result')
      local version = probe.result.mermaid_version
      a.eq('12.2.0', version, 'the pinned mermaid version')
      local res = ok_render(FLOW, 640, 'neutral')
      a.contains(key_of(FLOW, 640, 'neutral', version), vim.fs.basename(res.path))
    end
  )

  QA.check(
    'R-213',
    'The key string uses the lowercase hex hash of the source, not the source itself',
    { severity = 'medium' },
    function()
      local res = ok_render(FLOW, 700, 'base')
      local raw = vim.fn.sha256(FLOW .. '|700|base|12.2.0')
      a.not_contains(raw, vim.fs.basename(res.path), 'not keyed on the raw source')
      a.contains(key_of(FLOW, 700, 'base', '12.2.0'), vim.fs.basename(res.path))
    end
  )

  QA.check(
    'R-297',
    'The no-diagram-type text keeps every line break of the diagram text',
    function()
      local _, err = U.render('teapots are lovely\n\nand so are kettles\n  and cups', 960)
      a.not_nil(err)
      a.contains(
        'for text: teapots are lovely\n\nand so are kettles\n  and cups',
        tostring(err.message)
      )
    end
  )

  QA.check('R-297', 'The no-diagram-type text leaves out directives and comments', function()
    local src =
      '%%{init: {"theme": "dark"}}%%\nteapots are lovely\n%% a remark about tea\nand so are kettles'
    local _, err = U.render(src, 960)
    a.not_nil(err)
    local msg = tostring(err.message)
    a.matches(
      'for text: \n?teapots are lovely\nand so are kettles$',
      msg,
      'the text without the directive and the comment'
    )
    a.not_contains('init', msg)
    a.not_contains('remark', msg)
  end)

  QA.check('R-297', 'The no-diagram-type text leaves out front matter', function()
    local _, err =
      U.render('---\ntitle: Tea time\n---\nteapots are lovely\nand so are kettles', 960)
    a.not_nil(err)
    local msg = tostring(err.message)
    a.matches('for text: \n?teapots are lovely\nand so are kettles$', msg)
    a.not_contains('Tea time', msg)
  end)

  QA.check('R-297', 'The no-diagram-type text is not shortened', function()
    local lines = {}
    for i = 1, 40 do
      lines[i] = 'teapot line ' .. i .. ' with some words to make it long'
    end
    local text = table.concat(lines, '\n')
    local _, err = U.render(text, 960)
    a.not_nil(err)
    a.contains('for text: ' .. text, tostring(err.message))
  end)
end)
