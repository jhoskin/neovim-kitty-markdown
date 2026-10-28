// Prints the CoreGraphics window number and size of each normal window of
// a process. media/record.sh uses it to find the demo kitty window.

import CoreGraphics
let pid = Int32(CommandLine.arguments[1])!
let l = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as! [[String: Any]]
for w in l where (w[kCGWindowOwnerPID as String] as? Int32) == pid && (w[kCGWindowLayer as String] as? Int) == 0 {
  let b = w[kCGWindowBounds as String] as! [String: Any]
  print(w[kCGWindowNumber as String]!, b["Width"]!, b["Height"]!)
}
