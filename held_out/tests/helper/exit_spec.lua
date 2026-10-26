-- qa: serial
-- Helper shutdown when Neovim exits (R-137, R-197). Each check runs a child
-- editor that starts a fake helper and then exits.

local QA = require('qa')
local U = require('helper_util')
local a = QA.assert

local CHILD = QA.evals .. '/helper/child_exit.lua'

-- Runs the child editor with the fake helper. Returns a table with
-- ready (bool), ms from READY to the child exit, the child exit code, and
-- the child stderr.
local function run_child(fake)
  local ready_at, exit_at = nil, nil
  local errbuf = {}
  local proc = vim.system({ vim.v.progpath, '--clean', '-l', CHILD, QA.root, fake }, {
    text = true,
    stderr = function(_, data)
      if data then
        table.insert(errbuf, data)
        if
          not ready_at
          and table.concat(errbuf):find('READY\n', 1, true)
          and not table.concat(errbuf):find('NOT_READY', 1, true)
        then
          ready_at = vim.uv.hrtime()
        end
      end
    end,
  }, function()
    exit_at = vim.uv.hrtime()
  end)
  local res = proc:wait(15000)
  QA.wait(500, function()
    return exit_at ~= nil
  end)
  local stderr = table.concat(errbuf)
  QA.need(not stderr:find('NO_ENSURE', 1, true), 'an ensure function in the helper module')
  return {
    ready = ready_at ~= nil,
    ms = (ready_at and exit_at) and (exit_at - ready_at) / 1e6 or nil,
    code = res and res.code,
    stderr = stderr,
  }
end

local function wait_dead(pids, ms)
  return QA.wait(ms, function()
    for _, pid in ipairs(pids) do
      if U.alive(pid) then
        return false
      end
    end
    return true
  end, 20)
end

QA.describe('helper exit with Neovim', function()
  QA.check('R-137', 'When Neovim exits, the plugin sends shutdown to the helper', function()
    local fake, logp = U.make_fake()
    local r = run_child(fake)
    a.truthy(r.ready, 'the child started the helper: ' .. r.stderr)
    QA.wait(1000, function()
      return U.count(logp, 'method shutdown') > 0
    end)
    a.eq(1, U.count(logp, 'method shutdown'), 'shutdown requests')
  end)

  QA.check(
    'R-137',
    'When Neovim exits, the plugin waits for a helper that takes a moment to exit after shutdown',
    function()
      local fake, logp = U.make_fake({ shutdown_delay = 400 })
      local r = run_child(fake)
      a.truthy(r.ready, 'the child started the helper: ' .. r.stderr)
      QA.wait(2000, function()
        return U.count(logp, 'clean_exit') > 0 or U.count(logp, 'exit') > 0
      end)
      a.eq(1, U.count(logp, 'clean_exit'), 'the helper finished its own exit')
      a.ge(350, r.ms, 'ms Neovim waited after READY')
    end
  )

  QA.check('R-137', 'No helper process is left after Neovim exits', function()
    local fake, logp = U.make_fake()
    local r = run_child(fake)
    a.truthy(r.ready, 'the child started the helper: ' .. r.stderr)
    local pids = U.pids(logp)
    a.eq(1, #pids, 'helper processes')
    a.truthy(wait_dead(pids, 1000), 'the helper is gone')
  end)

  QA.check(
    'R-197',
    'A helper that ignores shutdown is killed and Neovim exits within about 2 seconds',
    function()
      local fake, logp = U.make_fake({ mode = 'ignore_shutdown' })
      local r = run_child(fake)
      a.truthy(r.ready, 'the child started the helper: ' .. r.stderr)
      a.not_nil(r.ms, 'the child exited')
      a.le(2800, r.ms, 'ms from READY to the Neovim exit')
      local pids = U.pids(logp)
      a.eq(1, #pids, 'helper processes')
      a.truthy(wait_dead(pids, 700), 'the helper was killed')
      for _, pid in ipairs(pids) do
        if U.alive(pid) then
          vim.uv.kill(pid, 'sigkill')
        end
      end
    end
  )

  QA.check(
    'R-197',
    'A helper that ignores shutdown gets the shutdown request before it is killed',
    { severity = 'medium' },
    function()
      local fake, logp = U.make_fake({ mode = 'ignore_shutdown' })
      local r = run_child(fake)
      a.truthy(r.ready, 'the child started the helper: ' .. r.stderr)
      local pids = U.pids(logp)
      wait_dead(pids, 1000)
      for _, pid in ipairs(pids) do
        if U.alive(pid) then
          vim.uv.kill(pid, 'sigkill')
        end
      end
      a.eq(1, U.count(logp, 'method shutdown'), 'shutdown requests')
    end
  )

  QA.check(
    'R-197',
    'A helper that exits at once on shutdown does not hold Neovim for the full 2 seconds',
    { severity = 'medium' },
    function()
      local fake = U.make_fake()
      local r = run_child(fake)
      a.truthy(r.ready, 'the child started the helper: ' .. r.stderr)
      a.not_nil(r.ms, 'the child exited')
      a.le(1500, r.ms, 'ms from READY to the Neovim exit')
    end
  )
end)
