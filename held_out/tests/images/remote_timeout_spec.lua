-- qa: serial
-- The default remote fetch timeout (R-289). Slow: one fetch waits for
-- the full default of 10 s.
local QA = require('qa')
local U = require('images_util')
local a = QA.assert

local srv

QA.describe('images: remote fetch timeout default', function()
  QA.before_each(QA.reset_plugin)
  QA.after_each(function()
    if srv then
      srv.stop()
      srv = nil
    end
  end)

  QA.check(
    { 'R-289', 'R-082' },
    'With no timeout option, a stalled remote fetch fails with "timeout" after 10000 ms, not before',
    { timeout_ms = 30000 },
    function()
      srv = U.server({ ['/stall.png'] = { stall = true } })
      local t0 = vim.uv.hrtime()
      local r = U.open({ '![stall](' .. srv.url('/stall.png') .. ')' }, { wait = false })
      local box
      QA.wait(14000, function()
        local bs = U.boxes(r.pbuf)
        if bs[1] and U.reason_in(bs[1]) then
          box = bs[1]
          return true
        end
        return false
      end, 50)
      local ms = (vim.uv.hrtime() - t0) / 1e6
      a.not_nil(box, 'a failure box within 14 s')
      a.contains('timeout', box.text)
      a.ge(9500, ms, 'ms until the timeout')
      a.le(12500, ms, 'ms until the timeout')
    end
  )

  QA.check(
    { 'R-289', 'R-082' },
    'The remote_images.timeout_ms option sets the fetch timeout',
    function()
      srv = U.server({ ['/s2.png'] = { stall = true } })
      local t0 = vim.uv.hrtime()
      local r = U.open(
        { '![s2](' .. srv.url('/s2.png') .. ')' },
        { remote = { timeout_ms = 700 }, ms = 5000 }
      )
      local ms = (vim.uv.hrtime() - t0) / 1e6
      a.contains('timeout', r.boxes[1].text)
      a.ge(600, ms, 'ms until the timeout')
      a.le(3000, ms, 'ms until the timeout')
    end
  )
end)
