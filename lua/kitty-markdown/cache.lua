-- Cache paths, keys, and garbage collection requests (package L-E).
-- Signatures: AGENTS.md, Conventions, Lua module contracts.
--
-- The helper owns the files. This module only names them (R-148 to
-- R-150) and keeps the size estimate that decides when to ask the helper
-- for a `cache.gc` (R-151, R-152, R-202).

local config = require('kitty-markdown.config')

local M = {}

-- Bytes the plugin believes the cache holds: `cache_bytes` from the last
-- probe or gc answer plus the `bytes` of every uncached result since.
local estimate = 0
local gc_in_flight = false

--- The cache root: the configured directory (R-148).
function M.root()
  return config.get().cache.dir
end

--- The three subdirectories under the root (R-149).
function M.dirs()
  local root = M.root()
  return {
    images = root .. '/images',
    remote = root .. '/remote',
    mermaid = root .. '/mermaid',
  }
end

-- The key is one SHA-256 over the parts joined by `|`, so that a change
-- in any render parameter changes the file name (AGENTS.md, Cache keys).
local function key(parts)
  return vim.fn.sha256(table.concat(parts, '|'))
end

--- Path of a prepared image. `content_sha` is the SHA-256 of the source
--- bytes; an absent height or width limit counts as `0`.
function M.key_image(content_sha, width_px, max_h, max_w)
  local k = key({ content_sha, tostring(width_px), tostring(max_h or 0), tostring(max_w or 0) })
  return M.dirs().images .. '/' .. k .. '.png'
end

--- Path of a fetched remote file: raw bytes, no extension (R-080).
function M.key_remote(url)
  return M.dirs().remote .. '/' .. vim.fn.sha256(url)
end

--- Path of a mermaid render (R-116).
function M.key_mermaid(source, width_px, theme, mermaid_version)
  local k = key({ vim.fn.sha256(source), tostring(width_px), theme, mermaid_version })
  return M.dirs().mermaid .. '/' .. k .. '.png'
end

--- Add the bytes of an uncached result to the estimate.
function M.note_bytes(n)
  estimate = estimate + (n or 0)
end

--- Replace the estimate with the size the helper reported.
function M.note_probe(cache_bytes)
  estimate = cache_bytes or 0
end

--- The current size estimate in bytes.
function M.estimate()
  return estimate
end

--- Paths that open previews still show. The helper must not delete them
--- (R-152). The preview module may not be loaded, or may not expose the
--- function yet, so the fallback is an empty list.
function M.keep_set()
  local ok, preview = pcall(require, 'kitty-markdown.preview')
  if ok and type(preview.in_use_paths) == 'function' then
    return preview.in_use_paths() or {}
  end
  return {}
end

--- Send one `cache.gc` when the estimate exceeds the limit (R-151, R-202).
--- Returns true when a request was sent. `cb(err, result)` is optional and
--- runs only when a request was sent. A second call while one gc is in
--- flight sends nothing, so one render batch produces at most one gc.
function M.gc_if_needed(cb)
  local max_bytes = config.get().cache.max_bytes
  if estimate <= max_bytes or gc_in_flight then
    return false
  end
  gc_in_flight = true
  local helper = require('kitty-markdown.helper')
  helper.request('cache.gc', { max_bytes = max_bytes, keep = M.keep_set() }, function(err, result)
    gc_in_flight = false
    if not err and result and result.cache_bytes then
      estimate = result.cache_bytes
    end
    if cb then
      cb(err, result)
    end
  end)
  return true
end

--- Test hook: forget the estimate and any gc in flight.
function M.reset()
  estimate = 0
  gc_in_flight = false
end

return M
