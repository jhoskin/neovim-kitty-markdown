-- R-236: the parity cases render at a target width of 960 pixels. The
-- helper output for every hidden case of a delivered phase must exist and
-- must not be wider than the target width.
local QA = require('qa')
local P = require('parity_util')
local a = QA.assert

QA.describe('parity: target width', function()
  QA.check(
    'R-236',
    'Every diagram of a delivered phase renders at the 960 pixel parity width and is never wider than it',
    function()
      local delivered = QA.delivered_phase()
      local fails, n = {}, 0
      local tmp = QA.tmpdir()
      for dtype, phase in pairs(P.PHASE) do
        if phase <= delivered then
          for _, name in ipairs(P.cases(dtype)) do
            n = n + 1
            local out = ('%s/%s-%s.png'):format(tmp, dtype, name)
            local r = QA.helper_cli({
              'render',
              '--mermaid',
              ('%s/%s/%s.mmd'):format(P.dir, dtype, name),
              '--width',
              tostring(P.WIDTH),
              '--theme',
              'default',
              '-o',
              out,
            })
            local w, h = QA.png_size(out)
            if r.code ~= 0 or not w then
              table.insert(
                fails,
                ('%s/%s: no PNG (%s)'):format(dtype, name, vim.trim(r.stderr or ''))
              )
            elseif w > P.WIDTH then
              table.insert(fails, ('%s/%s: %dx%d'):format(dtype, name, w, h))
            end
          end
        end
      end
      QA.need(n > 0, 'parity cases')
      a.truthy(#fails == 0, ('%d of %d cases: %s'):format(#fails, n, table.concat(fails, ' | ')))
    end
  )
end)
