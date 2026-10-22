local config = require('kitty-markdown.config')
local cache = require('kitty-markdown.cache')
local download = require('kitty-markdown.download')
local helper = require('kitty-markdown.helper')
local version = require('kitty-markdown.version')

local here = vim.fn.fnamemodify(debug.getinfo(1, 'S').source:sub(2), ':p')
local root = vim.fs.dirname(vim.fs.dirname(here))
local fake = root .. '/tests/fixtures/fake-helper'
local local_build = root .. '/helper/target/release/kitty-markdown-helper'

local function wait_for(pred, ms)
  return vim.wait(ms or 5000, pred, 10)
end

-- A wrapper in a fresh directory that runs the fixture, so tests can stand
-- in for the install dir and delete the binary without touching the fixture.
local function fake_install(dir)
  vim.fn.mkdir(dir, 'p')
  local path = dir .. '/kitty-markdown-helper'
  vim.fn.writefile({ '#!/bin/sh', 'exec ' .. vim.fn.shellescape(fake) .. ' "$@"' }, path)
  vim.uv.fs_chmod(path, 493)
  return path
end

describe('helper', function()
  local notices
  local real_notify

  before_each(function()
    vim.env.KM_NVIM = vim.v.progpath
    vim.env.KM_FAKE_VERSION = nil
    config.setup({})
    cache.reset()
    download.reset()
    helper.reset()
    helper.set_locator(function(cb)
      cb(fake, 'user')
    end)
    notices = {}
    real_notify = vim.notify
    vim.notify = function(msg, level)
      notices[#notices + 1] = { msg = msg, level = level }
    end
  end)

  after_each(function()
    helper.reset()
    download.reset()
    vim.notify = real_notify
    vim.env.KM_FAKE_VERSION = nil
    config.setup({})
  end)

  local function ready()
    local done, got = false, nil
    helper.ensure(function(err)
      got = err
      done = true
    end)
    assert.is_true(wait_for(function()
      return done
    end))
    assert.is_nil(got)
    assert.are.equal('ready', helper.state())
  end

  local function notices_at(level)
    local out = {}
    for _, n in ipairs(notices) do
      if n.level == level then
        out[#out + 1] = n.msg
      end
    end
    return out
  end

  describe('locate', function()
    local tmp

    before_each(function()
      tmp = vim.fn.tempname()
      helper.set_locator(nil)
    end)

    after_each(function()
      vim.fn.delete(tmp, 'rf')
    end)

    local function locate()
      local got
      helper.locate(function(path, kind)
        got = { path = path, kind = kind }
      end)
      assert.is_true(wait_for(function()
        return got ~= nil
      end))
      return got
    end

    it('prefers the user path and never downloads (R-135)', function()
      local calls = 0
      download.set_runner(function()
        calls = calls + 1
      end)
      config.setup({ helper = { path = fake, download = true } })
      local got = locate()
      assert.are.same({ path = fake, kind = 'user' }, got)
      assert.are.equal(0, calls)
    end)

    it('uses the install dir before downloading (R-193)', function()
      local calls = 0
      download.set_runner(function()
        calls = calls + 1
      end)
      download.set_install_dir(tmp)
      local path = fake_install(tmp)
      local got = locate()
      assert.are.same({ path = path, kind = 'installed' }, got)
      assert.are.equal(0, calls)
    end)

    it('downloads into the install dir when nothing is installed (R-132)', function()
      download.set_install_dir(tmp)
      local argv = {}
      download.set_runner(function(cmd, _, on_exit)
        argv[#argv + 1] = cmd
        if cmd[1] == 'curl' then
          vim.fn.writefile({ 'archive' }, cmd[4])
        else
          fake_install(cmd[5])
        end
        on_exit({ code = 0, stdout = '', stderr = '' })
      end)
      local got = locate()
      assert.are.same({ path = tmp .. '/kitty-markdown-helper', kind = 'installed' }, got)
      assert.are.equal('curl', argv[1][1])
      assert.are.equal(download.url(version, download.target()), argv[1][5])
      assert.are.equal('tar', argv[2][1])
    end)

    it('falls back to the local build or nothing when the download fails (R-133)', function()
      download.set_install_dir(tmp)
      download.set_runner(function(_, _, on_exit)
        on_exit({ code = 22, stdout = '', stderr = 'curl: (22) 404' })
      end)
      local got = locate()
      if vim.fn.executable(local_build) == 1 then
        assert.are.same({ path = local_build, kind = 'local' }, got)
      else
        assert.is_nil(got.path)
      end
      assert.are.equal(1, #notices_at(vim.log.levels.WARN))
    end)

    it('skips the download when it is off', function()
      download.set_install_dir(tmp)
      local calls = 0
      download.set_runner(function()
        calls = calls + 1
      end)
      config.setup({ helper = { download = false } })
      locate()
      assert.are.equal(0, calls)
    end)
  end)

  it('starts absent and becomes ready after the probe (R-136, R-142)', function()
    assert.are.equal('absent', helper.state())
    assert.is_false(helper.is_running())
    ready()
    assert.is_true(helper.is_running())
    assert.are.equal('12.2.0', helper.info().mermaid_version)
    assert.are.equal(version, helper.info().helper_version)
    assert.are.equal('user', helper.binary_kind())
  end)

  it('round-trips a request (R-138)', function()
    local got
    helper.request('test.echo', { a = 1, b = { 'x', 'y' } }, function(err, result)
      assert.is_nil(err)
      got = result
    end)
    assert.is_true(wait_for(function()
      return got ~= nil
    end))
    assert.are.same({ a = 1, b = { 'x', 'y' } }, got)
    assert.are.equal('ready', helper.state())
  end)

  it('matches answers to requests by id when they arrive out of order (R-139)', function()
    ready()
    local first, second
    helper.request('test.reorder', {}, function(_, result)
      first = result
    end)
    helper.request('test.reorder', { tag = 'two' }, function(_, result)
      second = result
    end)
    assert.is_true(wait_for(function()
      return first ~= nil and second ~= nil
    end))
    assert.are.same({ order = 'first' }, first)
    assert.are.same({ order = 'second', tag = 'two' }, second)
  end)

  it('passes a helper error object through', function()
    local got
    helper.request(
      'test.error',
      { code = 'parse_error', message = 'Parse error on line 2', data = { line = 2 } },
      function(err)
        got = err
      end
    )
    assert.is_true(wait_for(function()
      return got ~= nil
    end))
    assert.are.same(
      { code = 'parse_error', message = 'Parse error on line 2', data = { line = 2 } },
      got
    )
  end)

  it('times out a slow request and ignores the late answer (R-141)', function()
    ready()
    config.setup({ helper = { request_timeout_ms = 50 } })
    local calls, got = 0, nil
    helper.request('test.sleep', { ms = 300 }, function(err)
      calls = calls + 1
      got = err
    end)
    assert.is_true(wait_for(function()
      return got ~= nil
    end, 1000))
    assert.are.equal('timeout', got.code)
    assert.matches('test.sleep', got.message)
    vim.wait(500, function()
      return false
    end, 10)
    assert.are.equal(1, calls)
    assert.are.equal('ready', helper.state())
  end)

  it('fails pending requests on a crash and restarts on the next request (R-140, R-195)', function()
    ready()
    local sleep_err
    helper.request('test.sleep', { ms = 5000 }, function(err)
      sleep_err = err
    end)
    helper.request('test.crash', { code = 3 })
    assert.is_true(wait_for(function()
      return sleep_err ~= nil
    end))
    assert.are.same({ code = 'helper_exit', message = 'helper exited with code 3' }, sleep_err)
    assert.are.equal('absent', helper.state())
    assert.is_false(helper.is_running())
    local warns = notices_at(vim.log.levels.WARN)
    assert.are.equal(1, #warns)
    assert.are.equal(
      'kitty-markdown: helper exited with code 3; it restarts on the next request',
      warns[1]
    )
    local got
    helper.request('test.echo', { again = true }, function(_, result)
      got = result
    end)
    assert.is_true(wait_for(function()
      return got ~= nil
    end))
    assert.are.same({ again = true }, got)
    assert.are.equal('ready', helper.state())
  end)

  it('stops restarting after three exits within 60 s until reset_failures (R-196)', function()
    for _ = 1, 3 do
      ready()
      helper.request('test.crash', { code = 1 })
      assert.is_true(wait_for(function()
        return not helper.is_running()
      end))
    end
    assert.are.equal('failed', helper.state())
    local errors = notices_at(vim.log.levels.ERROR)
    assert.are.equal(1, #errors)
    assert.matches('3 times within 60 seconds', errors[1])
    local got
    helper.request('test.echo', {}, function(err)
      got = err
    end)
    assert.is_true(wait_for(function()
      return got ~= nil
    end))
    assert.are.equal('no_helper', got.code)
    assert.is_false(helper.is_running())
    helper.reset_failures()
    assert.are.equal('absent', helper.state())
    ready()
  end)

  it('warns once on a version mismatch for a user path and continues (R-142)', function()
    vim.env.KM_FAKE_VERSION = '9.9.9'
    ready()
    local warns = notices_at(vim.log.levels.WARN)
    assert.are.equal(1, #warns)
    assert.matches('helper version 9.9.9 differs from plugin version ' .. version, warns[1])
    assert.are.equal('9.9.9', helper.info().helper_version)
  end)

  it('replaces a downloaded binary with the wrong version once (R-143)', function()
    local tmp = vim.fn.tempname()
    local path = fake_install(tmp)
    vim.env.KM_FAKE_VERSION = '0.0.1'
    local calls, gone = 0, false
    helper.set_locator(function(cb)
      calls = calls + 1
      if calls == 2 then
        -- The first binary is gone; the "download" puts a good one back.
        gone = vim.uv.fs_stat(path) == nil
        vim.env.KM_FAKE_VERSION = nil
        fake_install(tmp)
      end
      cb(path, 'installed')
    end)
    ready()
    assert.are.equal(2, calls)
    assert.is_true(gone)
    assert.are.equal(version, helper.info().helper_version)
    assert.are.same({}, notices_at(vim.log.levels.WARN))
    vim.fn.delete(tmp, 'rf')
  end)

  it('keeps the last stderr lines', function()
    local done = false
    helper.request('test.stderr', { text = 'warning: fontdb empty' }, function()
      done = true
    end)
    assert.is_true(wait_for(function()
      return done and #helper.stderr_tail() > 0
    end))
    assert.are.same({ 'warning: fontdb empty' }, helper.stderr_tail())
  end)

  it('shutdown sends the request and waits for the exit (R-137, R-197)', function()
    ready()
    helper.shutdown(2000)
    assert.is_false(helper.is_running())
    assert.are.equal('absent', helper.state())
    assert.are.same({}, notices_at(vim.log.levels.WARN))
  end)

  it('reports no binary once with the build command (R-134, R-194)', function()
    helper.set_locator(function(cb)
      cb(nil)
    end)
    local errs = {}
    for _ = 1, 2 do
      helper.request('test.echo', {}, function(err)
        errs[#errs + 1] = err
      end)
    end
    assert.is_true(wait_for(function()
      return #errs == 2
    end))
    assert.are.equal('no_helper', errs[1].code)
    assert.are.equal('no_helper', errs[2].code)
    assert.are.equal('failed', helper.state())
    local errors = notices_at(vim.log.levels.ERROR)
    assert.are.same({
      'kitty-markdown: no helper binary found; run: '
        .. 'cargo build --release --manifest-path helper/Cargo.toml',
    }, errors)
  end)
end)
