-- Shared pieces for the helper lifecycle checks: fake helper wrappers, a
-- download runner stub, preview drivers, and box text readers.
--
-- Loading this module points XDG_DATA_HOME and XDG_CACHE_HOME at a private
-- folder, so no check reads or writes the real Neovim data or cache dirs.
-- Require it before any plugin module.

local QA = require('qa')

local U = {}

U.base = QA.tmpdir()
vim.env.XDG_DATA_HOME = U.base .. '/data'
vim.env.XDG_CACHE_HOME = U.base .. '/cache'
vim.env.XDG_STATE_HOME = U.base .. '/state'
vim.fn.mkdir(U.base .. '/data', 'p')
vim.fn.mkdir(U.base .. '/cache', 'p')

-- `nvim -l` loads no plugin files, so define the user command here.
vim.cmd('runtime plugin/kitty-markdown.lua')

U.fake_script = QA.evals .. '/helper/qa_fake_helper.lua'
U.BINARY = 'kitty-markdown-helper'
U.NO_HELPER =
  'kitty-markdown: no helper binary found; run: cargo build --release --manifest-path helper/Cargo.toml'
U.TARGETS = { 'aarch64-apple-darwin', 'x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu' }

function U.plugin_version()
  local ok, v = pcall(require, 'kitty-markdown.version')
  QA.need(ok and type(v) == 'string', 'a plugin version string')
  return v
end

-- The release URL the spec gives, for a version and target.
function U.release_url(version, target)
  return ('https://github.com/jhoskin/neovim-kitty-markdown/releases/download/v%s/kitty-markdown-helper-%s.tar.gz'):format(
    version,
    target
  )
end

-- This machine's release target name by the spec's list, or nil.
function U.this_target()
  local u = vim.uv.os_uname()
  if u.sysname == 'Darwin' and (u.machine == 'arm64' or u.machine == 'aarch64') then
    return 'aarch64-apple-darwin'
  elseif u.sysname == 'Linux' and u.machine == 'x86_64' then
    return 'x86_64-unknown-linux-gnu'
  elseif u.sysname == 'Linux' and (u.machine == 'aarch64' or u.machine == 'arm64') then
    return 'aarch64-unknown-linux-gnu'
  end
  return nil
end

function U.install_path()
  return vim.fn.stdpath('data') .. '/kitty-markdown/' .. U.BINARY
end

-- Writes an executable wrapper that runs the QA fake helper with the given
-- settings. opts: mode (string), version, delay, png, w, h, shutdown_delay,
-- dir, name. Returns the wrapper path and its log path.
function U.make_fake(opts)
  opts = opts or {}
  local dir = opts.dir or QA.tmpdir()
  vim.fn.mkdir(dir, 'p')
  local path = dir .. '/' .. (opts.name or U.BINARY)
  local logp = opts.log or (QA.tmpdir() .. '/fake.log')
  local png = opts.png
  if not png then
    png = QA.tmpdir() .. '/result.png'
    QA.make_png(png, opts.w or 40, opts.h or 20)
  end
  local env = {
    QA_FAKE_MODE = opts.mode or '',
    QA_FAKE_LOG = logp,
    QA_FAKE_VERSION = opts.version or U.plugin_version(),
    QA_FAKE_DELAY = tostring(opts.delay or 0),
    QA_FAKE_PNG = png,
    QA_FAKE_W = tostring(opts.w or 40),
    QA_FAKE_H = tostring(opts.h or 20),
    QA_FAKE_SHUTDOWN_DELAY = tostring(opts.shutdown_delay or 0),
  }
  local lines = { '#!/bin/sh' }
  for k, v in pairs(env) do
    table.insert(lines, ('export %s=%s'):format(k, vim.fn.shellescape(v)))
  end
  table.insert(
    lines,
    ('exec %s --clean -l %s "$@"'):format(
      vim.fn.shellescape(vim.v.progpath),
      vim.fn.shellescape(U.fake_script)
    )
  )
  vim.fn.writefile(lines, path)
  vim.uv.fs_chmod(path, 493)
  return path, logp
