local config = require('kitty-markdown.config')
local cache = require('kitty-markdown.cache')
local preview_mod = require('kitty-markdown.preview')
local real_in_use_paths = preview_mod.in_use_paths
local helper = require('kitty-markdown.helper')

local here = vim.fn.fnamemodify(debug.getinfo(1, 'S').source:sub(2), ':p')
local root = vim.fs.dirname(vim.fs.dirname(here))
local fake = root .. '/tests/fixtures/fake-helper'

local sha = vim.fn.sha256

local function wait_for(pred, ms)
  return vim.wait(ms or 5000, pred, 10)
end

describe('cache', function()
  before_each(function()
    config.setup({})
    cache.reset()
  end)

  after_each(function()
    config.setup({})
    cache.reset()
  end)

  it('roots the cache at stdpath cache unless configured (R-148)', function()
    assert.are.equal(vim.fn.stdpath('cache') .. '/kitty-markdown', cache.root())
    config.setup({ cache = { dir = '/tmp/km-cache' } })
    assert.are.equal('/tmp/km-cache', cache.root())
  end)

  it('names the three subdirectories (R-149)', function()
    config.setup({ cache = { dir = '/tmp/km-cache' } })
    assert.are.same({
      images = '/tmp/km-cache/images',
      remote = '/tmp/km-cache/remote',
      mermaid = '/tmp/km-cache/mermaid',
    }, cache.dirs())
  end)

  describe('keys', function()
    before_each(function()
      config.setup({ cache = { dir = '/c' } })
    end)

    it('hashes content and render parameters for an image (R-150)', function()
      local content = sha('png bytes')
      local k = sha(content .. '|800|0|0')
      assert.are.equal('/c/images/' .. k .. '.png', cache.key_image(content, 800))
      local limited = sha(content .. '|800|600|1200')
      assert.are.equal('/c/images/' .. limited .. '.png', cache.key_image(content, 800, 600, 1200))
    end)

    it('hashes the URL for a remote file without an extension (R-080)', function()
      local url = 'https://example.com/a.png'
      assert.are.equal('/c/remote/' .. sha(url), cache.key_remote(url))
    end)

    it('hashes source, width, theme, and mermaid version for a diagram (R-116)', function()
      local source = 'graph TD\n  A --> B\n'
      local k = sha(sha(source) .. '|640|dark|12.2.0')
      assert.are.equal(
        '/c/mermaid/' .. k .. '.png',
        cache.key_mermaid(source, 640, 'dark', '12.2.0')
      )
      assert.are_not.equal(
        cache.key_mermaid(source, 640, 'dark', '12.2.0'),
        cache.key_mermaid(source, 640, 'default', '12.2.0')
      )
    end)
  end)

  describe('estimate', function()
    it('starts at zero and adds result bytes', function()
      assert.are.equal(0, cache.estimate())
      cache.note_bytes(100)
      cache.note_bytes(50)
      assert.are.equal(150, cache.estimate())
    end)

    it('takes the probe size as the new baseline', function()
      cache.note_bytes(100)
      cache.note_probe(4096)
      assert.are.equal(4096, cache.estimate())
      cache.note_bytes(1)
      assert.are.equal(4097, cache.estimate())
    end)
  end)

  describe('keep set', function()
    local preview = require('kitty-markdown.preview')

    after_each(function()
      preview.in_use_paths = real_in_use_paths
    end)

    it('is empty when the preview has no in_use_paths', function()
      preview.in_use_paths = nil
      assert.are.same({}, cache.keep_set())
    end)

    it('passes the preview paths through (R-152)', function()
      preview.in_use_paths = function()
        return { '/c/images/a.png', '/c/mermaid/b.png' }
      end
      assert.are.same({ '/c/images/a.png', '/c/mermaid/b.png' }, cache.keep_set())
    end)
  end)

  describe('gc_if_needed', function()
    local preview = require('kitty-markdown.preview')
    local real_notify

    before_each(function()
      vim.env.KM_NVIM = vim.v.progpath
      helper.reset()
      helper.set_locator(function(cb)
        cb(fake, 'user')
      end)
      real_notify = vim.notify
      vim.notify = function() end
      config.setup({ cache = { max_bytes = 1000 } })
    end)

    after_each(function()
      helper.reset()
      vim.notify = real_notify
      preview.in_use_paths = real_in_use_paths
    end)

    it('sends nothing while the estimate is under the limit', function()
      cache.note_bytes(1000)
      local called = false
      assert.is_false(cache.gc_if_needed(function()
        called = true
      end))
      assert.is_false(called)
      assert.is_false(helper.is_running())
    end)

    it('sends one cache.gc with limit and keep set, then resets (R-151, R-202)', function()
      preview.in_use_paths = function()
        return { '/c/images/keep.png' }
      end
      cache.note_bytes(1500)
      local got
      assert.is_true(cache.gc_if_needed(function(err, result)
        got = { err = err, result = result }
      end))
      -- A second call while the first is in flight sends nothing more.
      assert.is_false(cache.gc_if_needed())
      assert.is_true(wait_for(function()
        return got ~= nil
      end))
      assert.is_nil(got.err)
      assert.are.same({ max_bytes = 1000, keep = { '/c/images/keep.png' } }, got.result.received)
      assert.are.equal(0, cache.estimate())
    end)

    it('leaves the estimate alone when the request fails', function()
      helper.set_locator(function(cb)
        cb(nil)
      end)
      cache.note_bytes(1500)
      local got
      assert.is_true(cache.gc_if_needed(function(err)
        got = err
      end))
      assert.is_true(wait_for(function()
        return got ~= nil
      end))
      assert.are.equal('no_helper', got.code)
      assert.are.equal(1500, cache.estimate())
      -- The flight is over, so the next batch may try again.
      assert.is_true(cache.gc_if_needed())
    end)
  end)
end)
