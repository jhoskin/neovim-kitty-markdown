-- How the plugin finds its helper: path option, installed download,
-- download, local build (R-132, R-133, R-135, R-143, R-191, R-192, R-193,
-- R-242, R-243, R-311, R-312, R-327). Every download goes through a stub runner; no network.

local QA = require('qa')
local U = require('helper_util')
local a = QA.assert

local LOCAL_BUILD = QA.root .. '/helper/target/release/kitty-markdown-helper'

-- Runs the real locate order and returns path, kind.
local function locate()
  local helper = require('kitty-markdown.helper')
  QA.need(type(helper.locate) == 'function', 'the locate order exposed by the helper module')
  local got
  helper.locate(function(path, kind)
    got = { path = path, kind = kind }
  end)
  QA.wait(5000, function()
    return got ~= nil
  end)
  a.not_nil(got, 'locate finished')
  return got.path, got.kind
end

local function ensure()
  local helper = U.client()
  QA.need(type(helper.ensure) == 'function', 'an ensure function in the helper module')
  local done, err = false, nil
  helper.ensure(function(e)
    done, err = true, e
  end)
  QA.wait(8000, function()
    return done
  end)
  return done, err
end

local function same_file(p1, p2)
  return p1 and p2 and vim.uv.fs_realpath(p1) == vim.uv.fs_realpath(p2)
end

local function install(bin)
  local dest = U.install_path()
  vim.fn.mkdir(vim.fn.fnamemodify(dest, ':h'), 'p')
  QA.write(dest, QA.read(bin))
  vim.uv.fs_chmod(dest, 493)
  return dest
end

