-- Helpers for the mermaid checks: pixel reads through a small Python PNG
-- decoder (independent of the helper), helper renders, oracle fixtures, and
-- an end-to-end preview driver with a faked kitty terminal.

local QA = require('qa')

local M = {}

M.dir = QA.evals .. '/mermaid'

-- Python PNG tool --------------------------------------------------------------

local PY = [==[
import json, struct, sys, zlib

def load(path):
    d = open(path, 'rb').read()
    assert d[:8] == b'\x89PNG\r\n\x1a\n', 'not a png'
    pos, idat, plte, trns = 8, b'', None, None
    while pos < len(d):
        n = struct.unpack('>I', d[pos:pos+4])[0]
        t = d[pos+4:pos+8]
        c = d[pos+8:pos+8+n]
        pos += 12 + n
        if t == b'IHDR':
            w, h, depth, ct, _, _, il = struct.unpack('>IIBBBBB', c)
        elif t == b'IDAT':
            idat += c
        elif t == b'PLTE':
            plte = c
        elif t == b'tRNS':
            trns = c
    assert depth == 8 and il == 0, 'only 8-bit non-interlaced'
    ch = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ct]
    raw = zlib.decompress(idat)
    stride = w * ch
    out = bytearray(h * stride)
    prev = bytearray(stride)
    i = 0
    for y in range(h):
        f = raw[i]; i += 1
        line = bytearray(raw[i:i+stride]); i += stride
        for x in range(stride):
            a = line[x-ch] if x >= ch else 0
            b = prev[x]
            c = prev[x-ch] if x >= ch else 0
            if f == 1: line[x] = (line[x] + a) & 255
            elif f == 2: line[x] = (line[x] + b) & 255
            elif f == 3: line[x] = (line[x] + ((a + b) >> 1)) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[x] = (line[x] + pr) & 255
        out[y*stride:(y+1)*stride] = line
        prev = line
    px = []
    for k in range(w * h):
        s = out[k*ch:(k+1)*ch]
        if ct == 6: px.append(tuple(s))
        elif ct == 2: px.append((s[0], s[1], s[2], 255))
        elif ct == 0: px.append((s[0], s[0], s[0], 255))
        elif ct == 4: px.append((s[0], s[0], s[0], s[1]))
        else:
            j = s[0]
            al = trns[j] if trns and j < len(trns) else 255
            px.append((plte[3*j], plte[3*j+1], plte[3*j+2], al))
    return w, h, px

def near(p, rgb, tol):
    return p[3] > 200 and max(abs(p[0]-rgb[0]), abs(p[1]-rgb[1]), abs(p[2]-rgb[2])) <= tol

def hexrgb(s):
    s = s.lstrip('#')
    return (int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16))

cmd = sys.argv[1]
if cmd == 'info':
    w, h, px = load(sys.argv[2])
    corners = [px[0], px[w-1], px[(h-1)*w], px[h*w-1]]
    counts = {}
    opaque = 0
    for p in px:
        if p[3] > 0:
            opaque += 1
        if p[3] > 200:
            k = '#%02x%02x%02x' % p[:3]
            counts[k] = counts.get(k, 0) + 1
    top = sorted(counts.items(), key=lambda kv: -kv[1])[:12]
    print(json.dumps({'w': w, 'h': h, 'corners': [list(c) for c in corners],
                      'transparent': sum(1 for p in px if p[3] == 0),
                      'opaque': opaque, 'top': [[k, v] for k, v in top]}))
elif cmd == 'count':
    w, h, px = load(sys.argv[2])
    rgb, tol = hexrgb(sys.argv[3]), int(sys.argv[4])
    print(json.dumps({'count': sum(1 for p in px if near(p, rgb, tol))}))
elif cmd == 'same':
    w1, h1, a = load(sys.argv[2])
    w2, h2, b = load(sys.argv[3])
    if (w1, h1) != (w2, h2):
        print(json.dumps({'same_size': False, 'fraction': 0}))
    else:
        print(json.dumps({'same_size': True, 'fraction': sum(1 for x, y in zip(a, b) if x == y) / float(len(a))}))
