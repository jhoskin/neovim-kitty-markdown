-- kitty graphics protocol writer, probe, placeholders (package L-D).
-- Signatures: AGENTS.md, Conventions, Lua module contracts.
--
-- Every byte that reaches the terminal goes through one writer so tests can
-- capture the exact sequences, and so the tmux passthrough wraps every
-- graphics sequence in one place.

local M = {}

local ESC = '\27'
local APC = ESC .. '_G'
local ST = ESC .. '\\'
local DA1_REQUEST = ESC .. '[c'
local PROBE_QUERY = APC .. 'i=31,s=1,v=1,a=q,t=d,f=24;AAAA' .. ST
local PROBE_TIMEOUT_MS = 500
local CHUNK_SIZE = 4096
local PLACEHOLDER = 0x10EEEE
local AUGROUP = 'kitty-markdown-transport'
-- Image ids live in a 32-bit space; the low 24 bits travel in the
-- foreground color and the high byte in a third diacritic.
local ID_COLOR_SPACE = 2 ^ 24
local ID_START_MAX = 2 ^ 23

-- kitty gen/rowcolumn-diacritics.txt in index order. Index 1 here is
-- diacritic number 0. tests/fixtures/diacritics.txt holds the same list.
-- stylua: ignore
M.DIACRITICS = {
  0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D, 0x033E, 0x033F,
  0x0346, 0x034A, 0x034B, 0x034C, 0x0350, 0x0351, 0x0352, 0x0357,
  0x035B, 0x0363, 0x0364, 0x0365, 0x0366, 0x0367, 0x0368, 0x0369,
  0x036A, 0x036B, 0x036C, 0x036D, 0x036E, 0x036F, 0x0483, 0x0484,
  0x0485, 0x0486, 0x0487, 0x0592, 0x0593, 0x0594, 0x0595, 0x0597,
  0x0598, 0x0599, 0x059C, 0x059D, 0x059E, 0x059F, 0x05A0, 0x05A1,
  0x05A8, 0x05A9, 0x05AB, 0x05AC, 0x05AF, 0x05C4, 0x0610, 0x0611,
  0x0612, 0x0613, 0x0614, 0x0615, 0x0616, 0x0617, 0x0657, 0x0658,
  0x0659, 0x065A, 0x065B, 0x065D, 0x065E, 0x06D6, 0x06D7, 0x06D8,
  0x06D9, 0x06DA, 0x06DB, 0x06DC, 0x06DF, 0x06E0, 0x06E1, 0x06E2,
  0x06E4, 0x06E7, 0x06E8, 0x06EB, 0x06EC, 0x0730, 0x0732, 0x0733,
  0x0735, 0x0736, 0x073A, 0x073D, 0x073F, 0x0740, 0x0741, 0x0743,
  0x0745, 0x0747, 0x0749, 0x074A, 0x07EB, 0x07EC, 0x07ED, 0x07EE,
  0x07EF, 0x07F0, 0x07F1, 0x07F3, 0x0816, 0x0817, 0x0818, 0x0819,
  0x081B, 0x081C, 0x081D, 0x081E, 0x081F, 0x0820, 0x0821, 0x0822,
  0x0823, 0x0825, 0x0826, 0x0827, 0x0829, 0x082A, 0x082B, 0x082C,
  0x082D, 0x0951, 0x0953, 0x0954, 0x0F82, 0x0F83, 0x0F86, 0x0F87,
  0x135D, 0x135E, 0x135F, 0x17DD, 0x193A, 0x1A17, 0x1A75, 0x1A76,
  0x1A77, 0x1A78, 0x1A79, 0x1A7A, 0x1A7B, 0x1A7C, 0x1B6B, 0x1B6D,
  0x1B6E, 0x1B6F, 0x1B70, 0x1B71, 0x1B72, 0x1B73, 0x1CD0, 0x1CD1,
  0x1CD2, 0x1CDA, 0x1CDB, 0x1CE0, 0x1DC0, 0x1DC1, 0x1DC3, 0x1DC4,
  0x1DC5, 0x1DC6, 0x1DC7, 0x1DC8, 0x1DC9, 0x1DCB, 0x1DCC, 0x1DD1,
  0x1DD2, 0x1DD3, 0x1DD4, 0x1DD5, 0x1DD6, 0x1DD7, 0x1DD8, 0x1DD9,
  0x1DDA, 0x1DDB, 0x1DDC, 0x1DDD, 0x1DDE, 0x1DDF, 0x1DE0, 0x1DE1,
  0x1DE2, 0x1DE3, 0x1DE4, 0x1DE5, 0x1DE6, 0x1DFE, 0x20D0, 0x20D1,
  0x20D4, 0x20D5, 0x20D6, 0x20D7, 0x20DB, 0x20DC, 0x20E1, 0x20E7,
  0x20E9, 0x20F0, 0x2CEF, 0x2CF0, 0x2CF1, 0x2DE0, 0x2DE1, 0x2DE2,
  0x2DE3, 0x2DE4, 0x2DE5, 0x2DE6, 0x2DE7, 0x2DE8, 0x2DE9, 0x2DEA,
  0x2DEB, 0x2DEC, 0x2DED, 0x2DEE, 0x2DEF, 0x2DF0, 0x2DF1, 0x2DF2,
  0x2DF3, 0x2DF4, 0x2DF5, 0x2DF6, 0x2DF7, 0x2DF8, 0x2DF9, 0x2DFA,
  0x2DFB, 0x2DFC, 0x2DFD, 0x2DFE, 0x2DFF, 0xA66F, 0xA67C, 0xA67D,
  0xA6F0, 0xA6F1, 0xA8E0, 0xA8E1, 0xA8E2, 0xA8E3, 0xA8E4, 0xA8E5,
  0xA8E6, 0xA8E7, 0xA8E8, 0xA8E9, 0xA8EA, 0xA8EB, 0xA8EC, 0xA8ED,
  0xA8EE, 0xA8EF, 0xA8F0, 0xA8F1, 0xAAB0, 0xAAB2, 0xAAB3, 0xAAB7,
  0xAAB8, 0xAABE, 0xAABF, 0xAAC1, 0xFE20, 0xFE21, 0xFE22, 0xFE23,
  0xFE24, 0xFE25, 0xFE26, 0x10A0F, 0x10A38, 0x1D185, 0x1D186, 0x1D187,
  0x1D188, 0x1D189, 0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243,
  0x1D244,
}