QA.describe('helper locate: download URL and targets', function()
  local restore
  QA.before_each(function()
    U.reset()
    _, restore = QA.capture_notify()
  end)
  QA.after_each(function()
    restore()
    U.cleanup()
  end)

  QA.check(
    'R-191',
    'The download URL is the GitHub release asset for the plugin version and the platform target',
    function()
      local rec = U.stub_runner()
      local target = U.this_target()
      QA.need(target, 'a supported platform for this run')
      locate()
      a.ge(1, #rec.urls, 'a download was attempted')
      a.eq(U.release_url(U.plugin_version(), target), rec.urls[1])
    end
  )

  QA.check(
    'R-192',
    'The release target names map macOS arm64 and Linux x86_64 and arm64 to the spec names',
    function()
      local download = require('kitty-markdown.download')
      QA.need(type(download.target) == 'function', 'a target name function in the download module')
      a.eq('aarch64-apple-darwin', download.target({ sysname = 'Darwin', machine = 'arm64' }))
      a.eq('x86_64-unknown-linux-gnu', download.target({ sysname = 'Linux', machine = 'x86_64' }))
      a.eq('aarch64-unknown-linux-gnu', download.target({ sysname = 'Linux', machine = 'aarch64' }))
    end
  )

  QA.check(
    'R-192',
    'A platform outside the three targets gets no made-up target name',
    { severity = 'medium' },
    function()
      local download = require('kitty-markdown.download')
      QA.need(type(download.target) == 'function', 'a target name function in the download module')
      for _, u in ipairs({
        { sysname = 'Darwin', machine = 'x86_64' },
        { sysname = 'Windows_NT', machine = 'x86_64' },
        { sysname = 'FreeBSD', machine = 'amd64' },
      }) do
        local t = download.target(u)
        a.truthy(
          t == nil or vim.tbl_contains(U.TARGETS, t),
          'target for ' .. u.sysname .. ' ' .. u.machine
        )
      end
    end
  )

  QA.check(
    { 'R-191', 'R-192' },
    'The download URL uses the target name of the platform the plugin runs on',
    function()
      local rec = U.stub_runner()
      local orig = vim.uv.os_uname
      vim.uv.os_uname = function()
        local u = orig()
        u.sysname, u.machine = 'Linux', 'x86_64'
        return u
      end
      local ok, err = pcall(locate)
      vim.uv.os_uname = orig
      assert(ok, err)
      a.contains(U.release_url(U.plugin_version(), 'x86_64-unknown-linux-gnu'), rec.urls)
    end
  )
end)

QA.describe('helper locate: download and install', function()
  local log, restore
  QA.before_each(function()
    U.reset()
    log, restore = QA.capture_notify()
  end)
  QA.after_each(function()
    restore()
    U.cleanup()
  end)

  QA.check(
    { 'R-132', 'R-193' },
    'With no helper installed, the release binary is downloaded and installed in the data dir',
    function()
      local fake = U.make_fake()
      local rec = U.stub_runner({ tarball = U.make_tarball(fake) })
      local path = locate()
      a.ge(1, #rec.urls, 'a download was attempted')
      a.truthy(vim.fn.executable(U.install_path()) == 1, 'an executable at the install path')
      a.truthy(same_file(U.install_path(), path), 'the plugin uses the installed binary')
    end
  )

  QA.check('R-132', 'A downloaded and installed helper is started and answers', function()
    local fake, logp = U.make_fake()
    U.stub_runner({ tarball = U.make_tarball(fake) })
    local done, err = ensure()
    a.truthy(done, 'ensure finished')
    a.is_nil(err, 'no error')
    a.eq(1, U.count(logp, 'start'), 'the downloaded binary ran')
  end)

  QA.check('R-132', 'An installed helper is used without a new download', function()
    local fake = U.make_fake()
    local dest = install(fake)
    local rec = U.stub_runner()
    local path = locate()
    a.eq({}, rec.urls, 'download URLs')
    a.truthy(same_file(dest, path), 'the installed binary')
  end)

  QA.check(
    'R-132',
    'The plugin downloads only once: the next lookup finds the installed binary',
    { severity = 'medium' },
    function()
      local fake = U.make_fake()
      local rec = U.stub_runner({ tarball = U.make_tarball(fake) })
      locate()
      local n = #rec.urls
      locate()
      a.eq(n, #rec.urls, 'download count after the second lookup')
    end
  )

  QA.check('R-133', 'When the download fails, the plugin uses the local release build', function()
    QA.need(vim.fn.executable(LOCAL_BUILD) == 1, 'a local release build in the repository')
    U.stub_runner()
    local path = locate()
    a.truthy(same_file(LOCAL_BUILD, path), 'the local build')
  end)

  QA.check(
    { 'R-133', 'R-327' },
    'On a platform without a release binary, the plugin uses the local release build',
    function()
      QA.need(vim.fn.executable(LOCAL_BUILD) == 1, 'a local release build in the repository')
      U.stub_runner()
      local orig = vim.uv.os_uname
      vim.uv.os_uname = function()
        local u = orig()
        u.sysname, u.machine = 'Darwin', 'x86_64'
        return u
      end
      local ok, res = pcall(locate)
      vim.uv.os_uname = orig
      assert(ok, res)
      a.truthy(same_file(LOCAL_BUILD, res), 'the local build')
    end
  )

  QA.check(
    'R-133',
    'A download archive without the helper binary falls back to the local release build',
    { severity = 'medium' },
    function()
      QA.need(vim.fn.executable(LOCAL_BUILD) == 1, 'a local release build in the repository')
      local dir = QA.tmpdir()
      QA.write(dir .. '/stage/README', 'nothing here')
      local tgz = dir .. '/empty.tar.gz'
      QA.sh({ 'tar', '-czf', tgz, '-C', dir .. '/stage', 'README' })
      U.stub_runner({ tarball = tgz })
      local path = locate()
      a.truthy(same_file(LOCAL_BUILD, path), 'the local build')
    end
  )

  QA.check(
    'R-133',
    'A failed download does not show an ERROR when the local build takes over',
    { severity = 'medium' },
    function()
      QA.need(vim.fn.executable(LOCAL_BUILD) == 1, 'a local release build in the repository')
      U.stub_runner()
      locate()
      a.eq({}, U.at_level(log, vim.log.levels.ERROR))
    end
  )
end)

QA.describe('helper locate: helper path option', function()
  local log, restore
  QA.before_each(function()
    U.reset()
    log, restore = QA.capture_notify()
  end)
  QA.after_each(function()
    restore()
    U.cleanup()
  end)

  QA.check(
    'R-135',
    'With the helper path option set, the plugin runs that binary and does not download',
    function()
      local fake, logp = U.make_fake()
      local rec = U.setup_with_path(fake)
      local done, err = ensure()
      a.truthy(done)
      a.is_nil(err)
      a.eq(1, U.count(logp, 'start'), 'the configured binary ran')
      a.eq({}, rec.urls, 'download URLs')
    end
  )

  QA.check('R-135', 'The helper path option wins over an installed binary', function()
    local other, other_log = U.make_fake()
    install(other)
    local fake, logp = U.make_fake()
    local rec = U.setup_with_path(fake)
    ensure()
    a.eq(1, U.count(logp, 'start'), 'the configured binary ran')
    a.eq(0, U.count(other_log, 'start'), 'the installed binary did not run')
    a.eq({}, rec.urls)
  end)

  QA.check(
    'R-243',
    'A helper path option that names a non-executable file gives one WARN and no download',
    function()
      local dir = QA.tmpdir()
      local bad = QA.write(dir .. '/kitty-markdown-helper', '#!/bin/sh\nexit 0\n')
      vim.uv.fs_chmod(bad, 420)
      local rec = U.setup_with_path(bad)
      locate()
      ensure()
      a.eq({}, rec.urls, 'download URLs')
      local warns = 0
      for _, m in ipairs(U.at_level(log, vim.log.levels.WARN)) do
        if m:find(bad, 1, true) or m:lower():find('execut') then
          warns = warns + 1
        end
      end
      a.eq(1, warns, 'WARN messages about the path')
    end
  )

  QA.check(
    'R-243',
    'The non-executable helper path WARN is not repeated on later requests',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local bad = QA.write(dir .. '/kitty-markdown-helper', 'not a program')
      vim.uv.fs_chmod(bad, 420)
      U.setup_with_path(bad)
      ensure()
      U.request('cache.gc', { max_bytes = 1 }, 3000)
      U.request('cache.gc', { max_bytes = 1 }, 3000)
      local warns = 0
      for _, m in ipairs(U.at_level(log, vim.log.levels.WARN)) do
        if m:find(bad, 1, true) or m:lower():find('execut') then
          warns = warns + 1
        end
      end
      a.eq(1, warns, 'WARN messages about the path')
    end
  )
end)

local NO_RELEASE_WARN =
  'kitty-markdown: helper download failed: no release binary for this platform'

-- Runs fn with os_uname reporting a platform that has no release binary.
local function on_unsupported_platform(fn)
  local orig = vim.uv.os_uname
  vim.uv.os_uname = function()
    local u = orig()
    u.sysname, u.machine = 'Darwin', 'x86_64'
    return u
  end
  local ok, res = pcall(fn)
  vim.uv.os_uname = orig
  assert(ok, res)
  return res
end

QA.describe('helper locate: platform without a release', function()
  local log, restore
  QA.before_each(function()
    U.reset()
    log, restore = QA.capture_notify()
  end)
  QA.after_each(function()
    restore()
    U.cleanup()
  end)

  QA.check(
    'R-327',
    'On a platform without a release binary, the plugin shows the no-release WARN once',
    function()
      QA.need(vim.fn.executable(LOCAL_BUILD) == 1, 'a local release build in the repository')
      U.stub_runner()
      on_unsupported_platform(function()
        locate()
        locate()
      end)
      local hits = 0
      for _, e in ipairs(log) do
        if e.msg == NO_RELEASE_WARN then
          hits = hits + 1
          a.eq(vim.log.levels.WARN, e.level, 'level')
        end
      end
      a.eq(1, hits, 'the exact WARN')
    end
  )

  QA.check(
    'R-327',
    'On a platform without a release binary, the plugin does not try a download',
    { severity = 'medium' },
    function()
      local rec = U.stub_runner()
      on_unsupported_platform(locate)
      a.eq({}, rec.urls, 'download URLs')
    end
  )

  QA.check(
    'R-327',
    'On a platform without a release binary, the local build starts and answers',
    function()
      QA.need(vim.fn.executable(LOCAL_BUILD) == 1, 'a local release build in the repository')
      U.stub_runner()
      local res = on_unsupported_platform(function()
        local done, err = ensure()
        return { done = done, err = err }
      end)
      a.truthy(res.done, 'ensure finished')
      a.is_nil(res.err)
    end
  )
end)

QA.describe('helper locate: missing or unusable path option', function()
  local log, restore
  QA.before_each(function()
    U.reset()
    log, restore = QA.capture_notify()
  end)
  QA.after_each(function()
    restore()
    U.cleanup()
  end)

  local function path_warns(bad)
    local n = 0
    for _, m in ipairs(U.at_level(log, vim.log.levels.WARN)) do
      if m:find(bad, 1, true) then
        n = n + 1
      end
    end
    return n
  end

  QA.check(
    { 'R-311', 'R-243' },
    'A helper path option that names a missing file gives one WARN and no download',
    function()
      local bad = QA.tmpdir() .. '/no-such-helper'
      local rec = U.setup_with_path(bad)
      locate()
      ensure()
      U.request('cache.gc', { max_bytes = 1 }, 3000)
      a.eq({}, rec.urls, 'download URLs')
      a.eq(1, path_warns(bad), 'WARN messages that name the path')
    end
  )

  QA.check('R-312', 'After the path option WARN, the plugin uses the installed binary', function()
    local bad = QA.tmpdir() .. '/no-such-helper'
    local rec = U.setup_with_path(bad)
    local fake, logp = U.make_fake()
    local dest = install(fake)
    local path = locate()
    a.truthy(same_file(dest, path), 'the installed binary')
    local done, err = ensure()
    a.truthy(done)
    a.is_nil(err)
    a.eq(1, U.count(logp, 'start'), 'the installed binary ran')
    a.eq({}, rec.urls, 'download URLs')
  end)

  QA.check(
    'R-312',
    'After the path option WARN with no installed binary, the plugin uses the local build',
    function()
      QA.need(vim.fn.executable(LOCAL_BUILD) == 1, 'a local release build in the repository')
      local dir = QA.tmpdir()
      local bad = QA.write(dir .. '/kitty-markdown-helper', 'not a program')
      vim.uv.fs_chmod(bad, 420)
      local rec = U.setup_with_path(bad)
      local path = locate()
      a.truthy(same_file(LOCAL_BUILD, path), 'the local build')
      a.eq({}, rec.urls, 'download URLs')
    end
  )

  QA.check(
    'R-312',
    'After the path option WARN, an installed binary wins over the local build',
    { severity = 'medium' },
    function()
      QA.need(vim.fn.executable(LOCAL_BUILD) == 1, 'a local release build in the repository')
      local dir = QA.tmpdir()
      local bad = QA.write(dir .. '/kitty-markdown-helper', 'not a program')
      vim.uv.fs_chmod(bad, 420)
      U.setup_with_path(bad)
      local dest = install(U.make_fake())
      local path = locate()
      a.truthy(same_file(dest, path), 'the installed binary')
      a.falsy(same_file(LOCAL_BUILD, path), 'not the local build')
    end
  )
end)

QA.describe('helper locate: version check', function()
  local log, restore
  QA.before_each(function()
    U.reset()
    log, restore = QA.capture_notify()
  end)
  QA.after_each(function()
    restore()
    U.cleanup()
  end)

  QA.check('R-142', 'The first request the plugin sends to a new helper is probe', function()
    local fake, logp = U.make_fake()
    U.setup_with_path(fake)
    U.request('cache.gc', { max_bytes = 1000000 }, 5000)
    local methods = {}
    for _, l in ipairs(U.log_lines(logp)) do
      local m = l:match('^method (.*)')
      if m then
        table.insert(methods, m)
      end
    end
    a.eq('probe', methods[1])
  end)

  QA.check(
    'R-142',
    'A request waits for the probe answer before it reaches the helper',
    { severity = 'medium' },
    function()
      local fake, logp = U.make_fake({ mode = 'silent_probe' })
      U.setup_with_path(fake, { helper = { request_timeout_ms = 1500 } })
      local helper = U.client()
      helper.request('cache.gc', { max_bytes = 1000000 }, function() end)
      QA.wait(600, function()
        return false
      end)
      local methods = {}
      for _, l in ipairs(U.log_lines(logp)) do
        local m = l:match('^method (.*)')
        if m then
          table.insert(methods, m)
        end
      end
      a.eq({ 'probe' }, methods)
    end
  )

  QA.check(
    'R-143',
    'A downloaded helper with a different version is replaced by a download of the matching version',
    function()
      local stale = U.make_fake({ version = '0.0.0-stale' })
      install(stale)
      local fresh, fresh_log = U.make_fake()
      local rec = U.stub_runner({ tarball = U.make_tarball(fresh) })
      local done, err = ensure()
      a.truthy(done, 'ensure finished')
      a.contains(U.release_url(U.plugin_version(), U.this_target() or 'none'), rec.urls)
      a.is_nil(err, 'ready after the new download')
      a.eq(1, U.count(fresh_log, 'start'), 'the new binary ran')
    end
  )

  QA.check(
    'R-143',
    'A downloaded helper with the matching version is not downloaded again',
    { severity = 'medium' },
    function()
      local good = U.make_fake()
      install(good)
      local rec = U.stub_runner()
      ensure()
      a.eq({}, rec.urls)
    end
  )

  QA.check(
    'R-242',
    'A user path helper with a different version gives one WARN and keeps working',
    function()
      local fake, logp = U.make_fake({ version = '9.9.9' })
      local rec = U.setup_with_path(fake)
      local done, err = ensure()
      a.truthy(done)
      a.is_nil(err, 'the helper is used')
      local e2, _, d2 = U.request('cache.gc', { max_bytes = 1000000 }, 5000)
      a.truthy(d2, 'a later request is answered')
      a.is_nil(e2)
      a.eq(1, #U.at_level(log, vim.log.levels.WARN), 'WARN messages')
      a.eq({}, rec.urls, 'no download for a user path')
      a.eq(1, U.count(logp, 'start'), 'one helper process')
    end
  )

  QA.check(
    'R-242',
    'The version WARN for a user path helper is not repeated after a restart',
    { severity = 'medium' },
    function()
      local fake = U.make_fake({ version = '9.9.9', mode = 'crash' })
      U.setup_with_path(fake)
      U.request('image.prepare', { path = '/x.png', width_px = 10 }, 5000)
      U.request('image.prepare', { path = '/x.png', width_px = 10 }, 5000)
      local n = 0
      for _, m in ipairs(U.at_level(log, vim.log.levels.WARN)) do
        if m:find('9.9.9', 1, true) then
          n = n + 1
        end
      end
      a.eq(1, n, 'version WARN messages')
    end
  )

  QA.check(
    'R-242',
    'A local build helper with a different version gives one WARN, keeps working, and is not replaced',
    function()
      local helper = U.client()
      QA.need(type(helper.set_locator) == 'function', 'a locator hook in the helper module')
      local fake, logp = U.make_fake({ version = '9.9.9' })
      local rec = U.stub_runner()
      helper.set_locator(function(cb)
        cb(fake, 'local')
      end)
      local done, err = ensure()
      a.truthy(done)
      a.is_nil(err)
      a.eq(1, #U.at_level(log, vim.log.levels.WARN), 'WARN messages')
      a.eq({}, rec.urls, 'no download')
      a.eq(1, U.count(logp, 'start'), 'one helper process')
    end
  )
end)
