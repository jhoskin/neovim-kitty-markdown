local QA = require('qa')
local a = QA.assert

QA.describe('layout hook basics', function()
  QA.check(
    'R-153',
    'layout() returns plain tables with no windows, jobs, or helper started',
    function()
      local wins = #vim.api.nvim_list_wins()
      local model = QA.layout({ '# Title', '', 'Some *text*.' })
      a.eq(wins, #vim.api.nvim_list_wins(), 'window count')
      local function plain(v, depth)
        local t = type(v)
        if t == 'function' or t == 'userdata' or t == 'thread' then
          return false
        end
        if t == 'table' and depth < 20 then
          for k, x in pairs(v) do
            if not plain(k, depth + 1) or not plain(x, depth + 1) then
              return false
            end
          end
        end
        return true
      end
      a.truthy(plain(model, 0), 'model holds only plain values')
      local helper = package.loaded['kitty-markdown.helper']
      a.falsy(helper and helper.is_running and helper.is_running(), 'helper running')
    end
  )

  QA.check('R-157', 'layout() is deterministic for the same content and options', function()
    local text = { '# A', '', '- one', '- two', '', '| x | y |', '| - | - |', '| 1 | 2 |' }
    local m1 = QA.layout(text, { width = 60 })
    local m2 = QA.layout(text, { width = 60 })
    a.eq(m1, m2)
  end)
end)
