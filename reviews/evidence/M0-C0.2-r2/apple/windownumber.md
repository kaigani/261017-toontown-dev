<!--
{
  "availability" : [
    "macOS: -"
  ],
  "documentType" : "symbol",
  "framework" : "AppKit",
  "identifier" : "/documentation/AppKit/NSEvent/windowNumber",
  "metadataVersion" : "0.1.0",
  "role" : "Instance Property",
  "symbol" : {
    "kind" : "Instance Property",
    "modules" : [
      "AppKit"
    ],
    "preciseIdentifier" : "c:objc(cs)NSEvent(py)windowNumber"
  },
  "title" : "windowNumber"
}
-->

# windowNumber

The identifier for the window device associated with the event.

```
var windowNumber: Int { get }
```

## Discussion

Periodic events do not have a window. The result of accessing this property on a periodic event is undefined.

---

Copyright &copy; 2026 Apple Inc. All rights reserved. | [Terms of Use](https://www.apple.com/legal/internet-services/terms/site.html) | [Privacy Policy](https://www.apple.com/privacy/privacy-policy)
