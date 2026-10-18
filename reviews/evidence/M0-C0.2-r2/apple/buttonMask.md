<!--
{
  "availability" : [
    "macOS: -"
  ],
  "documentType" : "symbol",
  "framework" : "AppKit",
  "identifier" : "/documentation/AppKit/NSEvent/buttonMask-swift.property",
  "metadataVersion" : "0.1.0",
  "role" : "Instance Property",
  "symbol" : {
    "kind" : "Instance Property",
    "modules" : [
      "AppKit"
    ],
    "preciseIdentifier" : "c:objc(cs)NSEvent(py)buttonMask"
  },
  "title" : "buttonMask"
}
-->

# buttonMask

A bit mask identifying the buttons pressed for a tablet event.

```
var buttonMask: NSEvent.ButtonMask { get }
```

## Discussion

Use one or more of the button-mask constants described in `Getting Unicode Values` to determine which of the pointing device’s buttons are pressed. This property is valid only for mouse events with a subtype of `NSTabletPointEventSubtype` and for events of type `NSTabletPoint`; otherwise, the property is set to `0`.

---

Copyright &copy; 2026 Apple Inc. All rights reserved. | [Terms of Use](https://www.apple.com/legal/internet-services/terms/site.html) | [Privacy Policy](https://www.apple.com/privacy/privacy-policy)
