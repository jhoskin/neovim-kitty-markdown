-- qa: serial
-- Pixel parity with the mermaid 12.2.0 oracle for the phase 1 diagram types.
-- Each check renders every hidden case of one type at the parity width,
-- crops the golden (oracle run with the R-237 --size) to the helper size
-- from the top left, and compares SSIM with no resampling.
local QA = require('qa')
local P = require('parity_util')

QA.describe('parity: phase 1 oracle SSIM', function()
  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 1 flowchart diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 1 },
    function()
      P.check_type('flowchart')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 1 sequence diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 1 },
    function()
      P.check_type('sequence')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 1 class diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 1 },
    function()
      P.check_type('class')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 1 state diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 1 },
    function()
      P.check_type('state')
    end
  )
end)
