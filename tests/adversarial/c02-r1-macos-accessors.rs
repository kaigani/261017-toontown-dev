// Prelude for the unchanged native_macos::device_name + convert functions, appended by Python.
// These doubles enforce Apple's documented accessor preconditions. This is NOT execution of AppKit.
#![allow(dead_code, non_snake_case)]
#[derive(Clone, Copy, PartialEq)]
enum NSEventType { TabletProximity, TabletPoint, LeftMouseDown, LeftMouseUp, LeftMouseDragged, MouseMoved, Other }
#[derive(Clone, Copy, PartialEq)]
enum NSEventSubtype { TabletProximity, TabletPoint, Default }
#[derive(Clone, Copy)]
enum NSPointingDeviceType { Pen, Eraser, Cursor, Unknown }
struct Point { x: f64, y: f64 }
struct NSEvent { kind: NSEventType, subtype: NSEventSubtype }
impl NSEvent {
    fn r#type(&self) -> NSEventType { self.kind }
    fn timestamp(&self) -> f64 { 1.0 }
    fn isEnteringProximity(&self) -> bool { true }
    fn pointingDeviceType(&self) -> NSPointingDeviceType { NSPointingDeviceType::Pen }
    fn subtype(&self) -> NSEventSubtype {
        assert!(matches!(self.kind, NSEventType::LeftMouseDown | NSEventType::LeftMouseUp |
            NSEventType::LeftMouseDragged | NSEventType::MouseMoved),
            "AppKit subtype contract: native tablet events are not mouse/system/periodic events");
        self.subtype
    }
    fn pressure(&self) -> f32 {
        assert!(matches!(self.kind, NSEventType::LeftMouseDown | NSEventType::LeftMouseUp |
            NSEventType::LeftMouseDragged | NSEventType::TabletPoint),
            "AppKit pressure contract: ordinary MouseMoved is not an allowed event type");
        0.5
    }
    fn locationInWindow(&self) -> Point { Point { x: 1.0, y: 2.0 } }
    fn tilt(&self) -> Point { Point { x: 0.0, y: 0.0 } }
    fn rotation(&self) -> f32 { 0.0 }
}
struct NativePacket { kind: &'static str, host_epoch_ms: f64, device_ms: Option<f64>, x: f32, y: f32,
    pressure: Option<f32>, tilt_x: Option<f32>, tilt_y: Option<f32>, twist: Option<f32>, tool: String }
fn epoch_ms() -> f64 { 1_000_000.0 }
#[test]
fn c02_r1_08_ordinary_mouse_move_never_calls_tablet_only_accessors() {
    let event = NSEvent { kind: NSEventType::MouseMoved, subtype: NSEventSubtype::Default };
    let _ = convert(&event);
}
#[test]
fn c02_r1_08_native_tablet_point_never_calls_mouse_subtype() {
    let event = NSEvent { kind: NSEventType::TabletPoint, subtype: NSEventSubtype::Default };
    let packet = convert(&event).expect("tablet-point must be captured");
    assert_eq!(packet.tool,"pen");
}
#[test]
fn proximity_control_does_not_read_pressure() {
    let event = NSEvent { kind: NSEventType::TabletProximity, subtype: NSEventSubtype::Default };
    assert_eq!(convert(&event).unwrap().kind,"in");
}
