local transport = require('kitty-markdown.transport')

local ESC = '\27'
local ST = ESC .. '\\'
local APC = ESC .. '_G'
local PLACEHOLDER = 0x10EEEE

local here = debug.getinfo(1, 'S').source:sub(2)
local fixture_path = vim.fs.dirname(here) .. '/fixtures/diacritics.txt'

local function capture()
  local out = {}
  transport.set_writer(function(bytes)
    out[#out + 1] = bytes
  end)
  return out
end

-- A file with a PNG signature and filler, long enough for `count` bytes.
local function write_png(count)
  local path = vim.fn.tempname()
  local signature = '\137PNG\r\n\26\n'
  local data = signature .. string.rep('x', count - #signature)
  local f = assert(io.open(path, 'wb'))
  f:write(data)
  f:close()
  return path, data
end

local function code_points(s)
  return vim.fn.str2list(s, 1)
end

local function fire(sequence)
  vim.api.nvim_exec_autocmds('TermResponse', { data = { sequence = sequence } })
end

local function run_probe()
  local result
  transport.probe(function(r)
    result = r
  end)
  return function(timeout_ms)
    vim.wait(timeout_ms or 100, function()
      return result ~= nil
    end)
    return result
  end
end

local function good_winsize()
  return 50, 80, 1600, 1000
end

describe('transport', function()
  local saved_tmux
  local saved_termguicolors

  before_each(function()
    saved_tmux = vim.env.TMUX
    saved_termguicolors = vim.o.termguicolors
    vim.env.TMUX = nil
    transport.reset()
  end)

  after_each(function()
    transport.reset()
    vim.env.TMUX = saved_tmux
    vim.o.termguicolors = saved_termguicolors
  end)

  describe('diacritics', function()
    it('has the 297 entries from kitty in fixture order', function()
      local expected = {}
      for _, line in ipairs(vim.fn.readfile(fixture_path)) do
        if not line:match('^#') and line ~= '' then
          expected[#expected + 1] = tonumber(line, 16)
        end
      end
      assert.are.equal(297, #expected)
      assert.are.same(expected, transport.DIACRITICS)
      assert.are.same(
        { 0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D },
        vim.list_slice(transport.DIACRITICS, 1, 6)
      )
    end)
  end)

  describe('placeholder cells', function()
    it('encodes the row and column diacritics after U+10EEEE', function()
      local cell = transport.encode_cell(42, 0, 1)
      assert.are.same({ PLACEHOLDER, 0x0305, 0x030D }, code_points(cell))
      assert.are.same({ PLACEHOLDER, 0x030E, 0x0305 }, code_points(transport.encode_cell(42, 2, 0)))
    end)

    it('adds the high byte as a third diacritic above 24 bits', function()
      local cell = transport.encode_cell(2 ^ 24 + 5, 0, 0)
      assert.are.same({ PLACEHOLDER, 0x0305, 0x0305, transport.DIACRITICS[2] }, code_points(cell))
      local two = transport.encode_cell(42 + 2 * 2 ^ 24, 1, 1)
      assert.are.same({ PLACEHOLDER, 0x030D, 0x030D, 0x030E }, code_points(two))
    end)

    it('fills every cell of every row with both diacritics', function()
      local rows, reason = transport.placeholder_rows(7, 3, 2)
      assert.is_nil(reason)
      assert.are.equal(2, #rows)
      for r, row in ipairs(rows) do
        local cps = code_points(row)
        assert.are.equal(9, #cps)
        for c = 0, 2 do
          assert.are.equal(PLACEHOLDER, cps[c * 3 + 1])
          assert.are.equal(transport.DIACRITICS[r], cps[c * 3 + 2])
          assert.are.equal(transport.DIACRITICS[c + 1], cps[c * 3 + 3])
        end
      end
    end)

    it('clamps rows and columns above 297 with the reason too tall', function()
      local rows, reason = transport.placeholder_rows(7, 2, 300)
      assert.are.equal(297, #rows)
      assert.are.equal('too tall', reason)
      local wide, wide_reason = transport.placeholder_rows(7, 300, 1)
      assert.are.equal(1, #wide)
      assert.are.equal(297 * 3, #code_points(wide[1]))
      assert.are.equal('too tall', wide_reason)
    end)
  end)

  describe('image ids', function()
    it('starts between 1 and 2^23 and increases', function()
      local first = transport.alloc_id()
      assert.is_true(first >= 1 and first <= 2 ^ 23)
      local previous = first
      for _ = 1, 50 do
        local id = transport.alloc_id()
        assert.is_true(id > previous)
        assert.is_true(id % 2 ^ 24 ~= 0)
        previous = id
      end
    end)

    it('skips an id whose low 24 bits are zero', function()
      transport.set_next_id(2 ^ 24 - 1)
      assert.are.equal(2 ^ 24 - 1, transport.alloc_id())
      assert.are.equal(2 ^ 24 + 1, transport.alloc_id())
    end)

    it('picks a new start after reset', function()
      transport.alloc_id()
      transport.alloc_id()
      transport.reset()
      local id = transport.alloc_id()
      assert.is_true(id >= 1 and id <= 2 ^ 23)
    end)

    it('defines a highlight group whose fg is the low 24 bits of the id', function()
      local name = transport.hl_group(0x123456)
      assert.are.equal('KittyMarkdownImg1193046', name)
      assert.are.equal(0x123456, vim.api.nvim_get_hl(0, { name = name }).fg)
      local high = transport.hl_group(2 ^ 24 + 5)
      assert.are.equal(5, vim.api.nvim_get_hl(0, { name = high }).fg)
      assert.are.equal(name, transport.hl_group(0x123456))
    end)
  end)

  describe('cell size', function()
    it('divides the pixel size by the cell counts', function()
      transport.set_winsize_reader(good_winsize)
      assert.are.same({ width_px = 20, height_px = 20 }, transport.cell_size())
    end)

    it('is nil when the terminal reports zero pixels', function()
      transport.set_winsize_reader(function()
        return 50, 80, 0, 0
      end)
      assert.is_nil(transport.cell_size())
    end)

    it('is nil when the reader fails', function()
      transport.set_winsize_reader(function()
        return nil
      end)
      assert.is_nil(transport.cell_size())
      transport.set_winsize_reader(function()
        error('ENOTTY')
      end)
      assert.is_nil(transport.cell_size())
    end)

    it('caches the value until VimResized', function()
      local calls = 0
      transport.set_winsize_reader(function()
        calls = calls + 1
        return 50, 80, 1600 * calls, 1000
      end)
      assert.are.equal(20, transport.cell_size().width_px)
      assert.are.equal(20, transport.cell_size().width_px)
      assert.are.equal(1, calls)
      vim.api.nvim_exec_autocmds('VimResized', {})
      assert.are.equal(40, transport.cell_size().width_px)
    end)
  end)

  describe('sequences', function()
    it('transmits base64 PNG data in 4096-character chunks', function()
      local out = capture()
      local path, data = write_png(9000)
      local encoded = vim.base64.encode(data)
      assert.are.equal(12000, #encoded)
      assert.is_true(transport.transmit(77, path))
      assert.are.equal(3, #out)
      assert.are.equal(APC .. 'a=T,f=100,t=d,i=77,q=2,m=1;' .. encoded:sub(1, 4096) .. ST, out[1])
      assert.are.equal(APC .. 'q=2,m=1;' .. encoded:sub(4097, 8192) .. ST, out[2])
      assert.are.equal(APC .. 'q=2,m=0;' .. encoded:sub(8193) .. ST, out[3])
      os.remove(path)
    end)

    it('sends a short image as one chunk with m=0', function()
      local out = capture()
      local path, data = write_png(100)
      assert.is_true(transport.transmit(78, path))
      assert.are.equal(1, #out)
      assert.are.equal(
        APC .. 'a=T,f=100,t=d,i=78,q=2,m=0;' .. vim.base64.encode(data) .. ST,
        out[1]
      )
      os.remove(path)
    end)

    it('transmits an id once unless forced', function()
      local out = capture()
      local path = write_png(100)
      assert.is_true(transport.transmit(79, path))
      assert.is_false(transport.transmit(79, path))
      assert.are.equal(1, #out)
      assert.is_true(transport.transmit(79, path, true))
      assert.are.equal(2, #out)
      os.remove(path)
    end)

    it('reports an unreadable file without writing', function()
      local out = capture()
      local ok, reason = transport.transmit(80, vim.fn.tempname() .. '/missing.png')
      assert.is_nil(ok)
      assert.are.equal('cannot read file', reason)
      assert.are.equal(0, #out)
    end)

    it('places a virtual placement with U=1', function()
      local out = capture()
      transport.place(5, 40, 12)
      assert.are.same({ APC .. 'a=p,U=1,i=5,c=40,r=12,q=2' .. ST }, out)
    end)

    it('deletes by id and forgets the transmitted id', function()
      local out = capture()
      local path = write_png(100)
      transport.transmit(81, path)
      transport.delete(81)
      assert.are.equal(APC .. 'a=d,d=I,i=81,q=2' .. ST, out[2])
      assert.is_true(transport.transmit(81, path))
      assert.are.equal(3, #out)
      os.remove(path)
    end)
  end)

  describe('tmux', function()
    it('wraps a sequence in DCS passthrough with doubled ESC', function()
      local wrapped = transport.wrap_tmux(ESC .. 'X' .. ST)
      assert.are.equal(ESC .. 'Ptmux;' .. ESC .. ESC .. 'X' .. ESC .. ESC .. '\\' .. ST, wrapped)
    end)

    it('wraps graphics sequences while $TMUX is set', function()
      vim.env.TMUX = '/tmp/tmux-501/default,123,0'
      local out = capture()
      transport.place(5, 2, 3)
      local inner = APC .. 'a=p,U=1,i=5,c=2,r=3,q=2' .. ST
      assert.are.same({ transport.wrap_tmux(inner) }, out)
      assert.matches('^\27Ptmux;\27\27_G', out[1])
    end)

    it('sends the DA1 request bare while $TMUX is set', function()
      vim.env.TMUX = '/tmp/tmux-501/default,123,0'
      local out = capture()
      run_probe()
      assert.are.equal(2, #out)
      assert.are.equal(transport.wrap_tmux(APC .. 'i=31,s=1,v=1,a=q,t=d,f=24;AAAA' .. ST), out[1])
      assert.are.equal(ESC .. '[c', out[2])
    end)
  end)

  describe('probe', function()
    before_each(function()
      vim.o.termguicolors = true
      transport.set_winsize_reader(good_winsize)
    end)

    it('writes the graphics query then Primary Device Attributes', function()
      local out = capture()
      run_probe()
      assert.are.same({ APC .. 'i=31,s=1,v=1,a=q,t=d,f=24;AAAA' .. ST, ESC .. '[c' }, out)
    end)

    it('turns images on when the graphics reply arrives', function()
      capture()
      local wait = run_probe()
      fire(ESC .. '_Gi=31;OK' .. ST)
      local result = wait()
      assert.are.same({ images = true, cell = { width_px = 20, height_px = 20 } }, result)
    end)

    it('turns images off when DA1 answers without a graphics reply', function()
      capture()
      local wait = run_probe()
      fire(ESC .. '[?62;c')
      local result = wait()
      assert.is_false(result.images)
      assert.are.equal('no graphics reply', result.reason)
    end)

    it('turns images off after the 500 ms timeout', function()
      capture()
      local wait = run_probe()
      local result = wait(700)
      assert.is_false(result.images)
      assert.are.equal('probe timeout', result.reason)
    end)

    it('ignores other terminal responses', function()
      capture()
      local wait = run_probe()
      fire(ESC .. ']11;rgb:0000/0000/0000' .. ST)
      assert.is_nil(wait(50))
      fire(ESC .. '_Gi=31;OK' .. ST)
      assert.is_true(wait().images)
    end)

    it('calls back once', function()
      capture()
      local count = 0
      transport.probe(function()
        count = count + 1
      end)
      fire(ESC .. '_Gi=31;OK' .. ST)
      fire(ESC .. '[?62;c')
      vim.wait(100, function()
        return count > 0
      end)
      vim.wait(50, function()
        return false
      end)
      assert.are.equal(1, count)
    end)

    it('keeps images off while termguicolors is off', function()
      vim.o.termguicolors = false
      capture()
      local wait = run_probe()
      fire(ESC .. '_Gi=31;OK' .. ST)
      local result = wait()
      assert.is_false(result.images)
      assert.are.equal('termguicolors off', result.reason)
    end)

    it('keeps images off while the cell size is unknown', function()
      transport.set_winsize_reader(function()
        return 50, 80, 0, 0
      end)
      capture()
      local wait = run_probe()
      fire(ESC .. '_Gi=31;OK' .. ST)
      local result = wait()
      assert.is_false(result.images)
      assert.are.equal('cell size unknown', result.reason)
      assert.is_nil(result.cell)
    end)

    it('reads the cell size again on every probe', function()
      assert.are.equal(20, transport.cell_size().width_px)
      transport.set_winsize_reader(function()
        return 50, 80, 3200, 1000
      end)
      capture()
      local wait = run_probe()
      fire(ESC .. '_Gi=31;OK' .. ST)
      assert.are.equal(40, wait().cell.width_px)
    end)

    it('drops the callback of a probe cancelled by reset', function()
      capture()
      local wait = run_probe()
      transport.reset()
      fire(ESC .. '_Gi=31;OK' .. ST)
      assert.is_nil(wait(50))
    end)
  end)
end)
