-- qa: serial
-- Pixel parity with the mermaid 12.2.0 oracle for the phase 3 diagram types.
-- Each check renders every hidden case of one type at the parity width,
-- crops the golden (oracle run with the R-237 --size) to the helper size
-- from the top left, and compares SSIM with no resampling.
local QA = require('qa')
local P = require('parity_util')

QA.describe('parity: phase 3 oracle SSIM', function()
  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 3 gitgraph diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 3 },
    function()
      P.check_type('gitgraph')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237', 'R-182' },
    'Phase 3 mindmap diagrams reach the SSIM threshold against at least one of the five oracle goldens',
    { phase = 3 },
    function()
      P.check_type('mindmap')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 3 timeline diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 3 },
    function()
      P.check_type('timeline')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 3 user journey diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 3 },
    function()
      P.check_type('journey')
    end
  )
end)
