-- Helpers for the core area checks (math, theme, test hooks).

local QA = require('qa')

local bit = require('bit')

local M = {}

local strwidth = vim.api.nvim_strwidth

-- Field access that tolerates named or positional fields.
local function field(t, names, pos)
  for _, n in ipairs(names) do
    if t[n] ~= nil then
      return t[n]
    end
  end
  if pos then
    return t[pos]
  end
  return nil
end
M.field = field

function M.hl_parts(h)
  return field(h, { 'line', 'lnum', 'row' }, 1),
    field(h, { 'col_start', 'start_col', 'start' }, 2),
    field(h, { 'col_end', 'end_col', 'finish', 'stop' }, 3),
    field(h, { 'group', 'hl_group', 'hl' }, 4)
end

function M.link_parts(l)
  return field(l, { 'line', 'lnum', 'row' }, 1),
    field(l, { 'col_start', 'start_col', 'start' }, 2),
    field(l, { 'col_end', 'end_col', 'finish', 'stop' }, 3),
    field(l, { 'kind', 'type' }, 4),
    field(l, { 'target', 'url', 'href', 'dest' }, 5)
end

function M.region_parts(r)
  return field(r, { 'line_start', 'first', 'start', 'first_line', 'start_line' }, 1),
    field(r, { 'line_end', 'last', 'finish', 'last_line', 'end_line' }, 2),
    field(r, { 'lang', 'language' }, 3)
end

