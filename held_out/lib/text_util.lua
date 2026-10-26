-- Helpers for the text rendering checks (SPEC.md "Text rendering").
local QA = require('qa')

local M = {}

M.MARGIN = 2
M.MAX = 100

-- Text width for a window width, per SPEC.md Definitions: the window width
-- minus 2 cells on each side, capped by the configured maximum (100).
function M.text_width(win_width, max)
  return math.min(win_width - 4, max or M.MAX)
end

function M.dw(s)
  return vim.fn.strdisplaywidth(s)
end

-- Layout with a window width. Returns the model and the buffer.
function M.layout(text, width, extra)
  local opts = vim.tbl_extend('force', { width = width or 80 }, extra or {})
  return QA.layout(text, opts)
end

-- Lines without trailing spaces and without the left margin.
function M.body(model)
  return vim.tbl_map(function(l)
    return vim.trim(l)
  end, model.lines)
end

-- Non-empty lines, trimmed.
function M.nonempty(model)
  local out = {}
  for _, l in ipairs(model.lines) do
    local t = vim.trim(l)
    if t ~= '' then
      table.insert(out, t)
    end
  end
  return out
end

-- The joined text of the model (lines joined by newline).
function M.text(model)
  return table.concat(model.lines, '\n')
end

-- Highlight fields, tolerant of the named or positional form.
function M.hl(h)
  return h.line or h[1],
    h.col_start or h.start_col or h[2],
    h.col_end or h.end_col or h[3],
    h.group or h[4]
end

-- First (line, byte col 0-based) of needle in the model, searching from line
-- `from` (default 1).
function M.find(model, needle, from)
  for i = from or 1, #model.lines do
    local s = model.lines[i]:find(needle, 1, true)
    if s then
      return i, s - 1
    end
  end
end

-- Groups of every span that covers each byte of needle (the intersection),
-- and the union. Returns intersection list, union list, line.
function M.groups_over(model, needle, from)
  local ln, col = M.find(model, needle, from)
  if not ln then
    return nil
  end
  local all, every = {}, {}
  local last = col + #needle
  for _, h in ipairs(model.highlights) do
    local l, cs, ce, g = M.hl(h)
    if l == ln and cs < last and ce > col then
      all[g] = true
      if cs <= col and ce >= last then
        every[g] = true
      end
    end
  end
  return vim.tbl_keys(every), vim.tbl_keys(all), ln
end

-- Groups of spans that cover byte col on line ln.
function M.groups_at_pos(model, ln, col)
  local out = {}
  for _, h in ipairs(model.highlights) do
    local l, cs, ce, g = M.hl(h)
    if l == ln and cs <= col and col < ce then
      table.insert(out, g)
    end
  end
  return out
end

-- Every group on a line.
function M.groups_on_line(model, ln)
  local out = {}
  for _, h in ipairs(model.highlights) do
    local l, _, _, g = M.hl(h)
    if l == ln then
      out[g] = true
    end
  end
  return vim.tbl_keys(out)
end

-- Make sure the plugin defined its highlight groups, then resolve a group
-- to its final attributes (links followed).
function M.ensure_theme()
  local ok = pcall(function()
    require('kitty-markdown').setup({})
  end)
  if not ok then
    pcall(function()
      require('kitty-markdown.theme').apply()
    end)
  end
end

function M.style(group)
  return vim.api.nvim_get_hl(0, { name = group, link = false }) or {}
end

-- True when any group in the list resolves to the attribute.
function M.any_attr(groups, attr)
  for _, g in ipairs(groups or {}) do
    if M.style(g)[attr] then
      return true, g
    end
  end
  return false
end

-- The foreground colors of a list of groups (set of numbers).
function M.fgs(groups)
  local out = {}
  for _, g in ipairs(groups or {}) do
    local fg = M.style(g).fg
    if fg then
      out[fg] = g
    end
  end
  return out
end

function M.plugin_groups(groups)
  return vim.tbl_filter(function(g)
    return type(g) == 'string' and g:match('^KittyMarkdown') ~= nil
  end, groups or {})
end

-- A code point is in a Unicode private use area (where Nerd Font icons
-- live).
function M.has_pua(s)
  for _, cp in ipairs(vim.fn.str2list(s)) do
    if
      (cp >= 0xE000 and cp <= 0xF8FF)
      or (cp >= 0xF0000 and cp <= 0xFFFFD)
      or (cp >= 0x100000 and cp <= 0x10FFFD)
    then
      return true
    end
  end
  return false
end

function M.has_non_ascii(s)
  return s:find('[\128-\255]') ~= nil
end

-- The muted color group: derived from a known muted element (a thematic
-- break, R-055), so the check does not hard-code its name.
function M.rule_groups()
  local model = M.layout({ 'a', '', '---', '', 'b' }, 60)
  for i, l in ipairs(model.lines) do
    if l:find('─', 1, true) or l:find('━', 1, true) then
      local out = {}
      for _, g in ipairs(M.groups_on_line(model, i)) do
        out[#out + 1] = g
      end
      return out
    end
  end
  return {}
end

-- Lines of the model that belong to a table (contain a vertical box
-- drawing border).
function M.table_lines(model)
  local out = {}
  for i, l in ipairs(model.lines) do
    if
      l:find('│', 1, true)
      or l:find('┌', 1, true)
      or l:find('└', 1, true)
      or l:find('├', 1, true)
    then
      table.insert(out, i)
    end
  end
  return out
end

-- Split a table row into its cell strings between the vertical borders.
function M.cells(line)
  local s = vim.trim(line)
  local parts = vim.split(s, '│', { plain = true })
  local out = {}
  for i = 2, #parts - 1 do
    out[#out + 1] = parts[i]
  end
  return out
end

-- Raw markdown marker patterns that must not appear in rendered prose.
M.MARKERS = {
  { '%*%*', 'strong marker **' },
  { '__', 'strong marker __' },
  { '~~', 'strikethrough marker ~~' },
  { '`', 'backtick' },
  { '%]%(', 'inline link ](' },
  { '^%s*#+%s', 'ATX heading marker' },
  { '^%s*>%s', 'blockquote marker >' },
  { '^%s*[-*+]%s', 'list marker' },
  { '%[[ xX]%]', 'task marker' },
  { '%[%^', 'footnote marker [^' },
  { '^%s*===+%s*$', 'setext underline' },
  { '^%s*%-%-%-+%s*$', 'thematic break or front matter ---' },
  { '%[!%u+%]', 'alert marker' },
  { '^%s*|.*|%s*$', 'pipe table row' },
}

return M
