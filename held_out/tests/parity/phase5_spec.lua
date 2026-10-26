-- qa: serial
-- Pixel parity with the mermaid 12.2.0 oracle for the phase 5 diagram types.
-- Each check renders every hidden case of one type at the parity width,
-- crops the golden (oracle run with the R-237 --size) to the helper size
-- from the top left, and compares SSIM with no resampling.
local QA = require('qa')
local P = require('parity_util')

QA.describe('parity: phase 5 oracle SSIM', function()
  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 5 radar diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 5 },
    function()
      P.check_type('radar')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 5 treemap diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 5 },
    function()
      P.check_type('treemap')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 5 venn diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 5 },
    function()
      P.check_type('venn')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 5 ishikawa diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 5 },
    function()
      P.check_type('ishikawa')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 5 wardley diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 5 },
    function()
      P.check_type('wardley')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237', 'R-182' },
    'Phase 5 cynefin diagrams reach the SSIM threshold against at least one of the five oracle goldens',
    { phase = 5 },
    function()
      P.check_type('cynefin')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 5 treeview diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 5 },
    function()
      P.check_type('treeview')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 5 event modeling diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 5 },
    function()
      P.check_type('eventmodeling')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 5 use case diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 5 },
    function()
      P.check_type('usecase')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 5 swimlane diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 5 },
    function()
      P.check_type('swimlane')
    end
  )
end)
