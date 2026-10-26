-- End to end: local images in a preview with a faked kitty terminal.
local QA = require('qa')
local U = require('images_util')
local a = QA.assert

local F = U.fixtures

local function only_box(r)
  a.eq(1, #r.boxes, 'one pixel block')
  return r.boxes[1]
end

local function float_count()
  local n = 0
  for _, w in ipairs(vim.api.nvim_list_wins()) do
    if vim.api.nvim_win_get_config(w).relative ~= '' then
      n = n + 1
    end
  end
  return n
end

QA.describe('images: local paths', function()
  QA.before_each(QA.reset_plugin)

  QA.check(
    'R-077',
    'A relative image path resolves against the directory of the markdown file, not the working directory',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/pics/p.png', 40, 40)
      local elsewhere = QA.tmpdir()
      vim.fn.chdir(elsewhere)
      local r = U.open({ '![rel](pics/p.png)' }, { dir = dir })
      local b = only_box(r)
      a.truthy(U.is_placed(b), 'placed; box: ' .. b.text)
    end
  )

  QA.check(
    'R-077',
    'A relative image path does not resolve against the working directory',
    function()
      local dir = QA.tmpdir()
      local cwd = QA.tmpdir()
      QA.make_png(cwd .. '/only-here.png', 40, 40)
      vim.fn.chdir(cwd)
      local r = U.open({ '![cwd](only-here.png)' }, { dir = dir })
      local b = only_box(r)
      a.falsy(U.is_placed(b), 'the image next to the working directory must not be used')
      a.contains('cannot read file', b.text)
    end
  )

  QA.check(
    'R-077',
    'A relative path with a parent directory part resolves against the markdown file directory',
    function()
      local root = QA.tmpdir()
      QA.make_png(root .. '/assets/p.png', 30, 30)
      vim.fn.mkdir(root .. '/notes', 'p')
      vim.fn.chdir(QA.tmpdir())
      local r = U.open({ '![up](../assets/p.png)' }, { dir = root .. '/notes' })
      a.truthy(U.is_placed(only_box(r)), 'placed')
    end
  )

  QA.check('R-078', 'An absolute image path is used as written', function()
    local other = QA.tmpdir()
    local p = QA.make_png(other .. '/abs.png', 40, 20)
    local r = U.open({ '![abs](' .. p .. ')' }, { dir = QA.tmpdir() })
    a.truthy(U.is_placed(only_box(r)), 'placed')
  end)

  QA.check(
    'R-172',
    'A missing local image shows a failure box with the reason "cannot read file" and its alt text',
    function()
      local r = U.open({ '![gone picture](gone.png)' })
      local b = only_box(r)
      a.contains('cannot read file', b.text)
      a.contains('gone picture', b.text)
    end
  )

  QA.check(
    'R-172',
    'An unreadable local image file shows a failure box with the reason "cannot read file"',
    function()
      local dir = QA.tmpdir()
      local p = QA.make_png(dir .. '/locked.png', 20, 20)
      vim.fn.setfperm(p, '---------')
      local ok, r = pcall(U.open, { '![locked](locked.png)' }, { dir = dir })
      vim.fn.setfperm(p, 'rw-r--r--')
      if not ok then
        error(r, 0)
      end
      a.contains('cannot read file', only_box(r).text)
    end
  )

  QA.check(
    'R-089',
    'An image in a raster format that the helper does not decode shows "unsupported format"',
    function()
      local dir = QA.tmpdir()
      QA.write(dir .. '/x.tiff', QA.read(F .. '/pic.tiff'))
      local r = U.open({ '![tiff](x.tiff)' }, { dir = dir })
      a.contains('unsupported format', only_box(r).text)
    end
  )

  QA.check(
    'R-089',
    'A file of plain text with an image extension shows "unsupported format"',
    function()
      local dir = QA.tmpdir()
      QA.write(dir .. '/notimage.png', 'this is plain text, not an image\n')
      local r = U.open({ '![text](notimage.png)' }, { dir = dir })
      a.contains('unsupported format', only_box(r).text)
    end
  )

  QA.check(
    'R-224',
    'A truncated PNG shows a failure box with the reason "unsupported format"',
    function()
      local dir = QA.tmpdir()
      local full = QA.read(QA.make_png(dir .. '/full.png', 64, 64))
      QA.write(dir .. '/cut.png', full:sub(1, 70))
      local r = U.open({ '![cut](cut.png)' }, { dir = dir })
      a.contains('unsupported format', only_box(r).text)
    end
  )

  QA.check(
    'R-224',
    'A truncated JPEG shows a failure box with the reason "unsupported format"',
    function()
      local dir = QA.tmpdir()
      local full = QA.read(F .. '/photo.jpg')
      QA.write(dir .. '/cut.jpg', full:sub(1, 40))
      local r = U.open({ '![cut](cut.jpg)' }, { dir = dir })
      a.contains('unsupported format', only_box(r).text)
    end
  )

  QA.check(
    { 'R-086', 'R-088' },
    'Every supported raster format and SVG is placed in the preview',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/a.png', 30, 20)
      local lines = { '![png](a.png)', '' }
      for _, f in ipairs({
        'photo.jpg',
        'still.gif',
        'anim.gif',
        'pic.webp',
        'pic.bmp',
        'drawing.svg',
      }) do
        QA.write(dir .. '/' .. f, QA.read(F .. '/' .. f))
        vim.list_extend(lines, { '![' .. f .. '](' .. f .. ')', '' })
      end
      local r = U.open(lines, { dir = dir, n = 7 })
      a.eq(7, #r.boxes)
      for i, b in ipairs(r.boxes) do
        a.truthy(U.is_placed(b), 'block ' .. i .. ': ' .. b.text)
      end
    end
  )
end)

QA.describe('images: size and placement', function()
  QA.before_each(QA.reset_plugin)

  QA.check(
    { 'R-090', 'R-227' },
    'An image wider than the target width is placed over the full text width with rows from its scaled height',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/wide.png', 1000, 200)
      local r = U.open({ '![wide](wide.png)' }, { dir = dir, cell = { 10, 20 } })
      local b = only_box(r)
      a.truthy(U.is_placed(b), b.text)
      local tw = U.text_width(r.win)
      local target = tw * 10
      local rows, cols = U.grid(b)
      a.eq(tw, cols, 'columns')
      a.eq(math.ceil(target * 200 / 1000 / 20), rows, 'rows of the scaled height')
      local pl = U.placements(r.cap)
      a.eq(tw, pl[#pl] and pl[#pl].cols, 'placement columns')
    end
  )

  QA.check(
    'R-091',
    'An image narrower than the target width keeps its native pixel size',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/small.png', 100, 50)
      local r = U.open({ '![small](small.png)' }, { dir = dir, cell = { 10, 20 } })
      local rows, cols = U.grid(only_box(r))
      a.eq(10, cols, 'columns of 100 px at 10 px cells')
      a.eq(3, rows, 'rows of 50 px at 20 px cells')
    end
  )

  QA.check(
    'R-093',
    'The reserved rows are the image height divided by the cell height, rounded up',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/h41.png', 30, 41)
      QA.make_png(dir .. '/h40.png', 30, 40)
      local r = U.open(
        { '![a](h41.png)', '', '![b](h40.png)' },
        { dir = dir, cell = { 10, 20 }, n = 2 }
      )
      a.eq(2, #r.boxes)
      a.eq(3, (U.grid(r.boxes[1])), 'a partial cell row rounds up')
      a.eq(2, (U.grid(r.boxes[2])), 'an exact multiple adds no row')
      for _, p in ipairs(U.placements(r.cap)) do
        a.truthy(p.rows == 2 or p.rows == 3, 'placement rows ' .. tostring(p.rows))
      end
    end
  )

  QA.check(
    'R-227',
    'The image columns are its pixel width divided by the cell width, rounded up',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/w95.png', 95, 20)
      QA.make_png(dir .. '/w90.png', 90, 20)
      local r = U.open(
        { '![a](w95.png)', '', '![b](w90.png)' },
        { dir = dir, cell = { 10, 20 }, n = 2 }
      )
      a.eq(10, select(2, U.grid(r.boxes[1])), 'a partial cell column rounds up')
      a.eq(9, select(2, U.grid(r.boxes[2])), 'an exact multiple adds no column')
    end
  )

  QA.check('R-094', 'The image grid follows the cell size that the terminal reports', function()
    local dir = QA.tmpdir()
    QA.make_png(dir .. '/p.png', 100, 50)
    local r1 = U.open({ '![p](p.png)' }, { dir = dir, cell = { 10, 20 } })
    local rows1, cols1 = U.grid(only_box(r1))
    QA.reset_plugin()
    for _, b in ipairs(vim.api.nvim_list_bufs()) do
      pcall(vim.api.nvim_buf_delete, b, { force = true })
    end
    local dir2 = QA.tmpdir()
    QA.make_png(dir2 .. '/p.png', 100, 50)
    local r2 = U.open({ '![p](p.png)' }, { dir = dir2, name = 'two.md', cell = { 7, 14 } })
    local b2 = only_box(r2)
    a.truthy(U.is_placed(b2), 'second preview placed: ' .. b2.text)
    local rows2, cols2 = U.grid(b2)
    a.eq({ 3, 10 }, { rows1, cols1 }, 'grid at 10x20 px cells')
    a.eq({ 4, 15 }, { rows2, cols2 }, 'grid at 7x14 px cells')
  end)

  QA.check(
    'R-092',
    'An <img> width smaller than the computed width sets the placed width',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/p.png', 200, 100)
      local r = U.open({ '<img src="p.png" width="50">' }, { dir = dir, cell = { 10, 20 } })
      local b = only_box(r)
      a.truthy(U.is_placed(b), b.text)
      a.eq(5, select(2, U.grid(b)), 'columns of the given width')
    end
  )

  QA.check(
    'R-092',
    'An <img> height smaller than the computed height sets the placed height',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/p.png', 200, 100)
      local r = U.open({ '<img src="p.png" height="40">' }, { dir = dir, cell = { 10, 20 } })
      local b = only_box(r)
      a.truthy(U.is_placed(b), b.text)
      a.eq(2, (U.grid(b)), 'rows of the given height')
    end
  )

  QA.check(
    'R-092',
    'An <img> width larger than the computed width does not enlarge the image',
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/p.png', 200, 100)
      local r = U.open({ '<img src="p.png" width="600">' }, { dir = dir, cell = { 10, 20 } })
      local rows, cols = U.grid(only_box(r))
      a.eq(20, cols, 'native width in columns')
      a.eq(5, rows, 'native height in rows')
    end
  )

  QA.check(
    'R-092',
    'An <img> width inside a paragraph also limits the placed width',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      QA.make_png(dir .. '/p.png', 300, 100)
      local r = U.open(
        { 'Some text <img src="p.png" width="100"> more text.' },
        { dir = dir, cell = { 10, 20 } }
      )
      local b = only_box(r)
      a.truthy(U.is_placed(b), b.text)
      a.eq(10, select(2, U.grid(b)), 'columns of the given width')
    end
  )
end)

QA.describe('images: paragraph placement (layout)', function()
  local function line_of(model, needle)
    for i, l in ipairs(model.lines) do
      if l:find(needle, 1, true) then
        return i
      end
    end
  end

  QA.check(
    'R-095',
    'An image inside a paragraph is placed on its own rows after the whole paragraph',
    function()
      local model = QA.layout(
        { 'alphaword ![pic](p.png) omegaword' },
        { width = 80, dir = QA.tmpdir() }
      )
      QA.need(#model.blocks >= 1, 'a block descriptor for the inline image')
      local blk = model.blocks[1]
      local la, lo = line_of(model, 'alphaword'), line_of(model, 'omegaword')
      a.not_nil(la, 'text before the image')
      a.not_nil(lo, 'text after the image')
      a.ge(math.max(la, lo) + 1, blk.line, 'anchor line after the paragraph text')
      a.eq('', vim.trim(model.lines[blk.line]), 'the anchor line is empty')
    end
  )

  QA.check('R-095', 'An image inside a paragraph sits before the next paragraph', function()
    local model = QA.layout(
      { 'first para ![pic](p.png) end of it', '', 'secondpara starts here' },
      { width = 80, dir = QA.tmpdir() }
    )
    QA.need(#model.blocks >= 1, 'a block descriptor for the inline image')
    local ls = line_of(model, 'secondpara')
    a.le(ls - 1, model.blocks[1].line, 'anchor before the next paragraph')
    a.ge(line_of(model, 'end of it') + 1, model.blocks[1].line, 'anchor after its own paragraph')
  end)

  QA.check(
    'R-095',
    'Several images inside one paragraph each get their own rows after the paragraph, in source order',
    function()
      local model = QA.layout(
        { 'aa ![one](1.png) bb ![two](2.png) cc' },
        { width = 80, dir = QA.tmpdir() }
      )
      a.eq(2, #model.blocks, 'two pixel blocks')
      local lc = line_of(model, 'cc')
      a.ge(lc + 1, model.blocks[1].line)
      a.ge(model.blocks[1].line + 1, model.blocks[2].line, 'second anchor after the first')
      a.contains('1.png', model.blocks[1].source)
      a.contains('2.png', model.blocks[2].source)
    end
  )

  QA.check(
    'R-095',
    'A paragraph image does not split the paragraph text into two paragraphs',
    { severity = 'medium' },
    function()
      local model = QA.layout(
        { 'leftside ![pic](p.png) rightside' },
        { width = 80, dir = QA.tmpdir() }
      )
      local l1, l2 = line_of(model, 'leftside'), line_of(model, 'rightside')
      a.eq(l1, l2, 'the text around the image stays on one line when it fits')
    end
  )

  QA.check(
    'R-093',
    'The layout reserves rows from the given pixel height and cell height, rounded up',
    function()
      local text = { '![pic](p.png)' }
      local base = QA.layout(text, { width = 80, dir = '/x' })
      local blk = base.blocks[1]
      QA.need(blk and blk.key, 'a block key on the descriptor')
      local model = QA.layout(text, {
        width = 80,
        dir = '/x',
        cell = { width_px = 10, height_px = 20 },
        sizes = { [blk.key] = { width_px = 100, height_px = 45 } },
      })
      a.eq(3, model.blocks[1].rows, 'rows of 45 px at 20 px cells')
      local exact = QA.layout(text, {
        width = 80,
        dir = '/x',
        cell = { width_px = 10, height_px = 20 },
        sizes = { [blk.key] = { width_px = 100, height_px = 60 } },
      })
      a.eq(3, exact.blocks[1].rows, 'rows of 60 px at 20 px cells')
    end
  )

  QA.check(
    'R-077',
    'The layout resolves a relative image path against the given base directory',
    { severity = 'medium' },
    function()
      local model = QA.layout({ '![pic](sub/p.png)' }, { width = 80, dir = '/base/dir' })
      a.eq(vim.fs.normalize('/base/dir/sub/p.png'), vim.fs.normalize(model.blocks[1].source))
    end
  )

  QA.check(
    'R-078',
    'The layout keeps an absolute image path as written',
    { severity = 'medium' },
    function()
      local model = QA.layout({ '![pic](/abs/path/p.png)' }, { width = 80, dir = '/base/dir' })
      a.eq('/abs/path/p.png', model.blocks[1].source)
    end
  )
end)

QA.describe('images: failure reporting', function()
  QA.before_each(QA.reset_plugin)

  QA.check(
    'R-096',
    'A failed image writes its full error to :messages and opens no popup',
    function()
      local before = U.messages()
      local floats = float_count()
      local r = U.open({ '![broken one](missing-file-q.png)' })
      a.contains('cannot read file', only_box(r).text)
      local new = U.new_messages(before)
      a.contains('missing-file-q.png', new, 'the message names the failing file')
      a.eq(floats, float_count(), 'floating windows')
    end
  )

  QA.check(
    'R-096',
    'An image with an unsupported format writes the error detail to :messages',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      QA.write(dir .. '/t.tiff', QA.read(F .. '/pic.tiff'))
      local before = U.messages()
      U.open({ '![tif](t.tiff)' }, { dir = dir })
      local new = U.new_messages(before)
      a.matches('[Uu]nsupported', new)
      a.matches('tiff', new, 'the detail of the error, not only the reason')
    end
  )

  QA.check('R-228', 'With no helper available, many images give no message per block', function()
    local dir = QA.tmpdir()
    local lines = {}
    for i = 1, 5 do
      QA.make_png(dir .. '/p' .. i .. '.png', 10, 10)
      vim.list_extend(lines, { '![nohelper' .. i .. '](p' .. i .. '.png)', '' })
    end
    local before = U.messages()
    local r = U.open(lines, { dir = dir, helper = false, wait = false })
    QA.wait(1500, function()
      return #U.boxes(r.pbuf) >= 5
    end)
    vim.wait(300)
    local new = U.new_messages(before)
    local n = 0
    for _ in new:gmatch('nohelper%d') do
      n = n + 1
    end
    a.eq(0, n, 'messages that name a block')
    local helper_lines = 0
    for l in new:gmatch('[^\n]+') do
      if l:find('helper', 1, true) then
        helper_lines = helper_lines + 1
      end
    end
    a.le(1, helper_lines, 'lines about the missing helper; got: ' .. new)
  end)
end)