end

function U.log_lines(logp)
  local s = QA.read(logp) or ''
  return vim.split(s, '\n', { trimempty = true })
end

function U.count(logp, prefix)
  local n = 0
  for _, l in ipairs(U.log_lines(logp)) do
    if l:sub(1, #prefix) == prefix then
      n = n + 1
    end
  end
  return n
end

function U.pids(logp)
  local out = {}
  for _, l in ipairs(U.log_lines(logp)) do
    local pid = l:match('^start (%d+)')
    if pid then
      table.insert(out, tonumber(pid))
    end
  end
  return out
end

function U.alive(pid)
  local ok, res = pcall(vim.uv.kill, pid, 0)
  return ok and res == 0
end

-- A tar.gz that holds `bin` as kitty-markdown-helper at its top level.
function U.make_tarball(bin)
  local dir = QA.tmpdir()
  local stage = dir .. '/stage'
  vim.fn.mkdir(stage, 'p')
  QA.write(stage .. '/' .. U.BINARY, QA.read(bin))
  vim.uv.fs_chmod(stage .. '/' .. U.BINARY, 493)
  local tgz = dir .. '/release.tar.gz'
  local res = QA.sh({ 'tar', '-czf', tgz, '-C', stage, U.BINARY })
  assert(res.code == 0, 'tar failed: ' .. tostring(res.stderr))
  return tgz
end

-- Installs a download runner stub. Every command is recorded. A command
-- whose program is curl (or wget) never runs: with opts.tarball it copies
-- that file to the output path and succeeds, otherwise it fails. Other
-- commands (tar, chmod) run for real. Returns the record list.
function U.stub_runner(opts)
  opts = opts or {}
  local download = require('kitty-markdown.download')
  QA.need(type(download.set_runner) == 'function', 'a runner hook in the download module')
  local rec = { cmds = {}, urls = {} }
  download.set_runner(function(cmd, sopts, on_exit)
    table.insert(rec.cmds, vim.deepcopy(cmd))
    local prog = vim.fs.basename(cmd[1] or '')
    if prog == 'curl' or prog == 'wget' then
      local out
      for i, v in ipairs(cmd) do
        if type(v) == 'string' and v:match('^https?://') then
          table.insert(rec.urls, v)
        end
        if v == '-o' or v == '--output' or v == '-O' then
          out = cmd[i + 1]
        end
      end
      local result
      if opts.tarball and out then
        QA.write(out, QA.read(opts.tarball))
        result = { code = 0, signal = 0, stdout = '', stderr = '' }
      else
        result = {
          code = 22,
          signal = 0,
          stdout = '',
          stderr = 'curl: (22) The requested URL returned error: 404',
        }
      end
      local obj = {
        wait = function()
          return result
        end,
        kill = function() end,
        pid = 0,
      }
      if on_exit then
        vim.uv.new_timer():start(5, 0, function()
          on_exit(result)
        end)
      end
      return obj
    end
    return vim.system(cmd, sopts, on_exit)
  end)
  return rec
end

-- Plugin reset plus the per-check setup every client check needs.
function U.reset(cfg)
  QA.reset_plugin()
  local download = package.loaded['kitty-markdown.download']
  if download and download.reset then
    download.reset()
  end
  vim.fn.delete(vim.fn.stdpath('data') .. '/kitty-markdown', 'rf')
  vim.o.termguicolors = true
  require('kitty-markdown').setup(vim.tbl_deep_extend('force', { placement = 'split' }, cfg or {}))
end

-- Points the plugin at a helper through the helper path option, the public
-- way, with downloads stubbed out.
function U.setup_with_path(path, extra)
  local cfg = vim.tbl_deep_extend('force', { helper = { path = path } }, extra or {})
  U.reset(cfg)
  return U.stub_runner()
end

-- A markdown file on disk with the given lines; opened in the current
-- window. Returns the buffer.
function U.open_doc(lines, dir)
  dir = dir or QA.tmpdir()
  local path = dir .. '/doc.md'
  for _, l in ipairs(lines) do
    for name in l:gmatch('%]%(([%w_%-]+%.png)%)') do
      QA.make_png(dir .. '/' .. name, 60, 30)
    end
  end
  vim.fn.writefile(lines, path)
  vim.cmd('edit ' .. vim.fn.fnameescape(path))
  local buf = vim.api.nvim_get_current_buf()
  vim.bo[buf].filetype = 'markdown'
  return buf, dir
end

-- Opens a preview of the current buffer with the terminal faked as kitty
-- (graphics reply, cell size, captured output).
function U.open_images_preview(lines, dir)
  local buf = U.open_doc(lines, dir)
  local cap = QA.capture_terminal()
  QA.fake_cell(10, 20, 40, 120)
  require('kitty-markdown').open('split')
  QA.answer_probe(true)
  vim.cmd('wincmd p')
  return buf, cap
end

-- The preview buffer of the source buffer (filetype kitty-markdown).
function U.preview_buf()
  for _, b in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_valid(b) and vim.bo[b].filetype == 'kitty-markdown' then
      return b
    end
  end
  return nil
end

function U.preview_bufs()
  local out = {}
  for _, b in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_valid(b) and vim.bo[b].filetype == 'kitty-markdown' then
      table.insert(out, b)
    end
  end
  return out
end

-- Every virtual line in a buffer, as strings, in extmark order.
function U.virt_rows(buf)
  buf = buf or U.preview_buf()
  local rows = {}
  if not buf then
    return rows
  end
  local marks = vim.api.nvim_buf_get_extmarks(buf, -1, 0, -1, { details = true })
  for _, m in ipairs(marks) do
    for _, row in ipairs((m[4] or {}).virt_lines or {}) do
      local parts = {}
      for _, chunk in ipairs(row) do
        table.insert(parts, chunk[1])
      end
      table.insert(rows, table.concat(parts))
    end
  end
  return rows
end

function U.virt_text(buf)
  return table.concat(U.virt_rows(buf), '\n')
end

function U.wait_text(needle, ms)
  return QA.wait(ms or 3000, function()
    return U.virt_text():find(needle, 1, true) ~= nil
  end)
end

-- Floating windows and windows overall, to tell a popup from a notice.
function U.win_snapshot()
  local floats = 0
  for _, w in ipairs(vim.api.nvim_list_wins()) do
    if vim.api.nvim_win_get_config(w).relative ~= '' then
      floats = floats + 1
    end
  end
  return { wins = #vim.api.nvim_list_wins(), floats = floats, tabs = vim.fn.tabpagenr('$') }
end

-- Messages from a notify log at exactly one level.
function U.at_level(log, level)
  local out = {}
  for _, e in ipairs(log) do
    if e.level == level then
      table.insert(out, e.msg)
    end
  end
  return out
end

-- The helper client: needs ensure/request to drive it without a preview.
function U.client()
  local helper = require('kitty-markdown.helper')
  QA.need(type(helper.request) == 'function', 'a request function in the helper module')
  return helper
end

-- Sends one request and waits for its callback. Returns err, result, done.
function U.request(method, params, ms)
  local helper = U.client()
  local got = nil
  helper.request(method, params or {}, function(err, result)
    got = { err = err, result = result }
  end)
  QA.wait(ms or 5000, function()
    return got ~= nil
  end)
  if not got then
    return nil, nil, false
  end
  return got.err, got.result, true
end

function U.cleanup()
  pcall(QA.reset_plugin)
  pcall(vim.cmd, 'silent! %bwipeout!')
end

return U
