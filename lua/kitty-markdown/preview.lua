-- Preview buffer and window, boxes, keys, scroll sync (package L-F).
-- Signatures: AGENTS.md, Conventions, Lua module contracts.
--
-- The preview composes the other modules: the layout gives the text and
-- the block descriptors, the helper renders pixels, and the transport puts
-- them on screen. The text reaches the buffer in the same call that asked
-- for the render; every pixel block arrives later through a scheduled
-- callback, so a render never blocks the source buffer (R-019, R-022).

local config = require('kitty-markdown.config')
local theme = require('kitty-markdown.theme')
local icons = require('kitty-markdown.icons')
local transport = require('kitty-markdown.transport')
local helper = require('kitty-markdown.helper')
local cache = require('kitty-markdown.cache')

local M = {}

local api = vim.api
local strwidth = api.nvim_strwidth

local AUGROUP = 'kitty-markdown-preview'
local SYNC_DEBOUNCE_MS = 100
-- Layout highlights sit above the default extmark priority of 100 only
-- when the tree-sitter token colors need to win over the code background.
local HL_PRIORITY = 100
local CODE_PRIORITY = 110
local MARGIN = '  '
local BORDER = 'KittyMarkdownBoxBorder'
local BOX_TEXT = 'KittyMarkdownBoxText'
local MUTED = 'KittyMarkdownMuted'

-- Failure box reasons by error code (AGENTS.md, Protocol methods and
-- errors). `parse_error` and `unsupported_feature` carry their detail in
-- `data` and are formatted in `reason_of`.
local REASONS = {
  timeout = 'timeout',
  too_large = 'too large',
  unsupported_format = 'unsupported format',
  unsupported_diagram_type = 'unsupported diagram type',
  fetch_failed = 'fetch failed',
  io = 'cannot read file',
  remote_images_off = 'remote images off',
  helper_exit = 'helper exited',
  too_tall = 'too tall',
}

-- Codes that are a state of the plugin, not a failure of one block, so
-- they do not repeat in `:messages` on every render.
-- A helper exit fails every waiting block at once; the helper module
-- writes the one WARN for it, so the blocks stay quiet (R-195, R-313).
local QUIET_CODES = { remote_images_off = true, no_helper = true, helper_exit = true }

local WINDOW_OPTIONS = {
  number = false,
  relativenumber = false,
  cursorline = false,
  list = false,
  wrap = false,
  spell = false,
  signcolumn = 'no',
  foldcolumn = '0',
  foldenable = false,
  winhighlight = 'Normal:KittyMarkdownText',
}

local ns = api.nvim_create_namespace('kitty-markdown-preview')
local previews = {}
local augroup = nil
-- `images` or `text` once the first probe answers; nil before that. Every
-- preview shares the terminal, so the mode is global (R-131).
local mode = nil
local probing = false

local function notify(msg, level)
  vim.notify(msg, level or vim.log.levels.INFO)
end

-- Text helpers -------------------------------------------------------------

