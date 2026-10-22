local download = require('kitty-markdown.download')

local function wait_for(pred, ms)
  return vim.wait(ms or 2000, pred, 10)
end

describe('download', function()
  local notices
  local real_notify
  local tmp

  before_each(function()
    download.reset()
    tmp = vim.fn.tempname()
    download.set_install_dir(tmp)
    notices = {}
    real_notify = vim.notify
    vim.notify = function(msg, level)
      notices[#notices + 1] = { msg = msg, level = level }
    end
  end)

  after_each(function()
    vim.notify = real_notify
    download.reset()
    vim.fn.delete(tmp, 'rf')
  end)

  describe('target', function()
    it('maps the three supported platforms (R-192)', function()
      assert.are.equal(
        'aarch64-apple-darwin',
        download.target({ sysname = 'Darwin', machine = 'arm64' })
      )
      assert.are.equal(
        'x86_64-unknown-linux-gnu',
        download.target({ sysname = 'Linux', machine = 'x86_64' })
      )
      assert.are.equal(
        'aarch64-unknown-linux-gnu',
        download.target({ sysname = 'Linux', machine = 'aarch64' })
      )
    end)

    it('returns nil for an unsupported platform', function()
      assert.is_nil(download.target({ sysname = 'Darwin', machine = 'x86_64' }))
      assert.is_nil(download.target({ sysname = 'Windows_NT', machine = 'x86_64' }))
      assert.is_nil(download.target({ sysname = 'FreeBSD', machine = 'amd64' }))
    end)

    it('reads the running machine by default', function()
      local uname = vim.uv.os_uname()
      assert.are.equal(download.target(uname), download.target())
    end)
  end)

  it('builds the release asset URL (R-191)', function()
    assert.are.equal(
      'https://github.com/jhoskin/neovim-kitty-markdown/releases/download/v0.1.0/'
        .. 'kitty-markdown-helper-aarch64-apple-darwin.tar.gz',
      download.url('0.1.0', 'aarch64-apple-darwin')
    )
  end)

  it('installs under stdpath data (R-193)', function()
    download.set_install_dir(nil)
    assert.are.equal(vim.fn.stdpath('data') .. '/kitty-markdown', download.install_dir())
    assert.are.equal(
      vim.fn.stdpath('data') .. '/kitty-markdown/kitty-markdown-helper',
      download.binary_path()
    )
  end)

  describe('fetch', function()
    local function fetch(version)
      local done, got_err, got_path = false, nil, nil
      download.fetch(version, function(err, path)
        done, got_err, got_path = true, err, path
      end)
      assert.is_true(wait_for(function()
        return done
      end))
      return got_err, got_path
    end

    it('runs curl then tar and makes the binary executable (R-132)', function()
      local argv = {}
      download.set_runner(function(cmd, opts, on_exit)
        argv[#argv + 1] = { cmd = cmd, opts = opts }
        if cmd[1] == 'curl' then
          vim.fn.writefile({ 'archive' }, cmd[4])
        else
          vim.fn.writefile({ '#!/bin/sh', 'exit 0' }, cmd[5] .. '/kitty-markdown-helper')
        end
        on_exit({ code = 0, stdout = '', stderr = '' })
      end)
      local err, path = fetch('0.1.0')
      assert.is_nil(err)
      assert.are.equal(tmp .. '/kitty-markdown-helper', path)
      assert.are.equal(2, #argv)
      local tar_part = tmp .. '/kitty-markdown-helper.tar.gz.part'
      assert.are.same(
        { 'curl', '-fsSL', '-o', tar_part, download.url('0.1.0', download.target()) },
        argv[1].cmd
      )
      assert.are.equal(60000, argv[1].opts.timeout)
      assert.are.same({ 'tar', '-xzf', tar_part, '-C', tmp }, argv[2].cmd)
      assert.are.equal(1, vim.fn.executable(path))
      assert.is_nil(vim.uv.fs_stat(tar_part))
      assert.are.same({}, notices)
    end)

    it('reports a curl failure once and does not run tar', function()
      local calls = 0
      download.set_runner(function(_, _, on_exit)
        calls = calls + 1
        on_exit({
          code = 22,
          stdout = '',
          stderr = 'curl: (22) The requested URL returned error: 404',
        })
      end)
      local err, path = fetch('0.1.0')
      assert.is_nil(path)
      assert.are.equal('download_failed', err.code)
      assert.matches('404', err.message)
      assert.are.equal(1, calls)
      fetch('0.1.0')
      assert.are.equal(1, #notices)
      assert.are.equal(vim.log.levels.WARN, notices[1].level)
      assert.matches('helper download failed', notices[1].msg)
    end)

    it('reports a tar failure', function()
      download.set_runner(function(cmd, _, on_exit)
        if cmd[1] == 'curl' then
          vim.fn.writefile({ 'not an archive' }, cmd[4])
          on_exit({ code = 0, stdout = '', stderr = '' })
        else
          on_exit({ code = 1, stdout = '', stderr = 'tar: Error opening archive' })
        end
      end)
      local err = fetch('0.1.0')
      assert.are.equal('download_failed', err.code)
      assert.matches('tar', err.message)
    end)

    it('fails without a network call when the platform has no release', function()
      local real_uname = vim.uv.os_uname
      vim.uv.os_uname = function()
        return { sysname = 'Darwin', machine = 'x86_64' }
      end
      local calls = 0
      download.set_runner(function()
        calls = calls + 1
      end)
      local ok, err = pcall(fetch, '0.1.0')
      vim.uv.os_uname = real_uname
      assert.is_true(ok)
      assert.are.equal('download_failed', err.code)
      assert.matches('no release binary', err.message)
      assert.are.equal(0, calls)
    end)
  end)
end)
