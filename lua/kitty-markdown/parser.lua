-- Tree-sitter walk of the source buffer into the block list (package L-B).
-- Signatures: AGENTS.md, Conventions, Lua module contracts.
--
-- The walk reads only the `markdown` and `markdown_inline` trees. Trees of
-- injected code languages are ignored, so the block list does not depend on
-- which other parsers are installed (R-209).

local M = {}

local get_text = vim.treesitter.get_node_text

local ALERT_KINDS = { NOTE = true, TIP = true, IMPORTANT = true, WARNING = true, CAUTION = true }

-- Named entities the preview decodes (R-060 shows the rest as written).
local ENTITIES = {
  amp = '&',
  lt = '<',
  gt = '>',
  quot = '"',
  apos = "'",
  nbsp = '\194\160',
  copy = '©',
  reg = '®',
  trade = '™',
  hellip = '…',
  mdash = '—',
  ndash = '–',
  lsquo = '‘',
  rsquo = '’',
  ldquo = '“',
  rdquo = '”',
  laquo = '«',
  raquo = '»',
  times = '×',
  divide = '÷',
  deg = '°',
  plusmn = '±',
  middot = '·',
  bull = '•',
  euro = '€',
  pound = '£',
  yen = '¥',
  cent = '¢',
  sect = '§',
  para = '¶',
  larr = '←',
  rarr = '→',
  uarr = '↑',
  darr = '↓',
  harr = '↔',
  hearts = '♥',
  check = '✓',
}

local function decode_entity(text)
  local name = text:match('^&(%a+);$')
  if name then
    return ENTITIES[name] or text
  end
  local hex = text:match('^&#[xX](%x+);$')
  local code = hex and tonumber(hex, 16) or tonumber(text:match('^&#(%d+);$') or '')
  if code and code > 0 and code < 0x110000 then
    return vim.fn.nr2char(code)
  end
  return text
end

local function strip_angle(dest)
  local inner = dest:match('^<(.*)>$')
  return inner or dest
end

-- `<img>` attribute values; width and height become numbers, the rest stay
-- strings. Returns nil when the tag is not an img tag.
local function parse_img(tag)
  if not tag:match('^<[iI][mM][gG][%s/>]') then
    return nil
  end
  local attrs = {}
  for name, value in tag:gmatch('([%w-]+)%s*=%s*"([^"]*)"') do
    attrs[name:lower()] = value
  end
  for name, value in tag:gmatch("([%w-]+)%s*=%s*'([^']*)'") do
    attrs[name:lower()] = value
  end
  for name, value in tag:gmatch('([%w-]+)%s*=%s*([^%s"\'=<>`]+)') do
    if attrs[name:lower()] == nil then
      attrs[name:lower()] = value
    end
  end
  local size = {}
  for _, key in ipairs({ 'width', 'height' }) do
    local n = tonumber((attrs[key] or ''):match('^%s*(%d+)'))
    if n then
      size[key] = n
    end
  end
  return { src = attrs.src or '', alt = attrs.alt or '', attrs = size }
end

local function is_blank(line)
  return line:match('^%s*$') ~= nil
end

local function trim(s)
  return (s:gsub('^%s+', ''):gsub('%s+$', ''))
end

local function normalize_label(label)
  return (vim.fn.tolower(trim(label)):gsub('%s+', ' '))
end

-- One walk over one buffer. Holds the lines, the inline tree roots keyed by
-- start position, the link definitions, and the per-row prefix columns to
-- skip while slicing text out of a container.
local Walk = {}
Walk.__index = Walk

-- `shared` carries the state a nested walk (the body of a footnote) shares
-- with the document walk: link definitions and slug occurrences.
function Walk.new(lines, shared)
  local self = setmetatable({}, Walk)
  self.lines = lines
  self.source = table.concat(lines, '\n') .. '\n'
  self.inline_roots = {}
  self.defs = shared and shared.defs or {}
  self.slug_counts = shared and shared.slug_counts or {}
  self.skip = {}
  return self
end

local function key_of(row, col)
  return row .. ':' .. col
end

-- Block containers (quotes, lists) repeat a prefix on every continuation
-- line. The inline grammar never sees that prefix, so text slices must skip
-- it too. `skip[row]` is the column where the content of that row starts.
function Walk:note_continuations(node)
  for child in node:iter_children() do
    if child:type() == 'block_continuation' then
      local sr, _, _, ec = child:range()
      if (self.skip[sr] or 0) < ec then
        self.skip[sr] = ec
      end
    end
  end
end