local MAX_CELLS = #M.DIACRITICS

local state = {
  writer = nil,
  winsize_reader = nil,
  cell = nil,
  next_id = nil,
  transmitted = {},
  hl_defined = {},
  augroup = nil,
  probes = {},
}

local function write(bytes)
  if state.writer then
    state.writer(bytes)
  else
    vim.api.nvim_ui_send(bytes)
  end
end

--- Wrap one escape sequence in a tmux DCS passthrough. tmux unwraps the
--- outer DCS and undoubles the ESC bytes before it forwards the sequence.
function M.wrap_tmux(seq)
  local doubled = seq:gsub(ESC, ESC .. ESC)
  return ESC .. 'Ptmux;' .. doubled .. ST
end

-- Graphics sequences need the passthrough; the DA1 request does not,
-- because tmux answers DA1 itself.
local function send_graphics(seq)
  if vim.env.TMUX and vim.env.TMUX ~= '' then
    write(M.wrap_tmux(seq))
  else
    write(seq)
  end
end

-- Cell size ---------------------------------------------------------------

local ffi_ok, ffi = pcall(require, 'ffi')
local ffi_defined = false

-- TIOCGWINSZ differs per kernel: Linux uses the plain ioctl number, the
-- BSD family (macOS included) encodes the struct size in the request.
local function winsize_request()
  if vim.uv.os_uname().sysname == 'Linux' then
    return 0x5413
  end
  return 0x40087468
end

local function read_winsize()
  if state.winsize_reader then
    return state.winsize_reader()
  end
  if not ffi_ok then
    return nil
  end
  if not ffi_defined then
    -- A reloaded module would redefine the struct and ffi raises on that,
    -- so the result of cdef is ignored on purpose.
    pcall(
      ffi.cdef,
      [[
        struct winsize { unsigned short ws_row, ws_col, ws_xpixel, ws_ypixel; };
        int ioctl(int fd, unsigned long request, ...);
      ]]
    )
    ffi_defined = true
  end
  local ws = ffi.new('struct winsize')
  if ffi.C.ioctl(1, winsize_request(), ws) ~= 0 then
    return nil
  end
  return ws.ws_row, ws.ws_col, ws.ws_xpixel, ws.ws_ypixel
end

local ensure_augroup