elif cmd == 'masked':
    # Pixels of `variant` near `color` form the mask. Report the mask size,
    # how many base pixels under the mask share one color, and the share of
    # visible base pixels outside the mask (grown by `margin`) that stay equal.
    w1, h1, a = load(sys.argv[2])
    w2, h2, b = load(sys.argv[3])
    rgb, tol, margin = hexrgb(sys.argv[4]), int(sys.argv[5]), int(sys.argv[6])
    if (w1, h1) != (w2, h2):
        print(json.dumps({'same_size': False}))
        sys.exit(0)
    w, h = w1, h1
    mask = [near(p, rgb, tol) for p in b]
    grown = bytearray(w * h)
    for y in range(h):
        for x in range(w):
            if mask[y*w+x]:
                for yy in range(max(0, y-margin), min(h, y+margin+1)):
                    for xx in range(max(0, x-margin), min(w, x+margin+1)):
                        grown[yy*w+xx] = 1
    under = {}
    for k in range(w*h):
        if mask[k]:
            key = '#%02x%02x%02x%02x' % a[k]
            under[key] = under.get(key, 0) + 1
    total = same = 0
    for k in range(w*h):
        if not grown[k] and (a[k][3] > 0 or b[k][3] > 0):
            total += 1
            if a[k] == b[k]:
                same += 1
    top_under = max(under.values()) if under else 0
    print(json.dumps({'same_size': True, 'mask': sum(mask), 'mask_top_base': top_under,
                      'outside': total, 'outside_same': same}))
]==]

function M.python()
  QA.need(vim.fn.executable('python3') == 1, 'python3 for PNG pixel reads')
  return 'python3'
end

local function py(args)
  local res = QA.sh(vim.list_extend({ M.python(), '-I', '-c', PY }, args), { timeout = 60000 })
  assert(res.code == 0, 'png tool failed: ' .. tostring(res.stderr))
  return vim.json.decode(res.stdout)
end

function M.png_info(path)
  return py({ 'info', path })
end

function M.count_near(path, hex, tol)
  return py({ 'count', path, hex, tostring(tol or 8) }).count
end

-- Share of identical pixels (0 when the sizes differ).
function M.same_fraction(a, b)
  return py({ 'same', a, b }).fraction
end

function M.masked(base, variant, hex, tol, margin)
  return py({ 'masked', base, variant, hex, tostring(tol or 10), tostring(margin or 3) })
end

-- Fixtures ----------------------------------------------------------------------

function M.source(rel)
  local s = QA.read(M.dir .. '/' .. rel .. '.mmd')
  assert(s, 'missing fixture ' .. rel)
  return s
end

local sizes
function M.oracle_size(rel)
  sizes = sizes or dofile(M.dir .. '/oracle_sizes.lua')
  return sizes[rel]
end

-- The oracle's parse error text for a parse fixture (lines of the message).
function M.oracle_error(name)
  local s = QA.read(M.dir .. '/parse/' .. name .. '.oracle.txt')
  assert(s, 'missing oracle text ' .. name)
  local lines = vim.split((s:gsub('\n+$', '')), '\n', { plain = true })
  return lines, tonumber(lines[1]:match('^Parse error on line (%d+):'))
end