-- Buffer text between two positions, minus the continuation prefixes, as a
-- list of `{ text, row }` pieces, one per source row (row 1-based).
function Walk:slice(sr, sc, er, ec)
  local pieces = {}
  for row = sr, er do
    local line = self.lines[row + 1] or ''
    local from = row == sr and sc or 0
    local skip = self.skip[row]
    if skip and skip > from then
      from = skip
    end
    local to = row == er and ec or #line
    -- an empty row inside a span still separates the words around it, and
    -- so does a span that ends at the start of a row: the soft break before
    -- a styled run, a code span, or a link is one space (R-029)
    if to > from or row < er or row > sr then
      pieces[#pieces + 1] = { text = line:sub(from + 1, math.max(to, from)), row = row + 1 }
    end
  end
  return pieces
end

function Walk:node_text(node)
  local sr, sc, er, ec = node:range()
  local pieces = self:slice(sr, sc, er, ec)
  local parts = {}
  for i, piece in ipairs(pieces) do
    parts[i] = piece.text
  end
  return table.concat(parts, '\n')
end

-- Plain text runs are scanned for the two GitHub extensions that the inline
-- grammar does not mark: emoji shortcodes and bare URLs (R-037, R-056).
local function split_text(text, row, out)
  local pos = 1
  local len = #text
  while pos <= len do
    local s, e, shortcode = text:find(':([%w_+-]+):', pos)
    local us, ue = text:find('%f[%w]https?://[^%s<>]+', pos)
    if not us then
      us, ue = text:find('%f[%w]www%.[^%s<>]+', pos)
    end
    if us and (not s or us < s) then
      -- trailing sentence punctuation is not part of a bare URL
      while ue > us and text:sub(ue, ue):match('[%.,;:!%?%)%]]') do
        ue = ue - 1
      end
      if us > pos then
        out[#out + 1] = { type = 'text', text = text:sub(pos, us - 1), row = row }
      end
      local url = text:sub(us, ue)
      local target = url:match('^www%.') and ('http://' .. url) or url
      out[#out + 1] = { type = 'autolink', text = url, url = target, row = row }
      pos = ue + 1
    elseif s then
      if s > pos then
        out[#out + 1] = { type = 'text', text = text:sub(pos, s - 1), row = row }
      end
      out[#out + 1] = { type = 'emoji', shortcode = shortcode, row = row }
      pos = e + 1
    else
      out[#out + 1] = { type = 'text', text = text:sub(pos), row = row }
      pos = len + 1
    end
  end
end

-- Inline math with single dollar signs, as GitHub finds it (R-280): a `$`
-- opens only when a character other than a space or `$` follows it, and
-- the span closes at the next `$` only when a character other than a space
-- comes before that `$`. A span holds no `$` of its own, so in
-- `$5 and $10` neither dollar sign pairs. A `\$` neither opens nor closes.
-- Returns the start and the end byte of the next span from `pos`, or nil.
local function find_math(text, pos)
  local function can_open(at)
    local after = text:sub(at + 1, at + 1)
    local before = at > 1 and text:sub(at - 1, at - 1) or ''
    return after ~= '' and not after:match('[%s$]') and before ~= '$' and before ~= '\\'
  end
  local open = text:find('$', pos, true)
  while open do
    local close = text:find('$', open + 1, true)
    while close and text:sub(close - 1, close - 1) == '\\' do
      close = text:find('$', close + 1, true)
    end
    if not close then
      return nil
    end
    if
      can_open(open)
      and not text:sub(close - 1, close - 1):match('%s')
      and text:sub(close + 1, close + 1) ~= '$'
    then
      return open, close
    end
    open = close
  end
  return nil
end

-- Emit the text pieces of a slice. A newline inside a slice is a soft break,
-- shown as one space; the whitespace around it is dropped. Inline math is
-- found here, on the joined text, so a span may cross a soft break.
local function emit_pieces(pieces, out)
  local parts, starts, rows = {}, {}, {}
  local len = 0
  for i, piece in ipairs(pieces) do
    local text = piece.text
    if i > 1 then
      text = text:gsub('^%s+', '')
    end
    if i < #pieces then
      text = text:gsub('%s+$', '') .. ' '
    end
    if text ~= '' then
      parts[#parts + 1] = text
      starts[#starts + 1] = len + 1
      rows[#rows + 1] = piece.row
      len = len + #text
    end
  end
  local joined = table.concat(parts)
  -- Plain text from byte `a` to byte `b`, split at the piece boundaries so
  -- each item keeps the row it came from.
  local function plain(a, b)
    for k = 1, #starts do
      local s = math.max(a, starts[k])
      local e = math.min(b, (starts[k + 1] or len + 1) - 1)
      if e >= s then
        split_text(joined:sub(s, e), rows[k], out)
      end
    end
  end
  local function row_at(byte)
    local row = rows[1]
    for k = 1, #starts do
      if starts[k] <= byte then
        row = rows[k]
      end
    end
    return row
  end
  local pos = 1
  while pos <= len do
    local open, close = find_math(joined, pos)
    if not open then
      break
    end
    plain(pos, open - 1)
    out[#out + 1] = {
      type = 'math',
      tex = trim(joined:sub(open + 1, close - 1)),
      display = false,
      row = row_at(open),
    }
    pos = close + 1
  end
  plain(pos, len)
end

function Walk:inline_root(node)
  local sr, sc = node:start()
  return self.inline_roots[key_of(sr, sc)]
end

function Walk:resolve_ref(label)
  return self.defs[normalize_label(label)]
end

local function first_named(node, type_name)
  for child in node:iter_children() do
    if child:type() == type_name then
      return child
    end
  end
  return nil
end

-- Markers the preview conceals (R-032 to R-035, R-062).
local DELIMITERS = {
  emphasis_delimiter = true,
  code_span_delimiter = true,
  latex_span_delimiter = true,
}

-- Obsidian wikilinks and embeds (`[[Page]]`, `[[Page|alias]]`,
-- `![[image.png]]`) are not links on GitHub, so they stay as written
-- (R-036). The inline grammar reads them as a link inside brackets or as an
-- image; this finds their ranges, per row, as 0-based `{ row, s, e }` with
-- `e` exclusive.
function Walk:wikilinks(node)
  local spans = {}
  local sr, _, er = node:range()
  for row = sr, er do
    local line = self.lines[row + 1] or ''
    local pos = 1
    while true do
      local s, e = line:find('!?%[%[[^%[%]]+%]%]', pos)
      if not s then
        break
      end
      spans[#spans + 1] = { row, s - 1, e }
      pos = e + 1
    end
  end
  return spans
end

local LINK_NODES = {
  shortcut_link = true,
  full_reference_link = true,
  collapsed_reference_link = true,
  inline_link = true,
  image = true,
}

-- A child that stays in the surrounding text: a single-dollar math span
-- (the text scan of `emit_pieces` applies GitHub's delimiter rules to it)
-- and a link or image node that overlaps a wikilink.
function Walk:is_literal(child, wikilinks)
  if child:type() == 'latex_block' then
    local first = child:child(0)
    if first and first:type() == 'latex_span_delimiter' and #get_text(first, self.source) == 1 then
      return true
    end
  end
  if not LINK_NODES[child:type()] then
    return false
  end
  local csr, csc, cer, cec = child:range()
  for _, span in ipairs(wikilinks) do
    local row, s, e = span[1], span[2], span[3]
    if row >= csr and row <= cer then
      local from = row == csr and csc or 0
      local to = row == cer and cec or math.huge
      if from < e and to > s then
        return true
      end
    end
  end
  return false
end

-- Recursive walk of one inline tree node into inline items. Text between
-- the named children is plain text; delimiters are skipped.
function Walk:inlines_of(node, out)
  local _, _, er, ec = node:range()
  local cur_row, cur_col = node:start()
  local wikilinks = self:wikilinks(node)
  for child in node:iter_children() do
    if child:named() and not self:is_literal(child, wikilinks) then
      local csr, csc, cer, cec = child:range()
      emit_pieces(self:slice(cur_row, cur_col, csr, csc), out)
      if not DELIMITERS[child:type()] then
        self:inline_node(child, out)
      end
      cur_row, cur_col = cer, cec
    end
  end
  emit_pieces(self:slice(cur_row, cur_col, er, ec), out)
end

function Walk:children_inlines(node)
  local out = {}
  self:inlines_of(node, out)
  return out
end

local function plain_text(inlines)
  local parts = {}
  for _, item in ipairs(inlines) do
    if item.text then
      parts[#parts + 1] = item.text
    elseif item.type == 'emoji' then
      parts[#parts + 1] = ':' .. item.shortcode .. ':'
    elseif item.type == 'math' then
      parts[#parts + 1] = item.tex
    elseif item.type == 'image' then
      parts[#parts + 1] = item.alt
    elseif item.children then
      parts[#parts + 1] = plain_text(item.children)
    end
  end
  return table.concat(parts)
end

function Walk:link_item(node, text_node, target, out)
  local row = node:start() + 1
  local item = { type = 'link', target = target, row = row, children = {} }
  if text_node then
    item.children = self:children_inlines(text_node)
  end
  out[#out + 1] = item
end

-- The range between the first and the last delimiter child of a span node,
-- so the content of a code span or math span excludes its markers.
function Walk:inner_range(node)
  local sr, sc, er, ec = node:range()
  local first, last
  for child in node:iter_children() do
    if DELIMITERS[child:type()] then
      if not first then
        first = child
      end
      last = child
    end
  end
  if first then
    local _, _, fer, fec = first:range()
    sr, sc = fer, fec
  end
  if last and last ~= first then
    er, ec = last:start()
  end
  return sr, sc, er, ec
end

function Walk:inline_node(node, out)
  local ntype = node:type()
  local row = node:start() + 1
  if ntype == 'emphasis' or ntype == 'strong_emphasis' or ntype == 'strikethrough' then
    local kind = ntype == 'emphasis' and 'emph'
      or ntype == 'strong_emphasis' and 'strong'
      or 'strike'
    local children = self:children_inlines(node)
    -- `~~x~~` parses as a strikethrough inside a strikethrough; one is enough
    if kind == 'strike' and #children == 1 and children[1].type == 'strike' then
      children = children[1].children
    end
    out[#out + 1] = { type = kind, children = children, row = row }
  elseif ntype == 'code_span' then
    local parts = {}
    for _, piece in ipairs(self:slice(self:inner_range(node))) do
      parts[#parts + 1] = piece.text
    end
    local code = table.concat(parts, ' ')
    -- CommonMark strips one space on each side of a padded code span
    if code:match('^ .* $') and not code:match('^%s*$') then
      code = code:sub(2, -2)
    end
    out[#out + 1] = { type = 'code', text = code, row = row }
  elseif ntype == 'inline_link' then
    local dest = first_named(node, 'link_destination')
    local target = dest and strip_angle(self:node_text(dest)) or ''
    self:link_item(node, first_named(node, 'link_text'), target, out)
  elseif ntype == 'image' then
    local dest = first_named(node, 'link_destination')
    local desc = first_named(node, 'image_description')
    local alt = desc and plain_text(self:children_inlines(desc)) or ''
    local src = dest and strip_angle(self:node_text(dest)) or ''
    if not dest then
      -- `![alt][label]` resolves through the definitions like a link
      local label = first_named(node, 'link_label')
      local def = self:resolve_ref(label and self:node_text(label):sub(2, -2) or alt)
      src = def or ''
    end
    out[#out + 1] = { type = 'image', src = src, alt = alt, attrs = {}, row = row }
  elseif ntype == 'shortcut_link' then
    local text_node = first_named(node, 'link_text')
    local text = text_node and self:node_text(text_node) or ''
    if text:match('^%^') then
      out[#out + 1] = { type = 'footnote_ref', label = text:sub(2), row = row }
    else
      local target = self:resolve_ref(text)
      if target then
        self:link_item(node, text_node, target, out)
      else
        -- no definition: GitHub shows the brackets as written
        emit_pieces(self:slice(node:range()), out)
      end
    end
  elseif ntype == 'full_reference_link' or ntype == 'collapsed_reference_link' then
    local text_node = first_named(node, 'link_text')
    local label_node = first_named(node, 'link_label')
    local label = label_node and self:node_text(label_node):sub(2, -2)
      or (text_node and self:node_text(text_node))
      or ''
    local target = self:resolve_ref(label)
    if target then
      self:link_item(node, text_node, target, out)
    else
      emit_pieces(self:slice(node:range()), out)
    end
  elseif ntype == 'uri_autolink' or ntype == 'email_autolink' then
    local text = self:node_text(node):sub(2, -2)
    local url = text
    if ntype == 'email_autolink' then
      url = 'mailto:' .. text
    end
    out[#out + 1] = { type = 'autolink', text = text, url = url, row = row }
  elseif ntype == 'backslash_escape' then
    out[#out + 1] = { type = 'text', text = self:node_text(node):sub(2), row = row }
  elseif ntype == 'hard_line_break' then
    out[#out + 1] = { type = 'linebreak', row = row }
  elseif ntype == 'latex_block' then
    local open_len = 1
    for child in node:iter_children() do
      if child:type() == 'latex_span_delimiter' then
        open_len = #get_text(child, self.source)
        break
      end
    end
    local parts = {}
    for _, piece in ipairs(self:slice(self:inner_range(node))) do
      parts[#parts + 1] = piece.text
    end
    local tex = trim(table.concat(parts, '\n'))
    out[#out + 1] = { type = 'math', tex = tex, display = open_len >= 2, row = row }
  elseif ntype == 'html_tag' then
    local text = self:node_text(node)
    local img = parse_img(text)
    if text:match('^<!%-%-') then
      -- an HTML comment shows nothing; the text around it stays (R-270)
      return
    elseif img then
      out[#out + 1] = { type = 'image', src = img.src, alt = img.alt, attrs = img.attrs, row = row }
    else
      out[#out + 1] = { type = 'html', text = text, row = row }
    end
  elseif ntype == 'entity_reference' or ntype == 'numeric_character_reference' then
    out[#out + 1] = { type = 'text', text = decode_entity(self:node_text(node)), row = row }
  elseif ntype == 'link_text' or ntype == 'image_description' or ntype == 'inline' then
    self:inlines_of(node, out)
  else
    -- unknown inline node: keep its text so nothing disappears
    emit_pieces(self:slice(node:range()), out)
  end
end

-- Inline items for a block-tree node that carries an injected inline tree
-- (an `inline` node or a `pipe_table_cell`).
function Walk:inlines(node)
  if not node then
    return {}
  end
  self:note_continuations(node)
  local root = self:inline_root(node)
  local out = {}
  if root then
    self:inlines_of(root, out)
  else
    emit_pieces(self:slice(node:range()), out)
  end
  return out
end

local function trim_inlines(inlines)
  local first = inlines[1]
  if first and first.type == 'text' then
    first.text = first.text:gsub('^%s+', '')
    if first.text == '' then
      table.remove(inlines, 1)
    end
  end
  local last = inlines[#inlines]
  if last and last.type == 'text' then
    last.text = last.text:gsub('%s+$', '')
    if last.text == '' then
      table.remove(inlines)
    end
  end
  return inlines
end

-- Rows of a block node, 1-based and inclusive, without trailing blank rows.
function Walk:rows(node)
  local sr, _, er, ec = node:range()
  local last = ec == 0 and er or er + 1
  if last < sr + 1 then
    last = sr + 1
  end
  while last > sr + 1 and is_blank((self.lines[last] or ''):sub((self.skip[last - 1] or 0) + 1)) do
    last = last - 1
  end
  return sr + 1, last
end

-- Content lines of a node, with the container prefix and `indent` leading
-- columns removed from each line.
function Walk:content_lines(node, indent)
  self:note_continuations(node)
  local sr, sc, er, ec = node:range()
  local last = ec <= (self.skip[er] or 0) and er - 1 or er
  local out = {}
  for row = sr, last do
    local line = self.lines[row + 1] or ''
    local from = math.max(self.skip[row] or 0, row == sr and sc or 0)
    local text = line:sub(from + 1)
    if indent and indent > 0 then
      local spaces = text:match('^ *')
      text = text:sub(math.min(#spaces, indent) + 1)
    end
    out[#out + 1] = text
  end
  return out
end

-- Remove HTML comments from the lines of an HTML block (R-270). A line
-- that a comment touched and that holds nothing else is dropped; `rows`
-- keeps the source row of each line that stays. A comment open at the end
-- runs to the end of the block, as an HTML block of type 2 does.
local function strip_comments(lines, rows)
  local out_lines, out_rows = {}, {}
  local in_comment = false
  for i, line in ipairs(lines) do
    local kept = {}
    local touched = in_comment
    local pos = 1
    while pos <= #line do
      if in_comment then
        local e = line:find('-->', pos, true)
        in_comment = e == nil
        pos = e and e + 3 or #line + 1
      else
        local s = line:find('<!--', pos, true)
        kept[#kept + 1] = line:sub(pos, s and s - 1 or #line)
        if s then
          in_comment, touched = true, true
          pos = s + 4
        else
          pos = #line + 1
        end
      end
    end
    local text = table.concat(kept)
    if not touched then
      out_lines[#out_lines + 1] = line
      out_rows[#out_rows + 1] = rows[i]
    elseif not is_blank(text) then
      out_lines[#out_lines + 1] = (text:gsub('%s+$', ''))
      out_rows[#out_rows + 1] = rows[i]
    end
  end
  return out_lines, out_rows
end

local function mermaid_alt(source)
  local in_front = false
  for line in (source .. '\n'):gmatch('([^\n]*)\n') do
    local t = trim(line)
    if t == '---' then
      in_front = not in_front
    elseif not in_front and t ~= '' and not t:match('^%%%%') then
      return t:match('^%S+')
    end
  end
  return 'mermaid'
end

-- Pull every image out of an inline list. Top-level images become blocks
-- after the paragraph (R-095); images nested in a link or emphasis leave
-- their alt text behind so the surrounding text stays readable.
local function lift_images(inlines, lifted, nested)
  local kept = {}
  for _, item in ipairs(inlines) do
    if item.type == 'image' then
      lifted[#lifted + 1] = item
      if nested and item.alt ~= '' then
        kept[#kept + 1] = { type = 'text', text = item.alt, row = item.row }
      end
    else
      if item.children then
        item.children = lift_images(item.children, lifted, true)
      end
      kept[#kept + 1] = item
    end
  end
  return kept
end

function Walk:image_block(item, src_start, src_end)
  return {
    kind = 'image',
    src = item.src,
    alt = item.alt,
    attrs = item.attrs or {},
    src_start = src_start,
    src_end = src_end,
  }
end

-- Footnote definitions have no tree-sitter node, so a paragraph may hold
-- several `[^label]: body` lines. Split it at every such line start.
function Walk:split_footnotes(inlines, src_start, src_end, out)
  local groups = {}
  local current = nil
  for i, item in ipairs(inlines) do
    local prev = inlines[i - 1]
    local starts_line = prev == nil or (prev.type == 'text' and (prev.row or 0) < (item.row or 0))
    local line = self.lines[item.row or 0] or ''
    if
      item.type == 'footnote_ref'
      and starts_line
      and line:match('^%s*%[%^[^%]]+%]:')
      and (current ~= nil or i == 1)
    then
      if prev and prev.type == 'text' then
        prev.text = prev.text:gsub('%s+$', '')
        if prev.text == '' then
          current.inlines[#current.inlines] = nil
        end
      end
      current = { label = item.label, inlines = {}, row = item.row }
      groups[#groups + 1] = current
    elseif current then
      if #current.inlines == 0 and item.type == 'text' then
        item.text = item.text:gsub('^:%s*', '')
        if item.text ~= '' then
          current.inlines[1] = item
        end
      else
        current.inlines[#current.inlines + 1] = item
      end
    else
      return false
    end
  end
  for i, group in ipairs(groups) do
    local last = groups[i + 1] and groups[i + 1].row - 1 or src_end
    out[#out + 1] = {
      kind = 'footnote_def',
      label = group.label,
      inlines = trim_inlines(group.inlines),
      src_start = math.max(src_start, group.row),
      src_end = math.max(last, group.row),
    }
  end
  return #groups > 0
end

function Walk:paragraph(node, out)
  local src_start, src_end = self:rows(node)
  local inline = first_named(node, 'inline')
  local inlines = trim_inlines(self:inlines(inline))
  local first = inlines[1]
  if first and first.type == 'footnote_ref' then
    local line = self.lines[first.row or 0] or ''
    if
      line:match('^%s*%[%^[^%]]+%]:') and self:split_footnotes(inlines, src_start, src_end, out)
    then
      return
    end
  end
  local lifted = {}
  inlines = trim_inlines(lift_images(inlines, lifted, false))
  if #inlines > 0 then
    out[#out + 1] =
      { kind = 'paragraph', inlines = inlines, src_start = src_start, src_end = src_end }
  end
  for _, item in ipairs(lifted) do
    out[#out + 1] = self:image_block(item, item.row, item.row)
  end
end

function Walk:heading(node, out)
  local src_start, src_end = self:rows(node)
  local level = 1
  local inline
  for child in node:iter_children() do
    local t = child:type()
    local n = t:match('^atx_h(%d)_marker$') or t:match('^setext_h(%d)_underline$')
    if n then
      level = tonumber(n)
    elseif t == 'inline' then
      inline = child
    elseif t == 'paragraph' then
      inline = first_named(child, 'inline')
    end
  end
  local inlines = trim_inlines(self:inlines(inline))
  -- the closing `##` of an ATX heading is a marker, not text
  local last = inlines[#inlines]
  if last and last.type == 'text' then
    last.text = last.text:gsub('%s+#+%s*$', '')
    if last.text == '' then
      table.remove(inlines)
    end
  end
  local lifted = {}
  inlines = lift_images(inlines, lifted, true)
  local text = plain_text(inlines)
  local layout = require('kitty-markdown.layout')
  local slug = layout.slug(text)
  local count = self.slug_counts[slug]
  self.slug_counts[slug] = (count or 0) + 1
  if count then
    slug = slug .. '-' .. count
  end
  out[#out + 1] = {
    kind = 'heading',
    level = level,
    inlines = inlines,
    slug = slug,
    src_start = src_start,
    src_end = src_end,
  }
end

function Walk:list(node, out)
  local src_start, src_end = self:rows(node)
  local block = { kind = 'list', ordered = false, start = 1, items = {} }
  block.src_start, block.src_end = src_start, src_end
  local first_marker = true
  for item_node in node:iter_children() do
    if item_node:type() == 'list_item' then
      self:note_continuations(item_node)
      local item = { inlines = {}, children = {} }
      item.src_start, item.src_end = self:rows(item_node)
      local seen_para = false
      for child in item_node:iter_children() do
        local t = child:type()
        if t:match('^list_marker_') then
          if t == 'list_marker_dot' or t == 'list_marker_parenthesis' then
            block.ordered = true
            if first_marker then
              block.start = tonumber(get_text(child, self.source):match('%d+')) or 1
            end
          end
          first_marker = false
        elseif t == 'task_list_marker_checked' then
          item.checked = true
        elseif t == 'task_list_marker_unchecked' then
          item.checked = false
        elseif t == 'paragraph' and not seen_para then
          seen_para = true
          local blocks = {}
          self:paragraph(child, blocks)
          for i, b in ipairs(blocks) do
            if i == 1 and b.kind == 'paragraph' then
              item.inlines = b.inlines
            else
              item.children[#item.children + 1] = b
            end
          end
        elseif t ~= 'block_continuation' then
          seen_para = true
          self:block(child, item.children)
        end
      end
      block.items[#block.items + 1] = item
    end
  end
  out[#out + 1] = block
end

function Walk:table(node, out)
  local src_start, src_end = self:rows(node)
  local block = { kind = 'table', align = {}, header = {}, rows = {} }
  block.src_start, block.src_end = src_start, src_end
  local function cells_of(row_node)
    local cells = {}
    for cell in row_node:iter_children() do
      if cell:type() == 'pipe_table_cell' then
        cells[#cells + 1] = trim_inlines(self:inlines(cell))
      end
    end
    return cells
  end
  for child in node:iter_children() do
    local t = child:type()
    if t == 'pipe_table_header' then
      block.header = cells_of(child)
    elseif t == 'pipe_table_delimiter_row' then
      for cell in child:iter_children() do
        if cell:type() == 'pipe_table_delimiter_cell' then
          local left = first_named(cell, 'pipe_table_align_left') ~= nil
          local right = first_named(cell, 'pipe_table_align_right') ~= nil
          local align = 'left'
          if left and right then
            align = 'center'
          elseif right then
            align = 'right'
          end
          block.align[#block.align + 1] = align
        end
      end
    elseif t == 'pipe_table_row' then
      block.rows[#block.rows + 1] = cells_of(child)
    end
  end
  out[#out + 1] = block
end

function Walk:fenced_code(node, out)
  local src_start, src_end = self:rows(node)
  local lang
  local info = first_named(node, 'info_string')
  if info then
    local language = first_named(info, 'language')
    lang = language and get_text(language, self.source)
      or trim(get_text(info, self.source)):match('^%S+')
    if lang == '' then
      lang = nil
    end
  end
  local delimiter = first_named(node, 'fenced_code_block_delimiter')
  local indent = 0
  if delimiter then
    local text = get_text(delimiter, self.source)
    indent = #text:match('^ *')
  end
  local content = first_named(node, 'code_fence_content')
  local lines = content and self:content_lines(content, indent) or {}
  if lang == 'mermaid' then
    local source = table.concat(lines, '\n')
    out[#out + 1] = {
      kind = 'mermaid',
      source = source,
      alt = mermaid_alt(source),
      src_start = src_start,
      src_end = src_end,
    }
    return
  end
  out[#out + 1] =
    { kind = 'code', lang = lang, lines = lines, src_start = src_start, src_end = src_end }
end

function Walk:indented_code(node, out)
  local src_start, src_end = self:rows(node)
  local lines = self:content_lines(node, 4)
  while #lines > 0 and is_blank(lines[#lines]) do
    lines[#lines] = nil
  end
  -- Footnote definitions have no tree-sitter node, so the later paragraphs
  -- and blocks of a definition, indented by four columns after a blank
  -- line, parse as an indented code block. GitHub reads them as markdown
  -- inside the definition (R-273).
  local prev = out[#out]
  if prev and prev.kind == 'footnote_def' then
    prev.children = prev.children or {}
    vim.list_extend(prev.children, self:parse_nested(lines, src_start - 1))
    prev.src_end = src_end
    return
  end
  out[#out + 1] = { kind = 'code', lines = lines, src_start = src_start, src_end = src_end }
end

function Walk:block_quote(node, out)
  local src_start, src_end = self:rows(node)
  self:note_continuations(node)
  local children = {}
  for child in node:iter_children() do
    local t = child:type()
    if t ~= 'block_quote_marker' and t ~= 'block_continuation' then
      self:block(child, children)
    end
  end
  -- a quote whose first paragraph opens with `[!KIND]` is a GitHub alert
  local first = children[1]
  if first and first.kind == 'paragraph' then
    local a, b = first.inlines[1], first.inlines[2]
    local kind
    if a and a.type == 'text' and a.text:match('^%[!%u+%]%s*$') then
      kind = a.text:match('^%[!(%u+)%]')
    end
    if kind and ALERT_KINDS[kind] then
      table.remove(first.inlines, 1)
      if b and b.type == 'text' then
        b.text = b.text:gsub('^%s+', '')
      end
      if #trim_inlines(first.inlines) == 0 then
        table.remove(children, 1)
      end
      out[#out + 1] = {
        kind = 'alert',
        type = kind,
        children = children,
        src_start = src_start,
        src_end = src_end,
      }
      return
    end
  end
  out[#out + 1] =
    { kind = 'blockquote', children = children, src_start = src_start, src_end = src_end }
end

function Walk:html_block(node, out)
  local src_start, src_end = self:rows(node)
  local lines = self:content_lines(node)
  while #lines > 0 and is_blank(lines[#lines]) do
    lines[#lines] = nil
  end
  local joined = trim(table.concat(lines, ' '))
  local img = parse_img(joined)
  if img and joined:match('^<[^<>]*>$') then
    out[#out + 1] = self:image_block(img, src_start, src_end)
    return
  end
  local rows = {}
  for i = 1, #lines do
    rows[i] = src_start + i - 1
  end
  if table.concat(lines, '\n'):find('<!--', 1, true) then
    lines, rows = strip_comments(lines, rows)
    if #lines == 0 then
      -- a block of comments only shows nothing, so its neighbors keep the
      -- one blank line between blocks (R-270)
      return
    end
  end
  out[#out + 1] =
    { kind = 'html', lines = lines, line_rows = rows, src_start = src_start, src_end = src_end }
end

function Walk:reference_definition(node, out)
  local src_start, src_end = self:rows(node)
  local label_node = first_named(node, 'link_label')
  local label = label_node and get_text(label_node, self.source):sub(2, -2) or ''
  if label:match('^%^') then
    -- `[^label]: word` is a footnote whose body is one word
    local line = self.lines[src_start] or ''
    local body = line:match('^%s*%[%^[^%]]*%]:%s*(.*)$') or ''
    local inlines = {}
    split_text(trim(body), src_start, inlines)
    out[#out + 1] = {
      kind = 'footnote_def',
      label = label:sub(2),
      inlines = inlines,
      src_start = src_start,
      src_end = src_end,
    }
  end
  -- other definitions were collected in the first pass and show nothing
end

function Walk:collect_definitions(node)
  local t = node:type()
  if t == 'link_reference_definition' then
    local label_node = first_named(node, 'link_label')
    local dest = first_named(node, 'link_destination')
    if label_node and dest then
      local label = get_text(label_node, self.source):sub(2, -2)
      if not label:match('^%^') then
        local key = normalize_label(label)
        if self.defs[key] == nil then
          self.defs[key] = strip_angle(get_text(dest, self.source))
        end
      end
    end
    return
  end
  for child in node:iter_children() do
    if child:named() then
      self:collect_definitions(child)
    end
  end
end

function Walk:block(node, out)
  local t = node:type()
  self:note_continuations(node)
  if t == 'section' or t == 'document' then
    for child in node:iter_children() do
      if child:named() then
        self:block(child, out)
      end
    end
  elseif t == 'atx_heading' or t == 'setext_heading' then
    self:heading(node, out)
  elseif t == 'paragraph' then
    self:paragraph(node, out)
  elseif t == 'list' then
    self:list(node, out)
  elseif t == 'pipe_table' then
    self:table(node, out)
  elseif t == 'fenced_code_block' then
    self:fenced_code(node, out)
  elseif t == 'indented_code_block' then
    self:indented_code(node, out)
  elseif t == 'block_quote' then
    self:block_quote(node, out)
  elseif t == 'html_block' then
    self:html_block(node, out)
  elseif t == 'link_reference_definition' then
    self:reference_definition(node, out)
  elseif t == 'thematic_break' then
    local src_start, src_end = self:rows(node)
    out[#out + 1] = { kind = 'rule', src_start = src_start, src_end = src_end }
  elseif t == 'minus_metadata' or t == 'plus_metadata' then
    local src_start, src_end = self:rows(node)
    out[#out + 1] = {
      kind = 'front_matter',
      lines = self:content_lines(node),
      src_start = src_start,
      src_end = src_end,
    }
  end
  -- block_continuation and markers carry no content of their own
end

-- The walk loads the markdown grammars that ship with Neovim by file path,
-- under names of its own. A parser of another version on 'runtimepath' (for
-- example one from nvim-treesitter), a runtimepath with no parser
-- directory, or a missing `markdown_inline` then gives the same tree
-- (R-209). The injection query is the plugin's own too: it injects only
-- the inline grammar, so no parser of a code fence language is loaded.
local LANG = 'kitty_markdown'
local INLINE_LANG = 'kitty_markdown_inline'
local INJECTIONS = '([(inline) (pipe_table_cell)] @injection.content'
  .. ' (#set! injection.language "'
  .. INLINE_LANG
  .. '"))'

-- Directories that may hold the bundled parsers, most specific first: the
-- runtime directory, the `lib/nvim/parser` directory of the install prefix
-- (where release archives and Homebrew put them), and last the parser
-- directories on 'runtimepath', the bundled ones (late on the path) first.
local function parser_dirs()
  local dirs = {}
  local runtime = vim.env.VIMRUNTIME
  if runtime and runtime ~= '' then
    dirs[#dirs + 1] = vim.fs.joinpath(runtime, 'parser')
    dirs[#dirs + 1] = vim.fs.normalize(vim.fs.joinpath(runtime, '../../../lib/nvim/parser'))
  end
  local prog = vim.v.progpath
  if prog and prog ~= '' then
    local prefix = vim.fs.dirname(vim.fs.dirname(vim.fn.resolve(prog)))
    dirs[#dirs + 1] = vim.fs.joinpath(prefix, 'lib/nvim/parser')
  end
  local found = vim.api.nvim_get_runtime_file('parser/markdown_inline.*', true)
  for i = #found, 1, -1 do
    dirs[#dirs + 1] = vim.fs.dirname(found[i])
  end
  return dirs
end

local function library(dir, name)
  for _, ext in ipairs({ 'so', 'dll', 'dylib' }) do
    local path = vim.fs.joinpath(dir, name .. '.' .. ext)
    if vim.uv.fs_stat(path) then
      return path
    end
  end
  return nil
end

local languages_ready = false

local function load_languages()
  if languages_ready then
    return
  end
  for _, dir in ipairs(parser_dirs()) do
    local block_lib = library(dir, 'markdown')
    local inline_lib = library(dir, 'markdown_inline')
    if block_lib and inline_lib then
      local language = vim.treesitter.language
      local ok_block = language.add(LANG, { path = block_lib, symbol_name = 'markdown' })
      local ok_inline =
        language.add(INLINE_LANG, { path = inline_lib, symbol_name = 'markdown_inline' })
      if ok_block and ok_inline then
        languages_ready = true
        return
      end
    end
  end
  error('kitty-markdown: no markdown tree-sitter parser found in the Neovim install', 0)
end

-- Move every source row in a block list by `offset`: the rows of a nested
-- body are relative to the body, not to the document.
local function shift_rows(value, offset)
  for key, field in pairs(value) do
    if key == 'line_rows' then
      for i, row in ipairs(field) do
        field[i] = row + offset
      end
    elseif type(field) == 'table' then
      shift_rows(field, offset)
    elseif type(field) == 'number' and (key == 'src_start' or key == 'src_end' or key == 'row') then
      value[key] = field + offset
    end
  end
end

-- Parse lines into the block list. `shared` is the state of the document
-- walk when the lines are a nested body; `row_offset` moves the source rows
-- of the result to the rows of that body in the document.
local function parse_lines(lines, shared, row_offset)
  load_languages()
  local walk = Walk.new(lines, shared)
  local parser =
    vim.treesitter.get_string_parser(walk.source, LANG, { injections = { [LANG] = INJECTIONS } })
  parser:parse(true)
  local root
  parser:for_each_tree(function(tree, ltree)
    local lang = ltree:lang()
    if lang == LANG and not root then
      root = tree:root()
    elseif lang == INLINE_LANG then
      local node = tree:root()
      local sr, sc = node:start()
      walk.inline_roots[key_of(sr, sc)] = node
    end
  end)
  local blocks = {}
  if root then
    walk:collect_definitions(root)
    walk:block(root, blocks)
  end
  if row_offset and row_offset ~= 0 then
    shift_rows(blocks, row_offset)
  end
  return blocks
end

function Walk:parse_nested(lines, row_offset)
  return parse_lines(lines, { defs = self.defs, slug_counts = self.slug_counts }, row_offset)
end

--- Parse a markdown buffer into the block list. The list also carries
--- `dir`, the directory of the buffer name, which the layout uses to
--- resolve relative image and file paths when `opts.dir` is absent.
function M.parse(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local blocks = parse_lines(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false))
  local name = vim.api.nvim_buf_get_name(bufnr)
  blocks.dir = name ~= '' and vim.fs.dirname(name) or nil
  return blocks
end

return M