-- Joined preview text with the margins and trailing spaces removed.
function M.text(model)
  local out = {}
  for _, l in ipairs(model.lines) do
    out[#out + 1] = vim.trim(l)
  end
  return table.concat(out, '\n')
end

-- The rendered form of one inline math span. The span sits between two
-- marker words in a wide paragraph, so it never wraps.
function M.inline(tex)
  local model = QA.layout({ 'AAA $' .. tex .. '$ ZZZ' }, { width = 400 })
  local joined = table.concat(
    vim.tbl_map(function(l)
      return vim.trim(l)
    end, model.lines),
    ' '
  )
  local inner = joined:match('AAA (.-) ZZZ')
  if inner == nil then
    inner = joined:match('AAA(.-)ZZZ')
  end
  return inner, model, joined
end

-- A Unicode string from code points.
function M.u(...)
  local out = {}
  for _, cp in ipairs({ ... }) do
    out[#out + 1] = vim.fn.nr2char(cp, 1)
  end
  return table.concat(out)
end

-- Every highlight group that covers any byte of the first occurrence of
-- `needle` in the model lines. Lines are 1-based; columns are 0-based
-- byte offsets with an exclusive end (R-337).
function M.groups_over(model, needle)
  for i, l in ipairs(model.lines) do
    local s = l:find(needle, 1, true)
    if s then
      local e = s + #needle - 1
      local groups = {}
      for _, h in ipairs(model.highlights) do
        local line, cs, ce, g = M.hl_parts(h)
        if line == i and type(cs) == 'number' and type(ce) == 'number' then
          -- 0-based [cs, ce): covers byte b (1-based) when cs < b <= ce
          local covers0 = cs < e and ce >= s
          if covers0 then
            groups[#groups + 1] = g
          end
        end
      end
      return groups, i
    end
  end
  return nil
end

-- True when some highlight group covers every byte of `needle` (first
-- occurrence) and its name passes `pred`.
function M.covered_by(model, needle, pred)
  for i, l in ipairs(model.lines) do
    local s = l:find(needle, 1, true)
    if s then
      local e = s + #needle - 1
      for _, h in ipairs(model.highlights) do
        local line, cs, ce, g = M.hl_parts(h)
        if line == i and type(g) == 'string' and pred(g) then
          -- 0-based exclusive: bytes cs+1 .. ce in 1-based Lua terms
          if cs + 1 <= s and ce >= e then
            return true, g
          end
        end
      end
      return false
    end
  end
  return false
end

function M.is_math_group(g)
  return type(g) == 'string' and g:find('Math') ~= nil
end

-- Leading spaces of a line in cells.
function M.lead(l)
  return #(l:match('^ *'))
end

function M.width(s)
  return strwidth(s)
end

-- The left margin in cells, derived from a plain paragraph.
function M.margin(width)
  local model = QA.layout({ 'plainword' }, { width = width })
  for _, l in ipairs(model.lines) do
    if l:find('plainword', 1, true) then
      return M.lead(l)
    end
  end
  return 0
end

-- Checks that tables hold only plain values and no metatables.
function M.plain(v, seen)
  seen = seen or {}
  local t = type(v)
  if t == 'function' or t == 'userdata' or t == 'thread' then
    return false, t
  end
  if t == 'table' then
    if seen[v] then
      return true
    end
    seen[v] = true
    if getmetatable(v) ~= nil then
      return false, 'metatable'
    end
    for k, x in pairs(v) do
      local ok, why = M.plain(k, seen)
      if not ok then
        return false, why
      end
      ok, why = M.plain(x, seen)
      if not ok then
        return false, why
      end
    end
  end
  return true
end

-- Color helpers -----------------------------------------------------------

function M.rgb(n)
  return bit.rshift(n, 16) % 256, bit.rshift(n, 8) % 256, n % 256
end

function M.luminance(n)
  local r, g, b = M.rgb(n)
  local function ch(c)
    c = c / 255
    return c <= 0.03928 and c / 12.92 or ((c + 0.055) / 1.055) ^ 2.4
  end
  return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
end

-- Resolved highlight attributes (links followed) in a namespace.
function M.hl(name, ns)
  return vim.api.nvim_get_hl(ns or 0, { name = name, link = false })
end

-- The group names a layout model uses, found from known content.
function M.discover_groups()
  local model = QA.layout({
    '# Heading One',
    '',
    'para [linktext](https://example.com) and `codespan` and $\\alpha$ end',
    '',
    '<div>htmlblock</div>',
  }, { width = 120 })
  local function find(needle, pred)
    local groups = M.groups_over(model, needle) or {}
    for _, g in ipairs(groups) do
      if pred == nil or pred(g) then
        return g
      end
    end
    return groups[1]
  end
  return {
    heading = find('Heading One', function(g)
      return g:find('H1') or g:find('Heading')
    end),
    link = find('linktext', function(g)
      return g:find('Link') or g:find('Url')
    end),
    code = find('codespan', function(g)
      return g:find('Code')
    end),
    math = find(M.u(0x03B1), M.is_math_group),
    html = find('htmlblock'),
    model = model,
  }
end

-- Opens a text-only preview of a markdown buffer in a split. Returns the
-- preview window and buffer.
function M.open_preview(lines, placement)
  QA.capture_terminal()
  pcall(QA.fake_cell, 10, 20, 50, 200)
  local dir = QA.tmpdir()
  local path = dir .. '/doc.md'
  vim.fn.writefile(lines, path)
  local src = vim.fn.bufadd(path)
  vim.fn.bufload(src)
  vim.bo[src].filetype = 'markdown'
  vim.api.nvim_set_current_buf(src)
  local before = {}
  for _, w in ipairs(vim.api.nvim_list_wins()) do
    before[w] = true
  end
  QA.answer_probe(false)
  require('kitty-markdown').open(placement or 'split')
  local win
  QA.wait(2000, function()
    for _, w in ipairs(vim.api.nvim_list_wins()) do
      if not before[w] and vim.api.nvim_win_get_buf(w) ~= src then
        win = w
        return true
      end
    end
    return false
  end)
  QA.need(win ~= nil, 'a preview window opened in the headless test instance')
  QA.wait(300, function()
    return false
  end)
  return win, vim.api.nvim_win_get_buf(win), src
end

-- The highlight a window shows for a group: the window namespace when one
-- is set, then the window's winhighlight remap, then the global group.
function M.win_hl(win, name)
  local ns = 0
  if vim.api.nvim_get_hl_ns then
    local ok, n = pcall(vim.api.nvim_get_hl_ns, { winid = win })
    if ok and type(n) == 'number' and n > 0 then
      ns = n
    end
  end
  if ns > 0 then
    local h = M.hl(name, ns)
    if next(h) ~= nil then
      return h
    end
  end
  return M.hl(name, 0)
end

return M