function M.parse_cases()
  local out = {}
  for name, t in vim.fs.dir(M.dir .. '/parse') do
    if t == 'file' and name:match('%.mmd$') then
      out[#out + 1] = name:gsub('%.mmd$', '')
    end
  end
  table.sort(out)
  return out
end

-- Diagram types by phase, with the fixture name in types/.
M.phases = {
  { 'flowchart', 'sequence', 'class', 'state' },
  { 'er', 'pie', 'gantt', 'xychart', 'quadrant' },
  { 'gitgraph', 'mindmap', 'timeline', 'journey' },
  { 'sankey', 'block', 'packet', 'kanban', 'architecture', 'c4', 'requirement' },
  {
    'radar',
    'treemap',
    'venn',
    'ishikawa',
    'wardley',
    'cynefin',
    'treeview',
    'eventmodeling',
    'usecase',
    'swimlane',
  },
}

M.requirement_of_phase = { 'R-105', 'R-106', 'R-107', 'R-108', 'R-109' }

-- Helper calls -------------------------------------------------------------------

-- One shared session per spec file; closed by M.close_session().
local session
function M.session()
  if not session then
    session = QA.helper_session()
  end
  return session
end

function M.close_session()
  if session then
    session:close()
    session = nil
  end
end

-- mermaid.render through the session. Returns result, error.
function M.render(source, width_px, theme, background)
  local resp = M.session():request('mermaid.render', {
    source = source,
    width_px = width_px or 960,
    theme = theme or 'default',
    background = background or 'transparent',
  }, 30000)
  assert(resp, 'no response from the helper')
  return resp.result, resp.error
end

-- Renders through the CLI into a fresh file. Returns path, vim.system result.
function M.cli(source, width_px, theme, background)
  local dir = QA.tmpdir()
  local input = QA.write(dir .. '/in.mmd', source)
  local out = dir .. '/out.png'
  local args = { 'render', '--mermaid', input, '--width', tostring(width_px or 960), '-o', out }
  if theme then
    vim.list_extend(args, { '--theme', theme })
  end
  if background then
    vim.list_extend(args, { '--background', background })
  end
  local res = QA.helper_cli(args, { timeout = 60000 })
  return out, res
end

-- End-to-end preview ------------------------------------------------------------

M.CELL_W, M.CELL_H = 10, 20

local function ns()
  return vim.api.nvim_get_namespaces()['kitty-markdown-preview']
end

-- Opens a preview of a markdown file with the given lines, with the real
-- helper, a faked kitty that answers the probe, and a captured terminal.
-- opts.setup: extra config for setup(); opts.columns: editor width.
-- Returns a context table.
function M.open(lines, opts)
  opts = opts or {}
  if vim.fn.exists(':KittyMarkdown') ~= 2 then
    vim.cmd('runtime plugin/kitty-markdown.lua')
  end
  -- The checks model plain kitty; a tmux session in the environment of the
  -- QA run must not wrap the graphics bytes.
  vim.env.TMUX = nil
  vim.o.termguicolors = true
  vim.o.columns = opts.columns or 120
  vim.o.lines = opts.lines or 60
  local dir = QA.tmpdir()
  local cfg = vim.tbl_deep_extend('force', {
    placement = 'split',
    cache = { dir = dir .. '/cache' },
    helper = { download = false },
  }, opts.setup or {})
  require('kitty-markdown').setup(cfg)
  QA.use_helper()
  QA.fake_cell(M.CELL_W, M.CELL_H, vim.o.lines, vim.o.columns)
  local cap = QA.capture_terminal()
  local log, restore = QA.capture_notify()
  local path = dir .. '/doc.md'
  QA.write(path, table.concat(lines, '\n') .. '\n')
  vim.cmd('silent edit ' .. vim.fn.fnameescape(path))
  local src = vim.api.nvim_get_current_buf()
  vim.bo[src].filetype = 'markdown'
  local src_win = vim.api.nvim_get_current_win()
  QA.answer_probe(true)
  vim.cmd('KittyMarkdown open')
  local preview = require('kitty-markdown.preview')
  QA.wait(3000, function()
    return preview.mode() ~= nil
  end)
  local ctx = {
    dir = dir,
    src = src,
    src_win = src_win,
    cap = cap,
    log = log,
    restore = restore,
    path = path,
  }
  ctx.pbuf = M.preview_buf(ctx)
  return ctx
end

function M.preview_buf(ctx)
  for _, b in ipairs(vim.api.nvim_list_bufs()) do
    if b ~= ctx.src and vim.api.nvim_buf_is_loaded(b) and vim.bo[b].buftype == 'nofile' then
      local m = ns() and vim.api.nvim_buf_get_extmarks(b, ns(), 0, -1, {}) or {}
      if #m > 0 or vim.bo[b].filetype ~= '' then
        return b
      end
    end
  end
  local p = require('kitty-markdown.preview').get(ctx.src)
  return p and p.buf
end

-- Writes the source buffer (the trigger for a mermaid render).
function M.save(ctx, lines)
  local win = vim.api.nvim_get_current_win()
  vim.api.nvim_set_current_win(ctx.src_win)
  if lines then
    vim.api.nvim_buf_set_lines(ctx.src, 0, -1, false, lines)
  end
  vim.cmd('silent write')
  vim.api.nvim_set_current_win(win)
end

-- Every virtual line row in the preview, as strings.
function M.rows(ctx)
  local out = {}
  local buf = ctx.pbuf or M.preview_buf(ctx)
  if not buf or not ns() then
    return out
  end
  for _, mark in ipairs(vim.api.nvim_buf_get_extmarks(buf, ns(), 0, -1, { details = true })) do
    for _, row in ipairs(mark[4].virt_lines or {}) do
      local parts = {}
      for _, chunk in ipairs(row) do
        parts[#parts + 1] = chunk[1]
      end
      out[#out + 1] = table.concat(parts)
    end
  end
  return out
end

M.PLACEHOLDER = vim.fn.nr2char(0x10EEEE, 1)

-- Waits until no row says "loading" and some rows exist. Returns the rows.
function M.settle(ctx, ms)
  QA.wait(ms or 20000, function()
    local rows = M.rows(ctx)
    if #rows == 0 then
      return false
    end
    for _, r in ipairs(rows) do
      if r:find('loading', 1, true) then
        return false
      end
    end
    return true
  end, 20)
  return M.rows(ctx)
end

function M.rows_text(rows)
  return table.concat(rows, '\n')
end

-- Every message the plugin reported: captured notify calls plus :messages.
function M.messages(ctx)
  local out = {}
  for _, e in ipairs(ctx.log) do
    out[#out + 1] = tostring(e.msg)
  end
  local ok, m = pcall(vim.api.nvim_exec2, 'messages', { output = true })
  if ok and m and m.output then
    out[#out + 1] = m.output
  end
  return table.concat(out, '\n')
end

-- PNG images that the plugin transmitted (kitty graphics, f=100), decoded
-- from the captured bytes and written to files. Returns a list of paths.
function M.transmitted_pngs(ctx)
  local data = ctx.cap:text()
  -- Unwrap tmux passthrough, if any.
  data = data:gsub('\27Ptmux;(.-)\27\\', function(inner)
    return (inner:gsub('\27\27', '\27'))
  end)
  local images, cur = {}, nil
  for control, payload in data:gmatch('\27_G([^;\27]*);?([^\27]*)\27\\') do
    local keys = {}
    for k, v in control:gmatch('(%a)=([^,]*)') do
      keys[k] = v
    end
    if keys.a == 't' or keys.a == 'T' or (keys.f and not cur) then
      cur = { t = keys.t or 'd', parts = {} }
      images[#images + 1] = cur
    end
    if cur then
      cur.parts[#cur.parts + 1] = payload
      if keys.m == '0' or keys.m == nil then
        cur.done = true
        cur = nil
      end
    end
  end
  local out = {}
  for i, img in ipairs(images) do
    local raw = vim.base64.decode(table.concat(img.parts))
    local file
    if img.t == 'f' or img.t == 't' then
      file = raw
    else
      file = QA.write(ctx.dir .. '/sent-' .. i .. '.png', raw)
    end
    out[#out + 1] = file
  end
  return out
end

function M.close(ctx)
  if ctx and ctx.restore then
    ctx.restore()
  end
  pcall(function()
    require('kitty-markdown.transport').set_writer(nil)
  end)
  pcall(function()
    local h = require('kitty-markdown.helper')
    if h.stop then
      h.stop()
    elseif h.shutdown then
      h.shutdown()
    end
  end)
  QA.reset_plugin()
end

-- A markdown document with one mermaid fence.
function M.doc(source)
  local lines = { '# Diagram', '', '```mermaid' }
  vim.list_extend(lines, vim.split((source:gsub('\n+$', '')), '\n', { plain = true }))
  vim.list_extend(lines, { '```', '', 'After the diagram.' })
  return lines
end

return M
