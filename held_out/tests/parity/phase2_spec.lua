-- qa: serial
-- Pixel parity with the mermaid 12.2.0 oracle for the phase 2 diagram types.
-- Each check renders every hidden case of one type at the parity width,
-- frames it as mermaid-cli frames its screenshot, and compares SSIM.
local QA = require('qa')
local P = require('parity_util')

QA.describe('parity: phase 2 oracle SSIM', function()
  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 2 ER diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 2 },
    function()
      P.check_type('er')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 2 pie diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 2 },
    function()
      P.check_type('pie')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 2 gantt diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 2 },
    function()
      P.check_type('gantt')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 2 xychart diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 2 },
    function()
      P.check_type('xychart')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 2 quadrant diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 2 },
    function()
      P.check_type('quadrant')
    end
  )
end)