--- The size of one terminal cell in pixels, or nil when the terminal does
--- not report a pixel size. Cached until a resize or the next probe.
function M.cell_size()
  if state.cell then
    return state.cell
  end
  local ok, rows, cols, xpixel, ypixel = pcall(read_winsize)
  if not ok or not rows then
    return nil
  end
  if rows == 0 or cols == 0 or xpixel == 0 or ypixel == 0 then
    return nil
  end
  state.cell = { width_px = xpixel / cols, height_px = ypixel / rows }
  ensure_augroup()
  return state.cell
end

function ensure_augroup()
  if state.augroup then
    return state.augroup
  end
  state.augroup = vim.api.nvim_create_augroup(AUGROUP, { clear = true })
  vim.api.nvim_create_autocmd('VimResized', {
    group = state.augroup,
    callback = function()
      state.cell = nil
    end,
  })
  -- A colorscheme change clears every highlight group, which would turn
  -- the placeholder cells into plain text until the next render.
  vim.api.nvim_create_autocmd('ColorScheme', {
    group = state.augroup,
    callback = function()
      for id in pairs(state.hl_defined) do
        vim.api.nvim_set_hl(0, 'KittyMarkdownImg' .. id, { fg = id % ID_COLOR_SPACE })
      end
    end,
  })
  return state.augroup
end

-- Probe --------------------------------------------------------------------

--- Ask the terminal whether it speaks the graphics protocol. The result
--- arrives through `cb` with `{ images, reason, cell }`; `reason` is set
--- when `images` is false. Never blocks: the answers come through the
--- TermResponse event or the timeout.
function M.probe(cb)
  state.cell = nil
  local group = ensure_augroup()
  local probe = { done = false }
  state.probes[probe] = true

  local function teardown()
    probe.done = true
    state.probes[probe] = nil
    if probe.autocmd then
      pcall(vim.api.nvim_del_autocmd, probe.autocmd)
    end
    if probe.timer and not probe.timer:is_closing() then
      probe.timer:stop()
      probe.timer:close()
    end
  end

  local function finish(images, reason)
    if probe.done then
      return
    end
    teardown()
    local cell = M.cell_size()
    if images and not vim.o.termguicolors then
      images, reason = false, 'termguicolors off'
    elseif images and not cell then
      images, reason = false, 'cell size unknown'
    end
    vim.schedule(function()
      cb({ images = images, reason = reason, cell = cell })
    end)
  end
  -- A cancelled probe (reset between tests) drops its callback.
  probe.cancel = function()
    if not probe.done then
      teardown()
    end
  end

  probe.autocmd = vim.api.nvim_create_autocmd('TermResponse', {
    group = group,
    callback = function(ev)
      local seq = ev.data and ev.data.sequence
      if type(seq) ~= 'string' then
        return
      end
      if seq:find('^\27_G.*i=31') then
        finish(true, nil)
      elseif seq:find('^\27%[%?.*c$') then
        finish(false, 'no graphics reply')
      end
    end,
  })
  probe.timer = vim.uv.new_timer()
  probe.timer:start(
    PROBE_TIMEOUT_MS,
    0,
    vim.schedule_wrap(function()
      finish(false, 'probe timeout')
    end)
  )

  send_graphics(PROBE_QUERY)
  write(DA1_REQUEST)
end

-- Image ids ----------------------------------------------------------------

local function seed_ids()
  if state.next_id then
    return
  end
  -- Ids share a namespace with every other program in the terminal, so the
  -- start is random per instance; the seed mixes time and pid so two
  -- instances started in the same tick still differ.
  math.randomseed((vim.uv.hrtime() % 2 ^ 31) + vim.uv.os_getpid())
  state.next_id = math.random(1, ID_START_MAX)
end

--- The next unused image id. Ids increase and never repeat in this Neovim
--- instance. An id whose low 24 bits are zero would give a black
--- foreground that kitty reads as "no image", so those are skipped.
function M.alloc_id()
  seed_ids()
  local id = state.next_id
  while id % ID_COLOR_SPACE == 0 do
    id = id + 1
  end
  state.next_id = id + 1
  return id
end

--- The highlight group that carries `id` in its foreground color. Defined
--- once per id.
function M.hl_group(id)
  local name = 'KittyMarkdownImg' .. id
  if not state.hl_defined[id] then
    ensure_augroup()
    vim.api.nvim_set_hl(0, name, { fg = id % ID_COLOR_SPACE })
    state.hl_defined[id] = true
  end
  return name
end

-- Placeholder cells --------------------------------------------------------

