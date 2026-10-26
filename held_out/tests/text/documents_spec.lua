local QA = require('qa')
local T = require('text_util')
local a = QA.assert

local DOCS = { 'readme.md', 'gfm_all.md' }
local WIDTHS = { 50, 80, 120 }

local function load(name)
  local text = QA.read(QA.evals .. '/text/' .. name)
  QA.need(text, 'the sample document ' .. name)
  local lines = vim.split(text, '\n', { plain = true })
  if lines[#lines] == '' then
    table.remove(lines)
  end
  return lines
end

local function layout(name, w)
  return T.layout(load(name), w, { images = false, dir = QA.evals .. '/text' })
end

-- Preview lines that belong to a fenced code block body.
local function code_lines(model)
  local set = {}
  for _, r in ipairs(model.code_regions or {}) do
    for i = r.line_start or r[1], r.line_end or r[2] do
      set[i] = true
    end
  end
  return set
end

QA.describe('text: whole documents', function()
  QA.before_each(function()
    QA.reset_plugin()
  end)

  for _, name in ipairs(DOCS) do
    for _, w in ipairs(WIDTHS) do
      QA.check(
        { 'R-029', 'R-055' },
        'In a realistic document no preview line is wider than the text width plus the left margin',
        function()
          local model = layout(name, w)
          local limit = T.text_width(w) + T.MARGIN
          for i, l in ipairs(model.lines) do
            a.le(
              limit,
              T.dw((l:gsub('%s+$', ''))),
              ('line %d at window width %d: %q'):format(i, w, l)
            )
          end
        end
      )
    end

    QA.check(
      'R-171',
      'A realistic document renders with no raw markdown markers outside code blocks',
      function()
        local model = layout(name, 80)
        local code = code_lines(model)
        for i, l in ipairs(model.lines) do
          if not code[i] then
            -- an undefined footnote reference is shown as written (R-219)
            local s = l:gsub('%[%^ghost%]', '')
            for _, m in ipairs(T.MARKERS) do
              a.no_match(m[1], s, ('%s on line %d'):format(m[2], i))
            end
            a.no_match('%]%[', s, 'reference link marker on line ' .. i)
            a.no_match('%[[%w ]+%]%[', s, 'reference link on line ' .. i)
          end
        end
      end
    )

    QA.check(
      'R-061',
      'A realistic document starts with content and never has two blank lines in a row',
      function()
        local lines = QA.lines(layout(name, 80))
        a.ne('', lines[1], 'blank first line')
        for i = 2, #lines do
          a.falsy(lines[i] == '' and lines[i - 1] == '', 'two blank lines at ' .. i)
        end
      end
    )

    QA.check(
      'R-205',
      'source_lines maps every preview line and never goes backwards (before the footnote section)',
      function()
        local model = layout(name, 60)
        local src = model.source_lines
        QA.need(type(src) == 'table', 'source_lines in the layout model')
        a.eq(#model.lines, #src, 'one source line per preview line')
        local n = #load(name)
        for i = 1, #src do
          a.truthy(
            type(src[i]) == 'number' and src[i] >= 1 and src[i] <= n,
            'source line in range at ' .. i
          )
          if i > 1 then
            a.ge(src[i - 1], src[i], 'non-decreasing at ' .. i)
          end
        end
      end
    )

    QA.check(
      'R-157',
      'A full document gives the same model on every layout call',
      { severity = 'medium' },
      function()
        a.eq(layout(name, 70), layout(name, 70))
      end
    )
  end

  QA.check(
    'R-059',
    'In a full document the YAML front matter is hidden and the first preview line is the first heading',
    function()
      local model = layout('readme.md', 80)
      a.not_contains('tags:', T.text(model))
      a.not_contains('title:', T.text(model))
      a.contains('Widget Toolkit', model.lines[1])
    end
  )

  QA.check(
    {
      'R-027',
      'R-039',
      'R-040',
      'R-042',
      'R-043',
      'R-049',
      'R-051',
      'R-052',
      'R-053',
      'R-054',
      'R-055',
      'R-056',
    },
    'A document with every GFM feature shows every feature in rendered form',
    function()
      local model = layout('gfm_all.md', 80)
      local text = T.text(model)
      for _, s in ipairs({
        '🎉',
        '┌',
        '└',
        'lua',
        'Tip',
        'Important',
        'Warning',
        'Caution',
        'The first note.',
        'The second note.',
        'Setext level one',
        '3. Third',
        '1. Paren one',
        'The end.',
      }) do
        a.contains(s, text, s)
      end
      a.not_contains('Nobody points here', text, 'unreferenced definition shown')
      local one = T.find(model, 'The first note.')
      local two = T.find(model, 'The second note.')
      local fin = T.find(model, 'The end.')
      a.truthy(
        one and two and fin and fin < one and one < two,
        'definitions at the end in reference order'
      )
    end
  )
end)
