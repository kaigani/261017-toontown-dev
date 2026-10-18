<!--
{
  "availability" : [
    "macOS: -"
  ],
  "documentType" : "symbol",
  "framework" : "AppKit",
  "identifier" : "/documentation/AppKit/NSEvent/pointingDeviceType-swift.property",
  "metadataVersion" : "0.1.0",
  "role" : "Instance Property",
  "symbol" : {
    "kind" : "Instance Property",
    "modules" : [
      "AppKit"
    ],
    "preciseIdentifier" : "c:objc(cs)NSEvent(py)pointingDeviceType"
  },
  "title" : "pointingDeviceType"
}
-->

# pointingDeviceType

The kind of pointing device associated with this event.

```
var pointingDeviceType: NSEvent.PointingDeviceType { get }
```

## Discussion

For example, the device could be a pen, eraser, or cursor pointing device. This property is valid for mouse events with subtype `NSTabletProximityEventSubtype` and for `NSTabletProximity` events. See `Getting Unicode Values` for descriptions of valid NSPointingDeviceType constants.

---

Copyright &copy; 2026 Apple Inc. All rights reserved. | [Terms of Use](https://www.apple.com/legal/internet-services/terms/site.html) | [Privacy Policy](https://www.apple.com/privacy/privacy-policy)
