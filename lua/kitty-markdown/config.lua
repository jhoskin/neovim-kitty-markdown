-- User configuration. The defaults are the README table, verbatim.

local M = {}

M.defaults = {
  placement = 'tab',
  theme = 'auto',
  text_width = 100,
  scroll_sync = true,
  remote_images = {
    enabled = true,
    timeout_ms = 10000,
    max_bytes = 20 * 1024 * 1024,
  },
  cache = {
    dir = vim.fn.stdpath('cache') .. '/kitty-markdown',
    max_bytes = 500 * 1024 * 1024,
  },
  helper = {
    path = nil,
    download = true,
    request_timeout_ms = 30000,
  },
  mermaid = {
    theme = 'auto',
  },
}

local placements = { 'split', 'tab', 'float' }
local themes = { 'auto', 'light', 'dark' }
local mermaid_themes = { 'auto', 'default', 'dark', 'neutral', 'forest', 'base' }

local current = nil

local function check_choice(key, value, choices)
  if not vim.tbl_contains(choices, value) then
    error(
      string.format(
        'kitty-markdown: invalid %s %s, expected one of %s',
        key,
        vim.inspect(value),
        table.concat(choices, ', ')
      ),
      0
    )
  end
end

local function check_positive(key, value)
  if type(value) ~= 'number' or value <= 0 or value % 1 ~= 0 then
    error(
      string.format(
        'kitty-markdown: invalid %s %s, expected a positive integer',
        key,
        vim.inspect(value)
      ),
      0
    )
  end
end

local function validate(cfg)
  check_choice('placement', cfg.placement, placements)
  check_choice('theme', cfg.theme, themes)
  check_choice('mermaid.theme', cfg.mermaid.theme, mermaid_themes)
  check_positive('text_width', cfg.text_width)
  check_positive('remote_images.timeout_ms', cfg.remote_images.timeout_ms)
  check_positive('remote_images.max_bytes', cfg.remote_images.max_bytes)
  check_positive('cache.max_bytes', cfg.cache.max_bytes)
  check_positive('helper.request_timeout_ms', cfg.helper.request_timeout_ms)
  if type(cfg.cache.dir) ~= 'string' or cfg.cache.dir == '' then
    error('kitty-markdown: invalid cache.dir, expected a path', 0)
  end
  if cfg.helper.path ~= nil and type(cfg.helper.path) ~= 'string' then
    error('kitty-markdown: invalid helper.path, expected a path or nil', 0)
  end
end

--- Merge user options over the defaults, validate, and make the result
--- current. Returns the merged table.
function M.setup(user)
  local cfg = vim.tbl_deep_extend('force', vim.deepcopy(M.defaults), user or {})
  validate(cfg)
  current = cfg
  return cfg
end

--- The current configuration; the defaults when setup was never called.
function M.get()
  if not current then
    M.setup({})
  end
  return current
end

return M
