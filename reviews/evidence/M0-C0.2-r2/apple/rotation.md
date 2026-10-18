<!--
{
  "availability" : [
    "macOS: -"
  ],
  "documentType" : "symbol",
  "framework" : "AppKit",
  "identifier" : "/documentation/AppKit/NSEvent/rotation",
  "metadataVersion" : "0.1.0",
  "role" : "Instance Property",
  "symbol" : {
    "kind" : "Instance Property",
    "modules" : [
      "AppKit"
    ],
    "preciseIdentifier" : "c:objc(cs)NSEvent(py)rotation"
  },
  "title" : "rotation"
}
-->

# rotation

The rotation in degrees of the tablet pointing device associated with this event.

```
var rotation: Float { get }
```

## Discussion

Many devices do not support rotation, in which case the returned value is 0.0. This property is valid only for mouse events with subtype `NSTabletPointEventSubtype` and for `NSTabletPoint` events.

## See Also

[`var pressure: Float`](/documentation/AppKit/NSEvent/pressure)

A normalized value that indicates the degree of pressure applied to an appropriate input device.



---

Copyright &copy; 2026 Apple Inc. All rights reserved. | [Terms of Use](https://www.apple.com/legal/internet-services/terms/site.html) | [Privacy Policy](https://www.apple.com/privacy/privacy-policy)
