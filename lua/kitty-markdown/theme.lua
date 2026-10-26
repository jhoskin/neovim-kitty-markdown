-- GitHub light and dark markdown palettes as KittyMarkdown* highlight
-- groups (R-071 to R-076). Colors follow GitHub Primer's functional
-- variables: fg, muted, border, accent, success, attention, done, danger.

local config = require('kitty-markdown.config')

local M = {}

M.palettes = {
  light = {
    fg = '#1f2328',
    bg = '#ffffff',
    muted = '#59636e',
    border = '#d1d9e0',
    link = '#0969da',
    code_bg = '#f6f8fa',
    accent = '#0969da',
    success = '#1a7f37',
    attention = '#9a6700',
    done = '#8250df',
    danger = '#d1242f',
  },
  dark = {
    fg = '#f0f6fc',
    bg = '#0d1117',
    muted = '#9198a1',
    border = '#3d444d',
    link = '#4493f8',
    code_bg = '#151b23',
    accent = '#4493f8',
    success = '#3fb950',
    attention = '#d29922',
    done = '#ab7df8',
    danger = '#f85149',
  },
}

local function specs(p)
  return {
    KittyMarkdownH1 = { fg = p.fg, bold = true },
    KittyMarkdownH2 = { fg = p.fg, bold = true },
    KittyMarkdownH3 = { fg = p.fg, bold = true },
    KittyMarkdownH4 = { fg = p.fg, bold = true },
    KittyMarkdownH5 = { fg = p.fg, bold = true },
    KittyMarkdownH6 = { fg = p.muted, bold = true },
    KittyMarkdownText = { fg = p.fg },
    KittyMarkdownEmph = { fg = p.fg, italic = true },
    KittyMarkdownStrong = { fg = p.fg, bold = true },
    KittyMarkdownStrike = { fg = p.muted, strikethrough = true },
    KittyMarkdownCode = { fg = p.fg, bg = p.code_bg },
    KittyMarkdownCodeBg = { bg = p.code_bg },
    KittyMarkdownCodeLang = { fg = p.muted, bg = p.code_bg },
    KittyMarkdownCodeBorder = { fg = p.border },
    KittyMarkdownLink = { fg = p.link, underline = true },
    KittyMarkdownUrl = { fg = p.link, underline = true },
    KittyMarkdownBullet = { fg = p.muted },
    KittyMarkdownNumber = { fg = p.muted },
    KittyMarkdownChecked = { fg = p.success },
    KittyMarkdownUnchecked = { fg = p.muted },
    KittyMarkdownTableBorder = { fg = p.border },
    KittyMarkdownTableHeader = { fg = p.fg, bold = true },
    KittyMarkdownQuote = { fg = p.muted },
    KittyMarkdownQuoteBar = { fg = p.border },
    KittyMarkdownAlertNote = { fg = p.accent, bold = true },
    KittyMarkdownAlertTip = { fg = p.success, bold = true },
    KittyMarkdownAlertImportant = { fg = p.done, bold = true },
    KittyMarkdownAlertWarning = { fg = p.attention, bold = true },
    KittyMarkdownAlertCaution = { fg = p.danger, bold = true },
    KittyMarkdownRule = { fg = p.border },
    KittyMarkdownMuted = { fg = p.muted },
    KittyMarkdownMath = { fg = p.fg, italic = true },
    KittyMarkdownBoxBorder = { fg = p.border },
    KittyMarkdownBoxText = { fg = p.muted },
    KittyMarkdownLoading = { fg = p.attention },
    KittyMarkdownFailure = { fg = p.danger },
    KittyMarkdownFootnote = { fg = p.link },
    KittyMarkdownHtml = { fg = p.muted },
  }
end

--- Every group the theme defines, in a stable order.
M.groups = vim.tbl_keys(specs(M.palettes.light))
table.sort(M.groups)

-- What each group read back as right after the plugin last defined it.
-- A group whose current definition differs was set by someone else (the
-- user or a colorscheme), whenever that happened, and is never overwritten
-- (R-076, R-281). An empty group has no definition and is defined again.
-- The read-back value, not the spec, is compared, so `nvim_get_hl`
-- normalization (cterm attributes, links) cannot make a plugin group look
-- foreign.
local written = {}

local function ours(group, current)
  if next(current) == nil then
    return true
  end
  return written[group] ~= nil and vim.deep_equal(current, written[group])
end

local augroup = nil

-- A colorscheme runs `hi clear`, which empties every group, so the groups
-- are defined again on ColorScheme from the first apply on, not only once
-- a preview opens (R-282).
local function ensure_autocmd()
  if augroup then
    return
  end
  augroup = vim.api.nvim_create_augroup('kitty-markdown-theme', { clear = true })
  vim.api.nvim_create_autocmd('ColorScheme', {
    group = augroup,
    callback = function()
      M.apply()
    end,
  })
end

--- Resolve a theme name: 'auto' follows &background (R-072, R-073); 'light'
--- and 'dark' override it (R-074).
function M.resolve(name)
  if name == 'light' or name == 'dark' then
    return name
  end
  return vim.o.background == 'dark' and 'dark' or 'light'
end

--- The palette name in effect for the configured theme option.
function M.current()
  return M.resolve(config.get().theme)
end

--- The palette table for a name.
function M.palette(name)
  return M.palettes[M.resolve(name)]
end

--- Define the highlight groups for a palette. Called at setup and again on
--- every &background change (R-075) and colorscheme load (R-282);
--- re-applying overwrites the groups the plugin defined and the empty ones,
--- and leaves every other group alone.
function M.apply(name)
  ensure_autocmd()
  local palette = M.palette(name or config.get().theme)
  for group, spec in pairs(specs(palette)) do
    if ours(group, vim.api.nvim_get_hl(0, { name = group })) then
      vim.api.nvim_set_hl(0, group, spec)
      written[group] = vim.api.nvim_get_hl(0, { name = group })
    else
      written[group] = nil
    end
  end
end

--- The mermaid theme for the current palette: the configured one, or
--- `dark` on the dark palette and `default` on the light one (R-099 to
--- R-101).
function M.mermaid_theme()
  local wanted = config.get().mermaid.theme
  if wanted ~= 'auto' then
    return wanted
  end
  return M.current() == 'dark' and 'dark' or 'default'
end

--- Forget what the plugin wrote and clear every group. Tests use it to
--- start from a clean highlight table.
function M.reset()
  written = {}
  for _, group in ipairs(M.groups) do
    vim.api.nvim_set_hl(0, group, {})
  end
end

return M
