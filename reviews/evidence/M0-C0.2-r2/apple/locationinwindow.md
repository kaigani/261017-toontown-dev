<!--
{
  "availability" : [
    "macOS: -"
  ],
  "documentType" : "symbol",
  "framework" : "AppKit",
  "identifier" : "/documentation/AppKit/NSEvent/locationInWindow",
  "metadataVersion" : "0.1.0",
  "role" : "Instance Property",
  "symbol" : {
    "kind" : "Instance Property",
    "modules" : [
      "AppKit"
    ],
    "preciseIdentifier" : "c:objc(cs)NSEvent(py)locationInWindow"
  },
  "title" : "locationInWindow"
}
-->

# locationInWindow

The event location in the base coordinate system of the associated window.

```
var locationInWindow: NSPoint { get }
```

## Discussion

This property applies to mouse events. For non-mouse events the value of this property is undefined.

With [`NSMouseMoved`](/documentation/AppKit/NSMouseMoved) and possibly other events, the event can have a `nil` window (that is, the [`window`](/documentation/AppKit/NSEvent/window) property contains nil). In this case, this property contains the event location in screen coordinates.

In a method of a custom view that handles mouse events, you commonly use this property with the [`convert(_:from:)`](/documentation/AppKit/NSView/convert(_:from:)-1dq9l) method of [`NSView`](/documentation/AppKit/NSView) to get the mouse location in the view’s coordinate system. The following code shows how to perform this conversion. The y coordinate in the returned point starts from a base of 1, and not 0.

```objc
NSPoint event_location = theEvent.locationInWindow;
NSPoint local_point = [self convertPoint:event_location fromView:nil];
```

---

Copyright &copy; 2026 Apple Inc. All rights reserved. | [Terms of Use](https://www.apple.com/legal/internet-services/terms/site.html) | [Privacy Policy](https://www.apple.com/privacy/privacy-policy)
