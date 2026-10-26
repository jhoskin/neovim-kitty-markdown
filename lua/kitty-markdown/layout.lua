-- Pure layout: block list to layout model (package L-B).
-- Signatures: AGENTS.md, Conventions, Lua module contracts.
--
-- `build` reads no clock, no random source, and no environment, so the same
-- blocks and options give the same model (R-157). Everything that touches
-- the buffer or the terminal lives in the parser and the preview.

local icons = require('kitty-markdown.icons')
local emoji = require('kitty-markdown.emoji')

local M = {}

local strwidth = vim.api.nvim_strwidth

local MARGIN = '  '
local SUPERSCRIPT = { '⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹' }
local ALERT_TITLE = {
  NOTE = 'Note',
  TIP = 'Tip',
  IMPORTANT = 'Important',
  WARNING = 'Warning',
  CAUTION = 'Caution',
}
local ALERT_GROUP = {
  NOTE = 'KittyMarkdownAlertNote',
  TIP = 'KittyMarkdownAlertTip',
  IMPORTANT = 'KittyMarkdownAlertImportant',
  WARNING = 'KittyMarkdownAlertWarning',
  CAUTION = 'KittyMarkdownAlertCaution',
}

--- GitHub heading slug: lower case, punctuation other than hyphen and
--- underscore dropped, spaces to hyphens. The caller adds `-1`, `-2` for
--- duplicates.
function M.slug(text)
  local s = vim.fn.tolower(vim.trim(text))
  s = s:gsub('[%p]', function(c)
    if c == '-' or c == '_' then
      return c
    end
    return ''
  end)
  s = s:gsub('%s', '-')
  return s
end

