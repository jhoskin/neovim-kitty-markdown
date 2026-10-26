-- qa: serial
-- Pixel parity with the mermaid 12.2.0 oracle for the phase 4 diagram types.
-- Each check renders every hidden case of one type at the parity width,
-- frames it as mermaid-cli frames its screenshot, and compares SSIM.
local QA = require('qa')
local P = require('parity_util')

QA.describe('parity: phase 4 oracle SSIM', function()
  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 4 sankey diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 4 },
    function()
      P.check_type('sankey')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 4 block diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 4 },
    function()
      P.check_type('block')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 4 packet diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 4 },
    function()
      P.check_type('packet')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 4 kanban diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 4 },
    function()
      P.check_type('kanban')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 4 architecture diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 4 },
    function()
      P.check_type('architecture')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 4 C4 diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 4 },
    function()
      P.check_type('c4')
    end
  )

  QA.check(
    { 'R-110', 'R-181', 'R-236', 'R-237' },
    'Phase 4 requirement diagrams match the oracle at the parity width: the same pixel size within 1 px and an SSIM at or above the threshold',
    { phase = 4 },
    function()
      P.check_type('requirement')
    end
  )
end)
