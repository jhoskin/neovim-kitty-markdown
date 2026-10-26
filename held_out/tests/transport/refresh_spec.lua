local QA = require('qa')
local U = require('transport_util')
local a = QA.assert

local function doc(alt, file)
  return { '# Doc', '', 'Some text.', '', '![' .. alt .. '](' .. file .. ')', '', 'More text.' }
end

-- Opens two previews, each in its own tab, each with one image.
local function two_previews(dir)
  QA.make_png(dir .. '/a.png', 60, 40)
  QA.make_png(dir .. '/b.png', 50, 60, { 20, 20, 200, 255 })
  local src1 = U.open_doc(dir, doc('first image', 'a.png'), 'one.md')
  vim.cmd('tabnew')
  local src2 = U.open_doc(dir, doc('second image', 'b.png'), 'two.md')
  return src1, src2
end

local function refresh_from(src)
  local win = vim.fn.bufwinid(src)
  if win == -1 then
    for _, w in ipairs(vim.api.nvim_list_wins()) do
      if vim.api.nvim_win_get_buf(w) == src then
        win = w
      end
    end
  end
  vim.api.nvim_set_current_win(win)
  vim.cmd('KittyMarkdown refresh')
end

local function tx_ids(cap)
  local out = {}
  for _, t in ipairs(U.transmissions(cap)) do
    out[#out + 1] = t.id
  end
  return out
end

QA.describe('transport: refresh and mode changes', function()
  local cap, dir
  QA.before_each(function()
    cap, dir = U.env()
  end)
  QA.after_each(function()
    QA.reset_plugin()
  end)

  QA.check('R-130', 'The refresh command sends the graphics probe again', function()
    QA.make_png(dir .. '/a.png', 60, 40)
    local src = U.open_doc(dir, doc('pic', 'a.png'))
    U.answer_yes(cap)
    a.truthy(U.wait_placed(src, 1), 'placed')
    cap:clear()
    refresh_from(src)
    U.settle(30)
    a.eq(1, #U.queries(cap), 'graphics queries after refresh')
    a.truthy(cap:text():find('\27[c', 1, true) ~= nil, 'a DA1 request after refresh')
  end)

  QA.check(
    'R-130',
    'The refresh command transmits a placed image again with its full data',
    function()
      QA.make_png(dir .. '/a.png', 60, 40)
      local src = U.open_doc(dir, doc('pic', 'a.png'))
      U.answer_yes(cap)
      a.truthy(U.wait_placed(src, 1), 'placed')
      local before = U.transmissions(cap)[1]
      cap:clear()
      refresh_from(src)
      U.answer_yes(cap)
      a.truthy(
        QA.wait(5000, function()
          return #U.transmissions(cap) >= 1
        end),
        'a transmission after refresh'
      )
      U.settle(100)
      local tx = U.transmissions(cap)
      local same = false
      for _, t in ipairs(tx) do
        if t.data and t.data == before.data then
          same = true
        end
      end
      a.truthy(same, 'the image data is sent again')
      a.truthy(
        QA.wait(5000, function()
          return #U.placements(cap) >= 1
        end),
        'the virtual placement is made again'
      )
      a.truthy(U.wait_placed(src, 1), 'the image still shows after refresh')
    end
  )

  QA.check(
    'R-130',
    'The refresh command re-transmits the placed images of every open preview',
    function()
      local src1, src2 = two_previews(dir)
      U.answer_yes(cap)
      a.truthy(U.wait_placed(src1, 1) and U.wait_placed(src2, 1), 'both images placed')
      local first = {}
      for _, t in ipairs(U.transmissions(cap)) do
        first[#first + 1] = t.data
      end
      a.eq(2, #first, 'two transmissions before refresh')
      cap:clear()
      refresh_from(src2)
      U.answer_yes(cap)
      QA.wait(3000, function()
        return #U.transmissions(cap) >= 2
      end)
      U.settle(100)
      local again = {}
      for _, t in ipairs(U.transmissions(cap)) do
        for i, d in ipairs(first) do
          if t.data == d then
            again[i] = true
          end
        end
      end
      a.truthy(again[1], 'the image of the preview that was not current is sent again')
      a.truthy(again[2], 'the image of the current preview is sent again')
    end
  )

  QA.check(
    'R-131',
    'When the probe result turns images on, every open preview re-renders with images',
    function()
      local src1, src2 = two_previews(dir)
      U.answer_no()
      a.truthy(U.wait_mode('text'), 'text-only mode first')
      U.settle(50)
      a.eq(0, #U.placeholder_marks(U.preview_buf(src1)), 'no images in preview 1 in text mode')
      a.eq(0, #U.placeholder_marks(U.preview_buf(src2)), 'no images in preview 2 in text mode')
      refresh_from(src2)
      U.answer_yes(cap)
      a.truthy(U.wait_mode('images'), 'image mode after the re-probe')
      a.truthy(U.wait_placed(src2, 1), 'the current preview shows its image')
      a.truthy(U.wait_placed(src1, 1), 'the other preview shows its image')
      a.eq(2, #tx_ids(cap), 'both images transmitted')
    end
  )

  QA.check(
    'R-131',
    'When the probe result turns images off, every open preview re-renders with text-only boxes',
    function()
      local src1, src2 = two_previews(dir)
      U.answer_yes(cap)
      a.truthy(U.wait_placed(src1, 1) and U.wait_placed(src2, 1), 'both images placed')
      refresh_from(src1)
      U.answer_no()
      a.truthy(U.wait_mode('text'), 'text-only mode after the re-probe')
      a.truthy(
        QA.wait(2000, function()
          return #U.placeholder_marks(U.preview_buf(src1)) == 0
            and #U.placeholder_marks(U.preview_buf(src2)) == 0
        end),
        'no placeholder cells left in either preview'
      )
      local b1 = U.mark_with(U.preview_buf(src1), 'first image')
      local b2 = U.mark_with(U.preview_buf(src2), 'second image')
      a.truthy(b1 and U.is_box(b1.rows), 'preview 1 shows a text-only box')
      a.truthy(b2 and U.is_box(b2.rows), 'preview 2 shows a text-only box')
    end
  )
end)