local function superscript(n)
  local out = {}
  for digit in tostring(n):gmatch('%d') do
    out[#out + 1] = SUPERSCRIPT[tonumber(digit) + 1]
  end
  return table.concat(out)
end

local function link_kind(target)
  if target:match('^#') then
    return 'anchor'
  elseif target:match('^%a[%w+.-]*:') then
    return 'url'
  end
  return 'file'
end

local function resolve_path(path, dir)
  if path == '' or path:match('^/') or path:match('^%a[%w+.-]*:') or path:match('^~') then
    return path
  end
  if dir == nil or dir == '' or dir == '.' then
    return path
  end
  return vim.fs.joinpath(dir, path)
end

local function with_group(groups, group)
  local out = { unpack(groups) }
  out[#out + 1] = group
  return out
end

local function serialize_attrs(attrs)
  local keys = vim.tbl_keys(attrs or {})
  table.sort(keys)
  local parts = {}
  for _, k in ipairs(keys) do
    parts[#parts + 1] = k .. '=' .. tostring(attrs[k])
  end
  return table.concat(parts, ',')
end

-- The model under construction plus the state the renderers share.
local Doc = {}
Doc.__index = Doc

function Doc.new(opts, text_width)
  local self = setmetatable({}, Doc)
  self.opts = opts
  self.text_width = text_width
  self.lines = {}
  self.src = {}
  self.highlights = {}
  self.tail = {}
  self.links = {}
  self.anchors = {}
  self.code_regions = {}
  self.blocks = {}
  self.footnotes = {}
  self.footnote_numbers = {}
  self.footnote_defs = {}
  self.defined = {}
  return self
end

-- Add a highlight span, extending the previous span of the same group when
-- the two touch, so a styled run is one span even when it was built from
-- several words.
function Doc:hl(line, col_start, col_end, group)
  if col_end <= col_start then
    return
  end
  local tail = self.tail[group]
  if tail and tail.line == line and tail.col_end == col_start then
    tail.col_end = col_end
    return
  end
  local span = { line = line, col_start = col_start, col_end = col_end, group = group }
  self.highlights[#self.highlights + 1] = span
  self.tail[group] = span
end

function Doc:link(line, col_start, col_end, link)
  local last = self.links[#self.links]
  if
    last
    and last.line == line
    and last.col_end == col_start
    and last.kind == link.kind
    and last.target == link.target
  then
    last.col_end = col_end
    return
  end
  self.links[#self.links + 1] = {
    line = line,
    col_start = col_start,
    col_end = col_end,
    kind = link.kind,
    target = link.target,
  }
end

-- Append a preview line. `prefix` is `{ text, spans }` with spans relative
-- to the prefix start; `content` follows it. Returns the line number.
function Doc:line(prefix, content, src)
  local n = #self.lines + 1
  local base = #MARGIN
  local head = prefix.text
  if content == nil or content == '' then
    -- no trailing spaces on a line that holds only a prefix
    head = head:gsub('%s+$', '')
  end
  self.lines[n] = MARGIN .. head .. (content or '')
  self.src[n] = src
  for _, span in ipairs(prefix.spans or {}) do
    self:hl(n, base + span[1], base + span[2], span[3])
  end
  return n, base + #prefix.text
end

-- A separator line. Inside a quote it keeps the bar; elsewhere it is empty.
function Doc:blank(prefix)
  local head = prefix and prefix.text:gsub('%s+$', '') or ''
  if head ~= '' then
    self:line({ text = head, spans = prefix.spans }, '', nil)
  else
    local n = #self.lines + 1
    self.lines[n] = ''
    self.src[n] = nil
  end
end

function Doc:footnote_number(label)
  local key = vim.fn.tolower(label)
  local n = self.footnote_numbers[key]
  if not n then
    n = #self.footnotes + 1
    self.footnote_numbers[key] = n
    self.footnotes[n] = { label = label, number = n }
  end
  return n
end

-- Inline items to a flat token list. A token is a word fragment
-- `{ text, width, groups, link, row }`, a space `{ space, groups }`, a hard
-- break `{ br }`, or a centered display line `{ display, text, groups }`.
function Doc:flatten(inlines, groups, link, out)
  for _, item in ipairs(inlines) do
    local t = item.type
    if t == 'text' then
      self:words(item.text, groups, link, item.row, out)
    elseif t == 'emph' then
      self:flatten(item.children, with_group(groups, 'KittyMarkdownEmph'), link, out)
    elseif t == 'strong' then
      self:flatten(item.children, with_group(groups, 'KittyMarkdownStrong'), link, out)
    elseif t == 'strike' then
      self:flatten(item.children, with_group(groups, 'KittyMarkdownStrike'), link, out)
    elseif t == 'code' then
      self:words(item.text, with_group(groups, 'KittyMarkdownCode'), link, item.row, out)
    elseif t == 'link' then
      local kind = link_kind(item.target)
      local target = item.target
      if kind == 'anchor' then
        target = target:sub(2)
      elseif kind == 'file' then
        target = resolve_path(target, self.opts.dir)
      end
      local l = { kind = kind, target = target }
      self:flatten(item.children, with_group(groups, 'KittyMarkdownLink'), l, out)
    elseif t == 'autolink' then
      local l = { kind = 'url', target = item.url }
      self:words(item.text, with_group(groups, 'KittyMarkdownUrl'), l, item.row, out)
    elseif t == 'image' then
      -- a stray inline image (the parser lifts them) shows its alt text
      self:words(item.alt or '', groups, link, item.row, out)
    elseif t == 'footnote_ref' and not self.defined[vim.fn.tolower(item.label)] then
      -- no definition: GitHub shows the reference as written and gives it
      -- no number (R-219)
      self:words('[^' .. item.label .. ']', groups, link, item.row, out)
    elseif t == 'footnote_ref' then
      local n = self:footnote_number(item.label)
      self:words(superscript(n), with_group(groups, 'KittyMarkdownFootnote'), link, item.row, out)
    elseif t == 'math' then
      -- math.render lands with package L-C; until then the stub has no render
      local texmath = require('kitty-markdown.math')
      local rendered = texmath.render and texmath.render(item.tex) or nil
      local text = rendered or item.tex
      local g = with_group(groups, 'KittyMarkdownMath')
      if item.display then
        out[#out + 1] = { display = true, text = text, groups = g, row = item.row }
      else
        self:words(text, g, link, item.row, out)
      end
    elseif t == 'html' then
      self:words(item.text, with_group(groups, 'KittyMarkdownHtml'), link, item.row, out)
    elseif t == 'emoji' then
      local text = emoji.lookup(item.shortcode) or (':' .. item.shortcode .. ':')
      self:words(text, groups, link, item.row, out)
    elseif t == 'linebreak' then
      out[#out + 1] = { br = true }
    end
  end
end

-- Split a text run into word fragments and spaces; display math and hard
-- breaks never come through here.
function Doc:words(text, groups, link, row, out)
  local pos = 1
  local len = #text
  while pos <= len do
    local s, e = text:find('%s+', pos)
    if s == pos then
      out[#out + 1] = { space = true, groups = groups, link = link }
      pos = e + 1
    else
      local stop = s and s - 1 or len
      local word = text:sub(pos, stop)
      out[#out + 1] =
        { text = word, width = strwidth(word), groups = groups, link = link, row = row }
      pos = stop + 1
    end
  end
end

-- Word wrap a token list into lines. `first` and `cont` are the prefixes
-- of the first and the following lines; `width` is the cells available
-- after the prefix. A word wider than `width` stands alone on its line
-- (R-029, R-030).
function Doc:wrap(tokens, first, cont, width)
  local line, line_w = {}, 0
  local word, word_w = {}, 0
  local pending_space = nil
  local emitted = 0

  local function emit_line(force)
    if #line == 0 and not force then
      return
    end
    local prefix = emitted == 0 and first or cont
    local parts = {}
    local row
    for i, tok in ipairs(line) do
      parts[i] = tok.text
      if tok.row and (not row or tok.row < row) then
        row = tok.row
      end
    end
    local n, base = self:line(prefix, table.concat(parts), row)
    local col = base
    for _, tok in ipairs(line) do
      local stop = col + #tok.text
      for _, group in ipairs(tok.groups) do
        self:hl(n, col, stop, group)
      end
      if tok.link then
        self:link(n, col, stop, tok.link)
      end
      col = stop
    end
    emitted = emitted + 1
    line, line_w = {}, 0
  end

  local function place_word()
    if #word == 0 then
      return
    end
    if line_w > 0 and line_w + 1 + word_w > width then
      emit_line()
    end
    if line_w > 0 then
      -- the space inside a styled run or a link keeps its style
      line[#line + 1] = {
        text = ' ',
        groups = pending_space and pending_space.groups or {},
        link = pending_space and pending_space.link or nil,
      }
      line_w = line_w + 1
    end
    for _, frag in ipairs(word) do
      line[#line + 1] = frag
    end
    line_w = line_w + word_w
    word, word_w = {}, 0
    pending_space = nil
  end

  for _, tok in ipairs(tokens) do
    if tok.space then
      place_word()
      pending_space = tok
    elseif tok.br then
      place_word()
      emit_line(true)
      pending_space = nil
    elseif tok.display then
      place_word()
      emit_line()
      local pad = math.max(0, math.floor((width - strwidth(tok.text)) / 2))
      local prefix = emitted == 0 and first or cont
      local n, base = self:line(prefix, string.rep(' ', pad) .. tok.text, tok.row)
      for _, group in ipairs(tok.groups) do
        self:hl(n, base + pad, base + pad + #tok.text, group)
      end
      emitted = emitted + 1
      pending_space = nil
    else
      word[#word + 1] = tok
      word_w = word_w + tok.width
    end
  end
  place_word()
  emit_line()
  if emitted == 0 then
    emit_line(true)
  end
end

-- One line of inline content with no wrapping; table cells use it.
function Doc:cell_text(inlines)
  local tokens = {}
  self:flatten(inlines, {}, nil, tokens)
  local parts, frags = {}, {}
  local width = 0
  local need_space = false
  for _, tok in ipairs(tokens) do
    if tok.space or tok.br then
      need_space = #frags > 0
    elseif tok.text then
      if need_space then
        frags[#frags + 1] = { text = ' ', groups = {} }
        parts[#parts + 1] = ' '
        width = width + 1
        need_space = false
      end
      frags[#frags + 1] = tok
      parts[#parts + 1] = tok.text
      width = width + (tok.width or strwidth(tok.text))
    end
  end
  return { text = table.concat(parts), width = width, frags = frags }
end

-- Prefix helpers. A context is `{ prefix = { text, spans }, width,
-- groups }`: the line prefix after the margin, the cells left for content,
-- and the groups every text fragment carries (for example Quote).
local function extend_prefix(prefix, text, group)
  local spans = { unpack(prefix.spans or {}) }
  if group then
    spans[#spans + 1] = { #prefix.text, #prefix.text + #text, group }
  end
  return { text = prefix.text .. text, spans = spans }
end

local function spaces(n)
  return string.rep(' ', n)
end

function Doc:render_paragraph(block, ctx)
  local tokens = {}
  self:flatten(block.inlines, ctx.groups, nil, tokens)
  self:wrap(tokens, ctx.prefix, ctx.prefix, ctx.width)
end

function Doc:render_heading(block, ctx)
  local group = 'KittyMarkdownH' .. block.level
  local icon = icons.h[block.level] or icons.h[#icons.h]
  local first = extend_prefix(ctx.prefix, icon .. ' ', group)
  local cont = extend_prefix(ctx.prefix, spaces(strwidth(icon) + 1))
  local tokens = {}
  self:flatten(block.inlines, with_group(ctx.groups, group), nil, tokens)
  local line = #self.lines + 1
  self:wrap(tokens, first, cont, ctx.width - strwidth(icon) - 1)
  if block.slug and block.slug ~= '' and self.anchors[block.slug] == nil then
    self.anchors[block.slug] = line
  end
end

function Doc:render_list(block, ctx, depth)
  local indent = spaces(2 * depth)
  local number = block.start or 1
  for _, item in ipairs(block.items) do
    local marker, marker_group
    if item.checked ~= nil then
      -- The checkbox takes the place of the bullet or the number, as GitHub
      -- hides the list marker of a task item (R-042). A task item still
      -- counts in the numbering of an ordered list.
      marker = item.checked and icons.checked or icons.unchecked
      marker_group = item.checked and 'KittyMarkdownChecked' or 'KittyMarkdownUnchecked'
    elseif block.ordered then
      marker = tostring(number) .. '.'
      marker_group = 'KittyMarkdownNumber'
    else
      marker = icons.bullet[(depth % #icons.bullet) + 1]
      marker_group = 'KittyMarkdownBullet'
    end
    number = number + 1
    local first = extend_prefix(ctx.prefix, indent)
    first = extend_prefix(first, marker, marker_group)
    first = extend_prefix(first, ' ')
    local head_width = strwidth(marker) + 1
    local cont = extend_prefix(ctx.prefix, indent .. spaces(head_width))
    local width = ctx.width - 2 * depth - head_width
    local tokens = {}
    self:flatten(item.inlines, ctx.groups, nil, tokens)
    self:wrap(tokens, first, cont, width)
    local child_ctx = { prefix = cont, width = width, groups = ctx.groups }
    for _, child in ipairs(item.children) do
      if child.kind == 'list' then
        self:render_list(child, ctx, depth + 1)
      elseif child.kind == 'image' or child.kind == 'mermaid' then
        self:render_pixel(child)
        self:blank(cont)
      else
        self:blank(cont)
        self:render_block(child, child_ctx)
      end
    end
  end
end

function Doc:render_table(block, ctx)
  local ncols = #block.header
  if ncols == 0 then
    for _, row in ipairs(block.rows) do
      ncols = math.max(ncols, #row)
    end
  end
  local widths = {}
  for c = 1, ncols do
    widths[c] = 0
  end
  local function cells_of(cells)
    local out = {}
    for c = 1, ncols do
      local cell = self:cell_text(cells[c] or {})
      out[c] = cell
      if cell.width > widths[c] then
        widths[c] = cell.width
      end
    end
    return out
  end
  local header = cells_of(block.header)
  local rows = {}
  for i, row in ipairs(block.rows) do
    rows[i] = cells_of(row)
  end
  local border = 'KittyMarkdownTableBorder'

  local function rule(left, mid, right, src)
    local parts = { left }
    for c = 1, ncols do
      parts[#parts + 1] = string.rep('─', widths[c] + 2)
      parts[#parts + 1] = c < ncols and mid or right
    end
    local text = table.concat(parts)
    local n, base = self:line(ctx.prefix, text, src)
    self:hl(n, base, base + #text, border)
  end

  local function row_line(cells, src, header_row)
    local parts = { '│' }
    local offsets = {}
    local bytes = #'│'
    for c = 1, ncols do
      local cell = cells[c]
      local pad = widths[c] - cell.width
      local align = block.align[c] or 'left'
      local left_pad, right_pad = 0, pad
      if align == 'right' then
        left_pad, right_pad = pad, 0
      elseif align == 'center' then
        left_pad = math.floor(pad / 2)
        right_pad = pad - left_pad
      end
      local lead = ' ' .. spaces(left_pad)
      parts[#parts + 1] = lead
      bytes = bytes + #lead
      offsets[c] = bytes
      parts[#parts + 1] = cell.text
      bytes = bytes + #cell.text
      local trail = spaces(right_pad) .. ' │'
      parts[#parts + 1] = trail
      bytes = bytes + #trail
    end
    local text = table.concat(parts)
    local n, base = self:line(ctx.prefix, text, src)
    for c = 1, ncols do
      local cell = cells[c]
      local start = base + offsets[c]
      if header_row then
        self:hl(n, start, start + #cell.text, 'KittyMarkdownTableHeader')
      end
      local at = start
      for _, frag in ipairs(cell.frags) do
        local stop = at + #frag.text
        for _, group in ipairs(frag.groups) do
          self:hl(n, at, stop, group)
        end
        if frag.link then
          self:link(n, at, stop, frag.link)
        end
        at = stop
      end
    end
    -- every bar in the row is a border span
    local search = 1
    while true do
      local s, e = text:find('│', search, true)
      if not s then
        break
      end
      self:hl(n, base + s - 1, base + e, border)
      search = e + 1
    end
  end

  local src = block.src_start
  rule('┌', '┬', '┐', src)
  if #block.header > 0 then
    row_line(header, src, true)
    rule('├', '┼', '┤', src)
  end
  for i, cells in ipairs(rows) do
    row_line(cells, src + 1 + i, false)
  end
  rule('└', '┴', '┘', block.src_end)
end

function Doc:render_code(block, ctx)
  local width = ctx.width
  local lang = block.lang
  local top
  if lang and lang ~= '' then
    local label = ' ' .. lang .. ' '
    top = '──' .. label .. string.rep('─', math.max(0, width - 2 - strwidth(label)))
  else
    top = string.rep('─', width)
  end
  local n, base = self:line(ctx.prefix, top, block.src_start)
  if lang and lang ~= '' then
    self:hl(n, base, base + #'──', 'KittyMarkdownCodeBorder')
    self:hl(n, base + #'──', base + #'──' + #lang + 2, 'KittyMarkdownCodeLang')
    self:hl(n, base + #'──' + #lang + 2, base + #top, 'KittyMarkdownCodeBorder')
  else
    self:hl(n, base, base + #top, 'KittyMarkdownCodeBorder')
  end
  local first_line = #self.lines + 1
  for i, code in ipairs(block.lines) do
    local pad = math.max(0, width - strwidth(code))
    local text = code .. spaces(pad)
    local ln, b = self:line(ctx.prefix, text, block.src_start + i)
    self:hl(ln, b, b + #text, 'KittyMarkdownCodeBg')
  end
  local last_line = #self.lines
  if last_line >= first_line then
    self.code_regions[#self.code_regions + 1] =
      { line_start = first_line, line_end = last_line, lang = lang }
  end
  local bottom = string.rep('─', width)
  local bn, bb = self:line(ctx.prefix, bottom, block.src_end)
  self:hl(bn, bb, bb + #bottom, 'KittyMarkdownCodeBorder')
end

function Doc:render_blockquote(block, ctx)
  local prefix = extend_prefix(ctx.prefix, icons.quote, 'KittyMarkdownQuoteBar')
  prefix = extend_prefix(prefix, ' ')
  local inner = {
    prefix = prefix,
    width = ctx.width - strwidth(icons.quote) - 1,
    groups = with_group(ctx.groups, 'KittyMarkdownQuote'),
  }
  self:render_blocks(block.children, inner)
end

function Doc:render_alert(block, ctx)
  local group = ALERT_GROUP[block.type]
  local prefix = extend_prefix(ctx.prefix, icons.quote, group)
  prefix = extend_prefix(prefix, ' ')
  local title = (icons.alert[block.type] or '') .. ' ' .. (ALERT_TITLE[block.type] or block.type)
  local n, base = self:line(prefix, title, block.src_start)
  self:hl(n, base, base + #title, group)
  local inner = {
    prefix = prefix,
    width = ctx.width - strwidth(icons.quote) - 1,
    groups = with_group(ctx.groups, 'KittyMarkdownQuote'),
  }
  if #block.children > 0 then
    self:render_blocks(block.children, inner)
  end
end

function Doc:render_rule(block, ctx)
  local text = string.rep(icons.rule, ctx.width)
  local n, base = self:line(ctx.prefix, text, block.src_start)
  self:hl(n, base, base + #text, 'KittyMarkdownRule')
end

function Doc:render_html(block, ctx)
  for i, text in ipairs(block.lines) do
    local src = block.line_rows and block.line_rows[i] or block.src_start + i - 1
    local n, base = self:line(ctx.prefix, text, src)
    self:hl(n, base, base + #text, 'KittyMarkdownHtml')
  end
end

-- A pixel block is one empty anchor line; the preview hangs the box or the
-- placeholder rows below it (R-168). The descriptor carries what the
-- preview and the helper need.
function Doc:render_pixel(block)
  local opts = self.opts
  local n = #self.lines + 1
  self.lines[n] = ''
  self.src[n] = block.src_start
  local source
  if block.kind == 'image' then
    source = resolve_path(block.src, opts.dir)
  else
    source = block.source
  end
  local attrs = block.attrs or {}
  local width_cells = self.text_width
  local key = vim.fn.sha256(
    table.concat(
      { block.kind, source, tostring(width_cells), opts.theme, serialize_attrs(attrs) },
      '\0'
    )
  )
  local desc = {
    kind = block.kind,
    index = #self.blocks + 1,
    line = n,
    src_line = block.src_start,
    source = source,
    alt = block.alt or '',
    key = key,
    width_cells = width_cells,
    attrs = attrs,
  }
  local size = opts.sizes and opts.sizes[key]
  if size then
    desc.width_px = size.width_px
    desc.height_px = size.height_px
    desc.image_id = size.image_id
    if opts.cell and opts.cell.height_px and opts.cell.height_px > 0 and size.height_px then
      desc.rows = math.ceil(size.height_px / opts.cell.height_px)
    end
  end
  if opts.images == false then
    desc.text_only = true
  end
  self.blocks[#self.blocks + 1] = desc
  if block.kind == 'image' then
    self.links[#self.links + 1] =
      { line = n, col_start = 0, col_end = 0, kind = 'image', target = source }
  end
end

function Doc:render_block(block, ctx)
  local kind = block.kind
  if kind == 'paragraph' then
    self:render_paragraph(block, ctx)
  elseif kind == 'heading' then
    self:render_heading(block, ctx)
  elseif kind == 'list' then
    self:render_list(block, ctx, 0)
  elseif kind == 'table' then
    self:render_table(block, ctx)
  elseif kind == 'code' then
    self:render_code(block, ctx)
  elseif kind == 'blockquote' then
    self:render_blockquote(block, ctx)
  elseif kind == 'alert' then
    self:render_alert(block, ctx)
  elseif kind == 'rule' then
    self:render_rule(block, ctx)
  elseif kind == 'html' then
    self:render_html(block, ctx)
  elseif kind == 'footnote_def' then
    self.footnote_defs[#self.footnote_defs + 1] = block
  end
  -- front matter shows nothing (R-059)
end

local function is_hidden(block)
  return block.kind == 'front_matter' or block.kind == 'footnote_def'
end

local function is_pixel(block)
  return block.kind == 'image' or block.kind == 'mermaid'
end

-- Render a block sequence with one separator line between blocks (R-061).
-- The anchor line of a pixel block doubles as the separator before it, and
-- one separator follows it.
-- `sep_first` asks for a separator before the first block too.
function Doc:render_blocks(blocks, ctx, sep_first)
  local need_sep = sep_first == true
  for _, block in ipairs(blocks) do
    if is_hidden(block) then
      self:render_block(block, ctx)
    elseif is_pixel(block) then
      self:render_pixel(block)
      self:blank(ctx.prefix)
      need_sep = false
    else
      if need_sep then
        self:blank(ctx.prefix)
      end
      self:render_block(block, ctx)
      need_sep = true
    end
  end
end

-- Footnote definitions go to the end, in the order of first reference
-- (R-054). A definition nobody references is not shown.
function Doc:render_footnotes(ctx)
  local by_label = {}
  for _, def in ipairs(self.footnote_defs) do
    local key = vim.fn.tolower(def.label)
    if by_label[key] == nil then
      by_label[key] = def
    end
  end
  for _, note in ipairs(self.footnotes) do
    local def = by_label[vim.fn.tolower(note.label)]
    if def then
      -- a pixel block at the end already left its separator
      if #self.lines > 0 and self.lines[#self.lines] ~= '' then
        self:blank(ctx.prefix)
      end
      local mark = superscript(note.number)
      local first = extend_prefix(ctx.prefix, mark, 'KittyMarkdownFootnote')
      first = extend_prefix(first, ' ')
      local cont = extend_prefix(ctx.prefix, spaces(strwidth(mark) + 1))
      local tokens = {}
      self:flatten(def.inlines, ctx.groups, nil, tokens)
      self:wrap(tokens, first, cont, ctx.width - strwidth(mark) - 1)
      if def.children then
        local inner = { prefix = cont, width = ctx.width - strwidth(mark) - 1, groups = ctx.groups }
        self:render_blocks(def.children, inner, true)
      end
    end
  end
end

-- Blank lines take the source line of the next line with one; the mapping
-- then never decreases (R-205).
function Doc:finish_source_lines()
  local n = #self.lines
  local src = self.src
  local next_known = nil
  for i = n, 1, -1 do
    if src[i] ~= nil then
      next_known = src[i]
    else
      src[i] = next_known
    end
  end
  local prev = 1
  for i = 1, n do
    if src[i] == nil or src[i] < prev then
      src[i] = prev
    end
    prev = src[i]
  end
end

-- The labels of every footnote definition, in lower case, wherever the
-- definition sits (a quote or a list item may hold one).
local function collect_footnote_labels(blocks, set)
  for _, block in ipairs(blocks) do
    if block.kind == 'footnote_def' then
      set[vim.fn.tolower(block.label)] = true
    end
    collect_footnote_labels(block.children or {}, set)
    for _, item in ipairs(block.items or {}) do
      collect_footnote_labels(item.children or {}, set)
    end
  end
end

--- Build the layout model from a block list. `opts` keys: width, cell,
--- sizes, theme, images, dir (R-203).
function M.build(blocks, opts)
  opts = opts or {}
  local config = require('kitty-markdown.config').get()
  local width = opts.width or (config.text_width + 4)
  local text_width = math.max(1, math.min(width - 4, config.text_width))
  local resolved = {
    width = width,
    cell = opts.cell,
    sizes = opts.sizes or {},
    theme = opts.theme or require('kitty-markdown.theme').current(),
    images = opts.images ~= false,
    dir = opts.dir or blocks.dir,
  }
  local doc = Doc.new(resolved, text_width)
  collect_footnote_labels(blocks, doc.defined)
  local ctx = { prefix = { text = '', spans = {} }, width = text_width, groups = {} }
  doc:render_blocks(blocks, ctx)
  doc:render_footnotes(ctx)
  doc:finish_source_lines()
  return {
    width = width,
    text_width = text_width,
    theme = resolved.theme,
    lines = doc.lines,
    highlights = doc.highlights,
    conceals = {},
    blocks = doc.blocks,
    code_regions = doc.code_regions,
    links = doc.links,
    anchors = doc.anchors,
    source_lines = doc.src,
    footnotes = doc.footnotes,
  }
end

return M
