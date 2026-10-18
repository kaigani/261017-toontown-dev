<!--
{
  "availability" : [
    "macOS: -"
  ],
  "documentType" : "symbol",
  "framework" : "AppKit",
  "identifier" : "/documentation/AppKit/NSEvent/pressure",
  "metadataVersion" : "0.1.0",
  "role" : "Instance Property",
  "symbol" : {
    "kind" : "Instance Property",
    "modules" : [
      "AppKit"
    ],
    "preciseIdentifier" : "c:objc(cs)NSEvent(py)pressure"
  },
  "title" : "pressure"
}
-->

# pressure

A normalized value that indicates the degree of pressure applied to an appropriate input device.

```
var pressure: Float { get }
```

## Discussion

For input devices that are pressure-sensitive, the value is increased as pressure is applied to the device.

For [`NSEvent.EventType.pressure`](/documentation/AppKit/NSEvent/EventType/pressure) events, pressure value relates to the current [`stage`](/documentation/AppKit/NSEvent/stage) of the gesture event. Each stage has its own pressure curve. For example, pressure ranges from `0.0` through `1.0` for a stage 1 event, and `0.0` through `1.0` for a stage 2 event. Pressure readings should be retrieved for a single stage of a gesture only, and should not be combined to achieve a wider range of pressure levels. In most cases, retrieving pressure during stage 1 is sufficient and appropriate for supporting variable input. Stage 1 pressure is the most physically comfortable for the user. Stage 2 pressure should only be used in rare circumstances where additional tactile feedback is necessary prior to retrieving pressure level.

For input devices that aren’t pressure-sensitive, the value is either `0.0` or `1.0`. An <doc://com.apple.documentation/documentation/Foundation/NSExceptionName/internalInconsistencyException> exception is raised if this property is accessed on an event other than a mouse-up, mouse-down, mouse-drag, [`NSTabletPoint`](/documentation/AppKit/NSTabletPoint), or [`NSEvent.EventType.pressure`](/documentation/AppKit/NSEvent/EventType/pressure) event.

For tablet pointing devices that are in proximity, the pressure value is `0.0` if the user is not actually touching the tablet.

Pressure is not intended to measure weight.

## See Also

[`class func mouseEvent(with: NSEvent.EventType, location: NSPoint, modifierFlags: NSEvent.ModifierFlags, timestamp: TimeInterval, windowNumber: Int, context: NSGraphicsContext?, eventNumber: Int, clickCount: Int, pressure: Float) -> NSEvent?`](/documentation/AppKit/NSEvent/mouseEvent(with:location:modifierFlags:timestamp:windowNumber:context:eventNumber:clickCount:pressure:))

Creates and returns a new event object that describes a mouse-down, -up, -moved, or -dragged event.

[`enum EventType`](/documentation/AppKit/NSEvent/EventType)

Constants for the types of events that responder objects can handle.

[`var rotation: Float`](/documentation/AppKit/NSEvent/rotation)

The rotation in degrees of the tablet pointing device associated with this event.

[`case pressure`](/documentation/AppKit/NSEvent/EventType/pressure)

An event that reports a change in pressure on a pressure-sensitive device.



---

Copyright &copy; 2026 Apple Inc. All rights reserved. | [Terms of Use](https://www.apple.com/legal/internet-services/terms/site.html) | [Privacy Policy](https://www.apple.com/privacy/privacy-policy)
