<!--
{
  "availability" : [
    "macOS: -"
  ],
  "documentType" : "symbol",
  "framework" : "AppKit",
  "identifier" : "/documentation/AppKit/NSEvent/tilt",
  "metadataVersion" : "0.1.0",
  "role" : "Instance Property",
  "symbol" : {
    "kind" : "Instance Property",
    "modules" : [
      "AppKit"
    ],
    "preciseIdentifier" : "c:objc(cs)NSEvent(py)tilt"
  },
  "title" : "tilt"
}
-->

# tilt

The scaled tilt values of the pointing device that generated this event.

```
var tilt: NSPoint { get }
```

## Discussion

This property’s value can range from -1.0 to 1.0 for both axes. A negative x-coordinate value indicates a tilt to the left and a positive value indicates a tilt to the right; a negative y-coordinate value indicates a tilt to the top and a positive value indicates a tilt to the bottom. If the device is perfectly perpendicular to the table surface, the values are 0.0 for both axes. This property is valid for mouse events with subtype `NSTabletPointEventSubtype` and for `NSTabletPoint` events.

## See Also

[`var pressure: Float`](/documentation/AppKit/NSEvent/pressure)

A normalized value that indicates the degree of pressure applied to an appropriate input device.



---

Copyright &copy; 2026 Apple Inc. All rights reserved. | [Terms of Use](https://www.apple.com/legal/internet-services/terms/site.html) | [Privacy Policy](https://www.apple.com/privacy/privacy-policy)
