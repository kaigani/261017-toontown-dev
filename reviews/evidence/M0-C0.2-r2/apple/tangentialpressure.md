<!--
{
  "availability" : [
    "macOS: -"
  ],
  "documentType" : "symbol",
  "framework" : "AppKit",
  "identifier" : "/documentation/AppKit/NSEvent/tangentialPressure",
  "metadataVersion" : "0.1.0",
  "role" : "Instance Property",
  "symbol" : {
    "kind" : "Instance Property",
    "modules" : [
      "AppKit"
    ],
    "preciseIdentifier" : "c:objc(cs)NSEvent(py)tangentialPressure"
  },
  "title" : "tangentialPressure"
}
-->

# tangentialPressure

The tangential pressure on the device that generated this event.

```
var tangentialPressure: Float { get }
```

## Discussion

The property’s value can range from -1.0 to 1.0. Tangential pressure is also known as barrel pressure. Only some pointing devices support tangential pressure. This method is valid for mouse events with subtype `NSTabletPointEventSubtype` and for `NSTabletPoint` events.

## See Also

[`var pressure: Float`](/documentation/AppKit/NSEvent/pressure)

A normalized value that indicates the degree of pressure applied to an appropriate input device.



---

Copyright &copy; 2026 Apple Inc. All rights reserved. | [Terms of Use](https://www.apple.com/legal/internet-services/terms/site.html) | [Privacy Policy](https://www.apple.com/privacy/privacy-policy)
