-- Release binary download and install (package L-E).
-- Signatures: AGENTS.md, Conventions, Lua module contracts.
--
-- The fetch shells out to `curl` and `tar` through `vim.system`, so the
-- plugin needs no HTTP or gzip code of its own. Tests inject a runner and
-- an install directory; nothing here touches the network in a test.

local M = {}

local BINARY = 'kitty-markdown-helper'
local REPO = 'https://github.com/jhoskin/neovim-kitty-markdown'

local runner = nil
local install_dir_override = nil
local warned = false

local targets = {
  Darwin = { arm64 = 'aarch64-apple-darwin', aarch64 = 'aarch64-apple-darwin' },
  Linux = {
    x86_64 = 'x86_64-unknown-linux-gnu',
    aarch64 = 'aarch64-unknown-linux-gnu',
    arm64 = 'aarch64-unknown-linux-gnu',
  },
}

--- The release target name for this machine, or nil when no release
--- binary exists for it (R-192). `uname` defaults to `vim.uv.os_uname()`.
function M.target(uname)
  uname = uname or vim.uv.os_uname()
  local by_machine = targets[uname.sysname]
  return by_machine and by_machine[uname.machine] or nil
end

--- The release asset URL for a plugin version and target (R-191).
function M.url(version, target)
  return string.format('%s/releases/download/v%s/%s-%s.tar.gz', REPO, version, BINARY, target)
end

--- Where the downloaded binary lives (R-193).
function M.install_dir()
  return install_dir_override or (vim.fn.stdpath('data') .. '/kitty-markdown')
end

function M.binary_path()
  return M.install_dir() .. '/' .. BINARY
end

local function run(cmd, opts, on_exit)
  return (runner or vim.system)(cmd, opts, on_exit)
end

-- One WARN per instance: the caller falls through to the local build, and
-- a notice on every retry would drown the one that matters.
local function warn(reason)
  if warned then
    return
  end
  warned = true
  vim.notify('kitty-markdown: helper download failed: ' .. reason, vim.log.levels.WARN)
end

-- `vim.system` callbacks run in a fast event context, where `vim.notify`
-- is not allowed, so both the notice and the callback are scheduled.
local function fail(cb, reason)
  vim.schedule(function()
    warn(reason)
    cb({ code = 'download_failed', message = reason })
  end)
end

local function describe_exit(out)
  local text = vim.trim(out.stderr or '')
  if text == '' then
    text = 'exit code ' .. tostring(out.code)
  end
  return text
end

--- Download and unpack the release archive for `version` into the install
--- directory (R-132). `cb(err, path)` runs through `vim.schedule`.
function M.fetch(version, cb)
  local target = M.target()
  if not target then
    return fail(cb, 'no release binary for this platform')
  end
  local dir = M.install_dir()
  vim.fn.mkdir(dir, 'p')
  local tmp = dir .. '/' .. BINARY .. '.tar.gz.part'
  local url = M.url(version, target)
  run({ 'curl', '-fsSL', '-o', tmp, url }, { timeout = 60000 }, function(out)
    if out.code ~= 0 then
      os.remove(tmp)
      return fail(cb, 'curl: ' .. describe_exit(out))
    end
    run({ 'tar', '-xzf', tmp, '-C', dir }, {}, function(out2)
      os.remove(tmp)
      if out2.code ~= 0 then
        return fail(cb, 'tar: ' .. describe_exit(out2))
      end
      local bin = M.binary_path()
      if not vim.uv.fs_stat(bin) then
        return fail(cb, 'archive holds no ' .. BINARY)
      end
      vim.uv.fs_chmod(bin, 493)
      vim.schedule(function()
        cb(nil, bin)
      end)
    end)
  end)
end

--- Test hook: a `vim.system` replacement, or nil for the real one.
function M.set_runner(fn)
  runner = fn
end

--- Test hook: an install directory, or nil for `stdpath('data')`.
function M.set_install_dir(dir)
  install_dir_override = dir
end

--- Test hook: forget the hooks and the warning flag.
function M.reset()
  runner = nil
  install_dir_override = nil
  warned = false
end

return M
