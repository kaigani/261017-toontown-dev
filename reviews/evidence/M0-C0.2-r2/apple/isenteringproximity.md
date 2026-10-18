<!--
{
  "availability" : [
    "macOS: -"
  ],
  "documentType" : "symbol",
  "framework" : "AppKit",
  "identifier" : "/documentation/AppKit/NSEvent/isEnteringProximity",
  "metadataVersion" : "0.1.0",
  "role" : "Instance Property",
  "symbol" : {
    "kind" : "Instance Property",
    "modules" : [
      "AppKit"
    ],
    "preciseIdentifier" : "c:objc(cs)NSEvent(py)enteringProximity"
  },
  "title" : "isEnteringProximity"
}
-->

# isEnteringProximity

A Boolean value that indicates whether a pointing device is entering or leaving the proximity of its tablet.

```
var isEnteringProximity: Bool { get }
```

## Discussion

This property is set to <doc://com.apple.documentation/documentation/Swift/true> when the pointing device enters the proximity of its tablet and <doc://com.apple.documentation/documentation/Swift/false> when it leaves.

This method is valid only for mouse events with subtype `NSTabletProximityEventSubtype` and for `NSTabletProximity` events; otherwise it is set to <doc://com.apple.documentation/documentation/Swift/false>.

---

Copyright &copy; 2026 Apple Inc. All rights reserved. | [Terms of Use](https://www.apple.com/legal/internet-services/terms/site.html) | [Privacy Policy](https://www.apple.com/privacy/privacy-policy)