local char_cache = {}

local function char(cp)
  local s = char_cache[cp]
  if not s then
    s = vim.fn.nr2char(cp, 1)
    char_cache[cp] = s
  end
  return s
end

--- One placeholder cell for `id` at zero-based `row` and `col`.
function M.encode_cell(id, row, col)
  local cell = char(PLACEHOLDER) .. char(M.DIACRITICS[row + 1]) .. char(M.DIACRITICS[col + 1])
  if id >= ID_COLOR_SPACE then
    cell = cell .. char(M.DIACRITICS[math.floor(id / ID_COLOR_SPACE) + 1])
  end
  return cell
end

--- The placeholder text for a block of `cols` by `rows` cells, one string
--- per row, every cell with both diacritics. The diacritic list limits a
--- block to 297 rows and columns; a larger request is clamped and the
--- second return value is the reason `too tall`.
function M.placeholder_rows(id, cols, rows)
  local reason = nil
  if rows > MAX_CELLS then
    rows, reason = MAX_CELLS, 'too tall'
  end
  if cols > MAX_CELLS then
    cols, reason = MAX_CELLS, 'too tall'
  end
  local lines = {}
  for row = 0, rows - 1 do
    local cells = {}
    for col = 0, cols - 1 do
      cells[col + 1] = M.encode_cell(id, row, col)
    end
    lines[row + 1] = table.concat(cells)
  end
  return lines, reason
end

-- Transmit, place, delete --------------------------------------------------

local function read_file(path)
  local fd = vim.uv.fs_open(path, 'r', 438)
  if not fd then
    return nil
  end
  local stat = vim.uv.fs_fstat(fd)
  local data = stat and vim.uv.fs_read(fd, stat.size, 0)
  vim.uv.fs_close(fd)
  if not data or data == '' then
    return nil
  end
  return data
end

--- Send the PNG at `png_path` to the terminal as image `id`. Returns true
--- when the data was sent, false when the id was sent before and `force`
--- is not set, and nil plus a reason when the file cannot be read.
function M.transmit(id, png_path, force)
  if state.transmitted[id] and not force then
    return false
  end
  local data = read_file(png_path)
  if not data then
    return nil, 'cannot read file'
  end
  local encoded = vim.base64.encode(data)
  local total = #encoded
  local pos = 1
  local first = true
  repeat
    local chunk = encoded:sub(pos, pos + CHUNK_SIZE - 1)
    pos = pos + CHUNK_SIZE
    local more = pos <= total and 1 or 0
    local control
    if first then
      control = string.format('a=t,f=100,t=d,i=%d,q=2,m=%d', id, more)
      first = false
    else
      control = string.format('q=2,m=%d', more)
    end
    send_graphics(APC .. control .. ';' .. chunk .. ST)
  until more == 0
  state.transmitted[id] = true
  return true
end

--- Create the virtual placement that the placeholder cells refer to.
function M.place(id, cols, rows)
  send_graphics(APC .. string.format('a=p,U=1,i=%d,c=%d,r=%d,q=2', id, cols, rows) .. ST)
end

--- Delete image `id` and its placements, and free the data, so a later
--- transmit of the same id sends the bytes again.
function M.delete(id)
  send_graphics(APC .. string.format('a=d,d=I,i=%d,q=2', id) .. ST)
  state.transmitted[id] = nil
end

-- Test hooks ---------------------------------------------------------------

--- Replace the byte writer; nil restores `nvim_ui_send`.
function M.set_writer(fn)
  state.writer = fn
end

--- Replace the TIOCGWINSZ reader with a function that returns
--- `rows, cols, xpixel, ypixel`; nil restores the ioctl.
function M.set_winsize_reader(fn)
  state.winsize_reader = fn
  state.cell = nil
end

--- Move the id counter, so a test can reach the ids that `alloc_id` skips.
function M.set_next_id(id)
  state.next_id = id
end

--- Forget every piece of state: transmitted ids, the id counter, the cell
--- size, pending probes, defined groups, and the injected hooks.
function M.reset()
  for probe in pairs(state.probes) do
    probe.cancel()
  end
  state.probes = {}
  state.transmitted = {}
  state.hl_defined = {}
  state.next_id = nil
  state.cell = nil
  state.writer = nil
  state.winsize_reader = nil
  if state.augroup then
    vim.api.nvim_del_augroup_by_id(state.augroup)
    state.augroup = nil
  end
end

return M
