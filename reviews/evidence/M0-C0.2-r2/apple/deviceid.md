<!--
{
  "availability" : [
    "macOS: -"
  ],
  "documentType" : "symbol",
  "framework" : "AppKit",
  "identifier" : "/documentation/AppKit/NSEvent/deviceID",
  "metadataVersion" : "0.1.0",
  "role" : "Instance Property",
  "symbol" : {
    "kind" : "Instance Property",
    "modules" : [
      "AppKit"
    ],
    "preciseIdentifier" : "c:objc(cs)NSEvent(py)deviceID"
  },
  "title" : "deviceID"
}
-->

# deviceID

A special identifier the system matches against tablet-pointer and tablet-proximity events.

```
var deviceID: Int { get }
```

## Discussion

All tablet-pointer events generated in the period between the device entering and leaving tablet proximity have the same device ID. This property is valid only for mouse events with subtype `NSTabletPointEventSubtype` or `NSTabletProximityEventSubtype`, and for `NSTabletPoint` and `NSTabletProximity` events.

---

Copyright &copy; 2026 Apple Inc. All rights reserved. | [Terms of Use](https://www.apple.com/legal/internet-services/terms/site.html) | [Privacy Policy](https://www.apple.com/privacy/privacy-policy)
