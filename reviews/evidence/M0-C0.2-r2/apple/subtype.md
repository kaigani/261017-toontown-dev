<!--
{
  "availability" : [
    "macOS: -"
  ],
  "documentType" : "symbol",
  "framework" : "AppKit",
  "identifier" : "/documentation/AppKit/NSEvent/subtype",
  "metadataVersion" : "0.1.0",
  "role" : "Instance Property",
  "symbol" : {
    "kind" : "Instance Property",
    "modules" : [
      "AppKit"
    ],
    "preciseIdentifier" : "c:objc(cs)NSEvent(py)subtype"
  },
  "title" : "subtype"
}
-->

# subtype

The event’s subtype.

```
var subtype: NSEvent.EventSubtype { get }
```

## Discussion

Access this property only if the event is a mouse event or if the [`type`](/documentation/AppKit/NSEvent/type) property contains [`NSAppKitDefined`](/documentation/AppKit/NSAppKitDefined), [`NSSystemDefined`](/documentation/AppKit/NSSystemDefined), [`NSApplicationDefined`](/documentation/AppKit/NSApplicationDefined), or [`NSPeriodic`](/documentation/AppKit/NSPeriodic). If you access this property for other types, AppKit raises <doc://com.apple.documentation/documentation/Foundation/NSExceptionName/internalInconsistencyException>. For information about predefined mouse and tablet subtypes, see `Getting Unicode Values`.

[`NSPeriodic`](/documentation/AppKit/NSPeriodic) events don’t use this property.

## See Also

[`var data1: Int`](/documentation/AppKit/NSEvent/data1)

Additional data associated with this event.

[`var data2: Int`](/documentation/AppKit/NSEvent/data2)

Additional data associated with this event.

[`class func otherEvent(with: NSEvent.EventType, location: NSPoint, modifierFlags: NSEvent.ModifierFlags, timestamp: TimeInterval, windowNumber: Int, context: NSGraphicsContext?, subtype: Int16, data1: Int, data2: Int) -> NSEvent?`](/documentation/AppKit/NSEvent/otherEvent(with:location:modifierFlags:timestamp:windowNumber:context:subtype:data1:data2:))

Creates and returns a new event object that describes a custom event.



---

Copyright &copy; 2026 Apple Inc. All rights reserved. | [Terms of Use](https://www.apple.com/legal/internet-services/terms/site.html) | [Privacy Policy](https://www.apple.com/privacy/privacy-policy)