-- Cut `text` to at most `width` cells on a character boundary.
local function truncate(text, width)
  text = text:gsub('\t', '  ')
  if strwidth(text) <= width then
    return text
  end
  local out, used = {}, 0
  for _, ch in ipairs(vim.fn.split(text, '\\zs')) do
    local w = strwidth(ch)
    if used + w > width then
      break
    end
    out[#out + 1] = ch
    used = used + w
  end
  return table.concat(out)
end

-- Cut or pad `text` to exactly `width` cells.
local function fit(text, width)
  text = truncate(text, width)
  return text .. string.rep(' ', math.max(0, width - strwidth(text)))
end

local function title_of(desc)
  if desc.alt and desc.alt ~= '' then
    return desc.alt
  end
  return desc.kind == 'mermaid' and 'mermaid' or 'image'
end

-- One bordered box as virtual lines: a top border with an optional icon
-- and the title, one body row per entry of `body`, and a bottom border.
-- Every row starts with the 2-cell margin of the text (R-020, R-120,
-- R-121, R-169).
local function box_rows(width, title, icon, icon_group, body, body_group)
  width = math.max(8, width)
  local top = { { MARGIN, 'Normal' }, { '╭─ ', BORDER } }
  local used = 3
  if icon then
    top[#top + 1] = { icon, icon_group }
    top[#top + 1] = { ' ', BORDER }
    used = used + strwidth(icon) + 1
  end
  local label = truncate(title, math.max(0, width - used - 3))
  top[#top + 1] = { label, BOX_TEXT }
  used = used + strwidth(label)
  top[#top + 1] = { ' ' .. string.rep('─', math.max(0, width - used - 2)) .. '╮', BORDER }
  local rows = { top }
  for _, line in ipairs(body) do
    rows[#rows + 1] = {
      { MARGIN, 'Normal' },
      { '│ ', BORDER },
      { fit(line, width - 4), body_group },
      { ' │', BORDER },
    }
  end
  rows[#rows + 1] =
    { { MARGIN, 'Normal' }, { '╰' .. string.rep('─', width - 2) .. '╯', BORDER } }
  return rows
end

local function reason_of(err)
  local data = type(err.data) == 'table' and err.data or {}
  if err.code == 'parse_error' then
    return 'Parse error on line ' .. tostring(data.line)
  elseif err.code == 'unsupported_feature' then
    return 'unsupported: ' .. tostring(data.feature)
  end
  return REASONS[err.code] or 'helper error'
end

-- State lookup -------------------------------------------------------------

-- A command run inside the preview window names the preview buffer, not
-- the source, so both resolve to the same state.
local function resolve(buf)
  if previews[buf] then
    return previews[buf]
  end
  for _, p in pairs(previews) do
    if p.buf == buf then
      return p
    end
  end
  return nil
end

-- A result applies only when the preview, its render generation, and the
-- block entry are the ones the request was made for.
local function live(p, b, gen)
  return previews[p.src_buf] == p
    and p.generation == gen
    and p.blocks[b.key] == b
    and api.nvim_buf_is_valid(p.buf)
end

-- Block rows ----------------------------------------------------------------

local function set_rows(p, b, rows)
  local line = b.desc.line
  if line > api.nvim_buf_line_count(p.buf) then
    return
  end
  b.extmark_id = api.nvim_buf_set_extmark(p.buf, ns, line - 1, 0, {
    id = b.extmark_id,
    virt_lines = rows,
    strict = false,
  })
end

local function show_loading_box(p, b)
  local desc = b.desc
  set_rows(
    p,
    b,
    box_rows(
      desc.width_cells,
      title_of(desc),
      icons.loading,
      'KittyMarkdownLoading',
      { 'loading' },
      MUTED
    )
  )
end

local function show_failure_box(p, b)
  local desc = b.desc
  set_rows(
    p,
    b,
    box_rows(
      desc.width_cells,
      title_of(desc),
      icons.failure,
      'KittyMarkdownFailure',
      { b.reason },
      MUTED
    )
  )
end

-- Text-only box: the alt text for an image, the source for a diagram
-- (R-120, R-121). An image without alt text shows its path instead, so
-- the box is never empty.
local function show_text_box(p, b)
  local desc = b.desc
  local body
  if desc.kind == 'mermaid' then
    body = vim.split(desc.source, '\n', { plain = true })
    while #body > 0 and body[#body] == '' do
      body[#body] = nil
    end
    if #body == 0 then
      body = { '' }
    end
  else
    body = { (desc.alt and desc.alt ~= '') and desc.alt or desc.source }
  end
  set_rows(p, b, box_rows(desc.width_cells, title_of(desc), nil, nil, body, BOX_TEXT))
end

-- The `:messages` text of a failed image names the path or URL it was
-- for (R-288). Helper messages such as "unsupported image format: unknown"
-- carry only the cause, so the target goes in front of them.
local function message_of(desc, err)
  local msg = tostring(err.message or '')
  if desc.kind ~= 'image' or desc.source == '' or msg:find(desc.source, 1, true) then
    return msg
  end
  if msg == '' then
    return desc.source
  end
  return desc.source .. ': ' .. msg
end

local function fail_block(p, b, err)
  local desc = b.desc
  b.reason = reason_of(err)
  if err.code == 'no_helper' then
    -- The helper module already reported the missing binary with the
    -- build command (R-134); the block falls back to its text form.
    b.state = 'text'
    show_text_box(p, b)
    return
  end
  b.state = 'failed'
  show_failure_box(p, b)
  if not QUIET_CODES[err.code] then
    notify(
      string.format(
        'kitty-markdown: %s %s: %s: %s',
        desc.kind,
        title_of(desc),
        tostring(err.code),
        message_of(desc, err)
      ),
      vim.log.levels.ERROR
    )
  end
end

-- Hang the placeholder rows of a placed block below its anchor line. The
-- image data and the placement already exist in the terminal.
local function attach_rows(p, b)
  local lines = transport.placeholder_rows(b.image_id, b.cols, b.rows)
  local group = transport.hl_group(b.image_id)
  local rows = {}
  for i, text in ipairs(lines) do
    rows[i] = { { MARGIN, 'Normal' }, { text, group } }
  end
  set_rows(p, b, rows)
end

-- Make the virtual placement for a transmitted image and attach its rows.
-- The grid follows the pixel size, so a picture narrower than the text
-- width keeps its aspect ratio (R-090, R-093).
local function place_block(p, b)
  local cell = p.cell
  local rows = math.ceil(b.height_px / cell.height_px)
  local cols = math.min(b.desc.width_cells, math.max(1, math.ceil(b.width_px / cell.width_px)))
  local _, reason = transport.placeholder_rows(b.image_id, cols, rows)
  if reason then
    transport.delete(b.image_id)
    fail_block(p, b, {
      code = 'too_tall',
      message = string.format('%d rows exceed the placeholder limit', rows),
    })
    return
  end
  transport.place(b.image_id, cols, rows)
  b.rows, b.cols = rows, cols
  b.state = 'placed'
  attach_rows(p, b)
end

local function note_result(result)
  if type(result) == 'table' and result.cached == false and type(result.bytes) == 'number' then
    cache.note_bytes(result.bytes)
  end
end

local function succeed(p, b, result)
  b.image_id = b.image_id or transport.alloc_id()
  b.path = result.path
  b.width_px = result.width_px
  b.height_px = result.height_px
  local ok, reason = transport.transmit(b.image_id, b.path)
  if ok == nil then
    fail_block(p, b, { code = 'io', message = reason .. ': ' .. tostring(b.path) })
    return
  end
  place_block(p, b)
end

-- One render batch asks the cache for garbage collection once, after the
-- last request of the batch settled (R-202).
local function settle(p, gen)
  local batch = p.batch
  if not batch or batch.gen ~= gen then
    return
  end
  batch.outstanding = batch.outstanding - 1
  if batch.outstanding == 0 then
    cache.gc_if_needed()
  end
end

-- The URL scheme of an image source, lower case, or nil for a path. The
-- pattern is the RFC 3986 scheme syntax the layout uses to leave a URL
-- unresolved, so both agree on what is a URL.
local function scheme_of(source)
  local scheme = source:match('^(%a[%w+.-]*):')
  return scheme and scheme:lower() or nil
end

local function request_block(p, b, gen)
  local desc = b.desc
  b.fetched = nil
  local width_px = math.floor(desc.width_cells * p.cell.width_px)
  local function done(err, result)
    if not live(p, b, gen) then
      return
    end
    if err then
      fail_block(p, b, err)
    else
      note_result(result)
      succeed(p, b, result)
    end
    settle(p, gen)
  end
  if desc.kind == 'mermaid' then
    helper.request('mermaid.render', {
      source = desc.source,
      width_px = width_px,
      theme = theme.mermaid_theme(),
      -- Transparent, as GitHub renders mermaid: the terminal background
      -- shows through, so one cache entry serves both palettes and the
      -- parity check (SSIM composites over white) still matches the
      -- oracle's white goldens.
      background = 'transparent',
    }, done)
    return
  end
  local function prepare(path)
    local params = { path = path, width_px = width_px }
    local attrs = desc.attrs or {}
    if attrs.width then
      params.max_width_px = attrs.width
    end
    if attrs.height then
      params.max_height_px = attrs.height
    end
    helper.request('image.prepare', params, done)
  end
  local scheme = scheme_of(desc.source)
  if not scheme then
    prepare(desc.source)
    return
  end
  if scheme ~= 'http' and scheme ~= 'https' then
    -- Only http and https are fetched. A `file://` URL is not read as a
    -- local path either, so it never reaches the helper or the remote
    -- cache (R-225, R-287). Scheduled for the batch counter, as below.
    vim.schedule(function()
      done({
        code = 'fetch_failed',
        message = string.format('unsupported URL scheme "%s"', scheme),
        data = { status = 0 },
      })
    end)
    return
  end
  local remote = config.get().remote_images
  if not remote.enabled then
    -- Scheduled so the batch counter sees every block before the first
    -- one settles.
    vim.schedule(function()
      done({ code = 'remote_images_off', message = 'remote images are off' })
    end)
    return
  end
  helper.request('fetch', {
    url = desc.source,
    timeout_ms = remote.timeout_ms,
    max_bytes = remote.max_bytes,
  }, function(err, result)
    if not live(p, b, gen) then
      return
    end
    if err then
      fail_block(p, b, err)
      settle(p, gen)
      return
    end
    note_result(result)
    b.fetched = result.path
    prepare(result.path)
  end)
end

-- Buffer fill ------------------------------------------------------------

local function set_lines(p, lines)
  vim.bo[p.buf].modifiable = true
  api.nvim_buf_set_lines(p.buf, 0, -1, false, lines)
  vim.bo[p.buf].modifiable = false
end

-- Apply the layout highlights. Returns the byte column where the code text
-- starts on each code line, read from its background span, so the
-- tree-sitter pass can place captures in the right column.
local function apply_highlights(p, model)
  local code_offsets = {}
  for _, hl in ipairs(model.highlights) do
    api.nvim_buf_set_extmark(p.buf, ns, hl.line - 1, hl.col_start, {
      end_col = hl.col_end,
      hl_group = hl.group,
      priority = HL_PRIORITY,
      strict = false,
    })
    if hl.group == 'KittyMarkdownCodeBg' and code_offsets[hl.line] == nil then
      code_offsets[hl.line] = hl.col_start
    end
  end
  return code_offsets
end

local function parser_for(lang)
  if not lang or lang == '' then
    return nil
  end
  local resolved = vim.treesitter.language.get_lang(lang) or lang
  local ok, added = pcall(vim.treesitter.language.add, resolved)
  if not ok or added ~= true then
    return nil
  end
  local ok_q, query = pcall(vim.treesitter.query.get, resolved, 'highlights')
  if not ok_q or not query then
    return nil
  end
  return resolved, query
end

local SKIP_CAPTURES = { spell = true, nospell = true, conceal = true }

-- Tree-sitter token colors for each fenced code block whose language has a
-- parser (R-047). A block without one keeps the plain code group (R-048).
local function highlight_code(p, model, code_offsets)
  for _, region in ipairs(model.code_regions) do
    local lang, query = parser_for(region.lang)
    if lang then
      local pieces = {}
      for ln = region.line_start, region.line_end do
        pieces[#pieces + 1] = model.lines[ln]:sub((code_offsets[ln] or 0) + 1)
      end
      local code = table.concat(pieces, '\n')
      local ok, parser = pcall(vim.treesitter.get_string_parser, code, lang)
      if ok and parser then
        local trees = parser:parse()
        local root = trees and trees[1] and trees[1]:root()
        if root then
          for id, node in query:iter_captures(root, code) do
            local name = query.captures[id]
            if not SKIP_CAPTURES[name] and name:sub(1, 1) ~= '_' then
              local sr, sc, er, ec = node:range()
              local start_line = region.line_start + sr
              local end_line = region.line_start + er
              api.nvim_buf_set_extmark(
                p.buf,
                ns,
                start_line - 1,
                sc + (code_offsets[start_line] or 0),
                {
                  end_row = end_line - 1,
                  end_col = ec + (code_offsets[end_line] or 0),
                  hl_group = '@' .. name .. '.' .. lang,
                  priority = CODE_PRIORITY,
                  strict = false,
                }
              )
            end
          end
        end
      end
    end
  end
end

local function restore_cursor(p, cursor, line_count)
  if not api.nvim_win_is_valid(p.win) then
    return
  end
  local line = math.max(1, math.min(cursor[1], line_count))
  local text = api.nvim_buf_get_lines(p.buf, line - 1, line, false)[1] or ''
  local col = math.max(0, math.min(cursor[2], #text))
  api.nvim_win_set_cursor(p.win, { line, col })
end

-- Pixel blocks by key: an unchanged placed block keeps its image and only
-- gets its rows back (R-026, R-170); every other block gets a loading box
-- and a request; a block that vanished loses its image (R-128).
local function place_blocks(p, model, gen, opts)
  local batch = { gen = gen, outstanding = 0 }
  p.batch = batch
  local seen = {}
  for _, desc in ipairs(model.blocks) do
    seen[desc.key] = true
    local b = p.blocks[desc.key]
    if not b then
      b = { key = desc.key }
      p.blocks[desc.key] = b
    end
    b.desc = desc
    b.extmark_id = nil
    if desc.text_only then
      if b.state == 'placed' then
        transport.delete(b.image_id)
      end
      b.state = 'text'
      show_text_box(p, b)
    elseif b.state == 'placed' then
      if opts.force then
        -- A reattach may have lost the terminal's copy (R-130).
        transport.transmit(b.image_id, b.path, true)
        transport.place(b.image_id, b.cols, b.rows)
      end
      attach_rows(p, b)
    else
      b.state = 'loading'
      show_loading_box(p, b)
      batch.outstanding = batch.outstanding + 1
      request_block(p, b, gen)
    end
  end
  for key, b in pairs(p.blocks) do
    if not seen[key] then
      if b.state == 'placed' and b.image_id then
        transport.delete(b.image_id)
      end
      p.blocks[key] = nil
    end
  end
end

-- The source line range of each top-level block, in document order, so
-- scroll sync can find the block under the cursor (R-248).
local function source_blocks(blocks)
  local out = {}
  for _, block in ipairs(blocks) do
    if block.src_start then
      out[#out + 1] = { block.src_start, block.src_end or block.src_start }
    end
  end
  return out
end

local function do_render(p, opts)
  opts = opts or {}
  if not api.nvim_buf_is_valid(p.src_buf) or not api.nvim_buf_is_valid(p.buf) then
    return false
  end
  if not api.nvim_win_is_valid(p.win) then
    return false
  end
  p.generation = p.generation + 1
  local gen = p.generation
  local images = mode == 'images'
  local cell = images and transport.cell_size() or nil
  if images and not cell then
    images = false
  end
  p.cell = cell
  p.mode = images and 'images' or 'text'
  local sizes = {}
  for key, b in pairs(p.blocks) do
    if b.state == 'placed' then
      sizes[key] = { width_px = b.width_px, height_px = b.height_px, image_id = b.image_id }
    end
  end
  local blocks = require('kitty-markdown.parser').parse(p.src_buf)
  p.source_blocks = source_blocks(blocks)
  local model = require('kitty-markdown.layout').build(blocks, {
    width = api.nvim_win_get_width(p.win),
    cell = cell,
    sizes = sizes,
    theme = theme.current(),
    images = images,
  })
  p.model = model
  local cursor = api.nvim_win_get_cursor(p.win)
  set_lines(p, model.lines)
  api.nvim_buf_clear_namespace(p.buf, ns, 0, -1)
  local code_offsets = apply_highlights(p, model)
  highlight_code(p, model, code_offsets)
  restore_cursor(p, cursor, #model.lines)
  place_blocks(p, model, gen, opts)
  return true
end

-- Probe ----------------------------------------------------------------------

local function start_probe()
  if probing then
    return
  end
  probing = true
  transport.probe(function(result)
    probing = false
    local previous = mode
    mode = result.images and 'images' or 'text'
    if mode ~= previous then
      if mode == 'images' then
        notify('kitty-markdown: kitty graphics on, images render inline')
      else
        notify('kitty-markdown: text-only mode: ' .. tostring(result.reason))
      end
    end
    -- Before the first answer every preview rendered as text, so only a
    -- change of mode needs a new render (R-131).
    if mode ~= (previous or 'text') then
      M.render_all()
    end
  end)
end

-- Scroll sync --------------------------------------------------------------

-- The first source line of the top-level block under `src_line`. A line
-- outside every block, such as a blank line between two, belongs to the
-- block above it (R-248, R-340). The first block that holds the line wins,
-- so a paragraph comes before the image lifted out of it.
local function block_start(ranges, src_line)
  local above, above_end = nil, nil
  for _, r in ipairs(ranges) do
    if r[1] <= src_line and src_line <= r[2] then
      return r[1]
    end
    if r[2] < src_line and (above_end == nil or r[2] > above_end) then
      above, above_end = r[1], r[2]
    end
  end
  return above or src_line
end

--- The preview line that starts the block under source line `src_line`.
--- `ranges` (optional) are the source line ranges of the top-level blocks;
--- with them a line inside a block maps to the start of that block. The
--- last preview line whose source line is at most that line sits in the
--- block; the result is the first line of its run, less the blank
--- separator that precedes a block and carries its source line.
function M.sync_target(model, src_line, ranges)
  if ranges then
    src_line = block_start(ranges, src_line)
  end
  local source_lines = model.source_lines
  local lo, hi, best = 1, #source_lines, 1
  while lo <= hi do
    local mid = math.floor((lo + hi) / 2)
    if source_lines[mid] <= src_line then
      best = mid
      lo = mid + 1
    else
      hi = mid - 1
    end
  end
  local first = best
  while first > 1 and source_lines[first - 1] == source_lines[best] do
    first = first - 1
  end
  if first < best and model.lines[first] == '' then
    local is_anchor = false
    for _, desc in ipairs(model.blocks or {}) do
      if desc.line == first then
        is_anchor = true
        break
      end
    end
    if not is_anchor then
      first = first + 1
    end
  end
  return first
end

--- The top line that puts `target` in the top third of a window of
--- `height` rows (R-211).
function M.sync_topline(target, height)
  return math.max(1, target - math.floor(height / 3))
end

--- Scroll the preview of `src_buf` to the block under source line
--- `src_line`. Returns the preview line it moved to, or nil.
function M.sync(src_buf, src_line)
  local p = resolve(src_buf)
  if not p or not p.model or not api.nvim_win_is_valid(p.win) then
    return nil
  end
  local target = M.sync_target(p.model, src_line, p.source_blocks)
  local topline = M.sync_topline(target, api.nvim_win_get_height(p.win))
  api.nvim_win_call(p.win, function()
    vim.fn.winrestview({ topline = topline, lnum = target, col = 0 })
  end)
  return target
end

local function schedule_sync(p, src_win)
  if not p.sync_timer then
    return
  end
  p.sync_timer:stop()
  p.sync_timer:start(
    SYNC_DEBOUNCE_MS,
    0,
    vim.schedule_wrap(function()
      if previews[p.src_buf] ~= p or not api.nvim_win_is_valid(src_win) then
        return
      end
      if api.nvim_win_get_buf(src_win) ~= p.src_buf then
        return
      end
      M.sync(p.src_buf, api.nvim_win_get_cursor(src_win)[1])
    end)
  )
end

-- Teardown -------------------------------------------------------------------

-- True when `win` is the only window of its tab page that is not a
-- float, so closing it closes the tab page (R-252).
local function last_in_tab(win)
  if api.nvim_win_get_config(win).relative ~= '' then
    return false
  end
  for _, w in ipairs(api.nvim_tabpage_list_wins(api.nvim_win_get_tabpage(win))) do
    if w ~= win and api.nvim_win_get_config(w).relative == '' then
      return false
    end
  end
  return true
end

-- Keep the only window of the only tab page open on another buffer: the
-- source when it is still loaded (R-346), else a new empty buffer (R-347).
-- The window options of the preview go back to their global values.
local function release_window(p)
  local other = p.src_buf
  if not (api.nvim_buf_is_valid(other) and api.nvim_buf_is_loaded(other)) then
    other = api.nvim_create_buf(true, false)
  end
  api.nvim_win_set_buf(p.win, other)
  for name in pairs(WINDOW_OPTIONS) do
    local value = api.nvim_get_option_value(name, { scope = 'global' })
    api.nvim_set_option_value(name, value, { win = p.win, scope = 'local' })
  end
end

-- Drop the state of a preview and free its images. The window and buffer
-- go now, or on the next loop iteration when an autocommand asked, since
-- a window cannot close from inside its own WinClosed.
local function teardown(p, deferred)
  if previews[p.src_buf] == p then
    previews[p.src_buf] = nil
  end
  if p.sync_timer then
    p.sync_timer:stop()
    if not p.sync_timer:is_closing() then
      p.sync_timer:close()
    end
    p.sync_timer = nil
  end
  for _, b in pairs(p.blocks) do
    if b.state == 'placed' and b.image_id then
      transport.delete(b.image_id)
    end
  end
  p.blocks = {}
  local function cleanup()
    if api.nvim_win_is_valid(p.win) then
      if last_in_tab(p.win) and #api.nvim_list_tabpages() == 1 then
        pcall(release_window, p)
      elseif last_in_tab(p.win) then
        -- Floats would keep the window from closing; `tabclose` takes
        -- them with the tab page (R-252).
        local tab = api.nvim_win_get_tabpage(p.win)
        if not pcall(api.nvim_win_close, p.win, true) then
          pcall(vim.cmd.tabclose, api.nvim_tabpage_get_number(tab))
        end
      else
        pcall(api.nvim_win_close, p.win, true)
      end
    end
    if api.nvim_buf_is_valid(p.buf) then
      pcall(api.nvim_buf_delete, p.buf, { force = true })
    end
  end
  if deferred then
    vim.schedule(cleanup)
  else
    cleanup()
  end
end

-- Autocommands ---------------------------------------------------------------

local function ensure_autocmds()
  if augroup then
    return
  end
  augroup = api.nvim_create_augroup(AUGROUP, { clear = true })
  api.nvim_create_autocmd('BufWritePost', {
    group = augroup,
    callback = function(ev)
      local p = previews[ev.buf]
      if p then
        do_render(p)
      end
    end,
  })
  api.nvim_create_autocmd({ 'BufUnload', 'BufWipeout', 'BufDelete' }, {
    group = augroup,
    callback = function(ev)
      local p = previews[ev.buf]
      if p then
        teardown(p, true)
        return
      end
      for _, q in pairs(previews) do
        if q.buf == ev.buf then
          teardown(q, true)
        end
      end
    end,
  })
  api.nvim_create_autocmd('WinClosed', {
    group = augroup,
    callback = function(ev)
      local win = tonumber(ev.match)
      for _, q in pairs(previews) do
        if q.win == win then
          teardown(q, true)
        end
      end
    end,
  })
  api.nvim_create_autocmd('OptionSet', {
    group = augroup,
    pattern = 'background',
    callback = function()
      M.retheme()
    end,
  })
  api.nvim_create_autocmd('VimResized', {
    group = augroup,
    callback = function()
      M.render_all()
    end,
  })
  api.nvim_create_autocmd('CursorMoved', {
    group = augroup,
    callback = function(ev)
      local p = previews[ev.buf]
      if p then
        schedule_sync(p, api.nvim_get_current_win())
      end
    end,
  })
  api.nvim_create_autocmd('WinScrolled', {
    group = augroup,
    callback = function()
      local scrolled = vim.v.event or {}
      for _, p in pairs(previews) do
        local win = vim.fn.bufwinid(p.src_buf)
        if win ~= -1 and scrolled[tostring(win)] then
          schedule_sync(p, win)
        end
      end
    end,
  })
end

-- Buffer and window ----------------------------------------------------------

local function create_buffer(src_buf)
  local buf = api.nvim_create_buf(false, true)
  local name = 'kitty-markdown://' .. api.nvim_buf_get_name(src_buf)
  if not pcall(api.nvim_buf_set_name, buf, name) then
    -- Two unnamed sources would share a name; the buffer number keeps the
    -- second one distinct.
    pcall(api.nvim_buf_set_name, buf, name .. '#' .. buf)
  end
  local bo = vim.bo[buf]
  bo.buftype = 'nofile'
  bo.bufhidden = 'wipe'
  bo.swapfile = false
  bo.undolevels = -1
  bo.filetype = 'kitty-markdown'
  bo.modifiable = false
  return buf
end

local function open_window(buf, placement)
  local win
  if placement == 'split' then
    -- The source window keeps the focus, so the cursor that drives scroll
    -- sync stays where the user left it.
    win = api.nvim_open_win(buf, false, { split = 'right', win = 0 })
  elseif placement == 'float' then
    local width = math.max(1, math.min(config.get().text_width + 4, vim.o.columns - 4))
    local height = math.max(1, vim.o.lines - 4)
    win = api.nvim_open_win(buf, true, {
      relative = 'editor',
      width = width,
      height = height,
      row = 1,
      col = math.max(0, math.floor((vim.o.columns - width) / 2)),
      border = 'rounded',
    })
  else
    vim.cmd(string.format('tab sbuffer %d', buf))
    win = api.nvim_get_current_win()
  end
  for name, value in pairs(WINDOW_OPTIONS) do
    api.nvim_set_option_value(name, value, { win = win, scope = 'local' })
  end
  return win
end

local function set_keys(p)
  vim.keymap.set('n', 'q', function()
    M.close(p.src_buf)
  end, { buffer = p.buf, nowait = true, desc = 'kitty-markdown: close the preview' })
  vim.keymap.set('n', 'gx', function()
    M.follow(p.src_buf)
  end, { buffer = p.buf, nowait = true, desc = 'kitty-markdown: open the link under the cursor' })
end

-- Public API -------------------------------------------------------------------

--- Open a preview of `src_buf` with `placement` (`split`, `tab`, or
--- `float`; the configured one when nil). An existing preview is focused
--- instead (R-002, R-006, R-007). Returns the preview state, or nil when
--- the buffer is not markdown (R-012, R-165).
function M.open(src_buf, placement)
  src_buf = src_buf or api.nvim_get_current_buf()
  if not api.nvim_buf_is_valid(src_buf) then
    return nil
  end
  local ft = vim.bo[src_buf].filetype
  if ft ~= 'markdown' then
    notify(
      string.format('kitty-markdown: not a markdown buffer (filetype is "%s")', ft),
      vim.log.levels.ERROR
    )
    return nil
  end
  local existing = previews[src_buf]
  if existing then
    if api.nvim_win_is_valid(existing.win) then
      api.nvim_set_current_win(existing.win)
    end
    return existing
  end
  placement = placement or config.get().placement
  ensure_autocmds()
  theme.apply()
  local p = {
    src_buf = src_buf,
    buf = create_buffer(src_buf),
    placement = placement,
    mode = 'text',
    blocks = {},
    generation = 0,
  }
  previews[src_buf] = p
  p.win = open_window(p.buf, placement)
  set_keys(p)
  if placement == 'split' and config.get().scroll_sync then
    p.sync_timer = vim.uv.new_timer()
  end
  if mode == nil then
    start_probe()
  end
  do_render(p)
  return p
end

--- Close the preview of `src_buf` and delete its images (R-008). Returns
--- true when a preview existed.
function M.close(src_buf)
  local p = resolve(src_buf or api.nvim_get_current_buf())
  if not p then
    return false
  end
  teardown(p, false)
  return true
end

--- Close an existing preview or open one (R-009, R-010).
function M.toggle(src_buf, placement)
  src_buf = src_buf or api.nvim_get_current_buf()
  if resolve(src_buf) then
    M.close(src_buf)
    return nil
  end
  return M.open(src_buf, placement)
end

--- The preview state of `src_buf`, or nil.
function M.get(src_buf)
  return resolve(src_buf or api.nvim_get_current_buf())
end

-- Send the data and the placement of every placed image of `p` again,
-- since a reattach may have lost the terminal's copy (R-130).
local function retransmit(p)
  for _, b in pairs(p.blocks) do
    if b.state == 'placed' and b.image_id and b.path then
      transport.transmit(b.image_id, b.path, true)
      transport.place(b.image_id, b.cols, b.rows)
    end
  end
end

--- Render the preview of `src_buf` again. With `opts.reprobe` the helper
--- restart stop is cleared and the terminal is probed again, also when
--- `src_buf` has no preview, and every placed image of every open preview
--- is transmitted again (R-017, R-130, R-310). Returns true when a
--- preview of `src_buf` existed.
function M.render(src_buf, opts)
  opts = opts or {}
  local p = resolve(src_buf or api.nvim_get_current_buf())
  if opts.reprobe then
    helper.reset_failures()
    start_probe()
    for _, q in pairs(previews) do
      if q ~= p then
        retransmit(q)
      end
    end
  end
  if not p then
    return false
  end
  return do_render(p, { force = opts.reprobe })
end

--- Render every open preview again.
function M.render_all(opts)
  for _, p in pairs(previews) do
    do_render(p, opts)
  end
end

--- Define the highlight groups for the current palette and render every
--- preview again (R-075).
function M.retheme()
  theme.apply()
  M.render_all()
end

--- The cache paths of every placed image and of the fetched file of each
--- placed remote image, which `cache.gc` keeps (R-152, R-328).
function M.in_use_paths()
  local out = {}
  for _, p in pairs(previews) do
    for _, b in pairs(p.blocks) do
      if b.state == 'placed' and b.path then
        out[#out + 1] = b.path
        -- The fetched original of a remote image stays too, so the next
        -- render finds it in `remote` and does not fetch again (R-328).
        if b.fetched and b.fetched ~= b.path then
          out[#out + 1] = b.fetched
        end
      end
    end
  end
  table.sort(out)
  return out
end

-- The link under `col` on `line`: a span that contains the column, or the
-- empty span of an image anchor line.
local function link_at(model, line, col)
  local fallback = nil
  for _, link in ipairs(model.links) do
    if link.line == line then
      if link.col_start <= col and col < link.col_end then
        return link
      end
      if link.col_start == link.col_end and not fallback then
        fallback = link
      end
    end
  end
  return fallback
end

--- Follow the link under the preview cursor: a URL, file, or image goes
--- to the system opener, an anchor scrolls the preview (R-159 to R-161).
--- Returns the link, or nil when the cursor is not on one.
function M.follow(src_buf)
  local p = resolve(src_buf or api.nvim_get_current_buf())
  if not p or not p.model or not api.nvim_win_is_valid(p.win) then
    return nil
  end
  local cursor = api.nvim_win_get_cursor(p.win)
  local link = link_at(p.model, cursor[1], cursor[2])
  if not link then
    return nil
  end
  if link.kind == 'anchor' then
    local target = p.model.anchors[link.target]
    if target then
      api.nvim_win_set_cursor(p.win, { target, 0 })
    end
    return link
  end
  vim.ui.open(link.target)
  return link
end

--- The probe outcome shared by every preview: `images`, `text`, or nil
--- before the first answer.
function M.mode()
  return mode
end

--- Test hook: close every preview and forget the probe result.
function M.reset()
  for _, p in pairs(previews) do
    teardown(p, false)
  end
  previews = {}
  mode = nil
  probing = false
  if augroup then
    api.nvim_del_augroup_by_id(augroup)
    augroup = nil
  end
end

return M
