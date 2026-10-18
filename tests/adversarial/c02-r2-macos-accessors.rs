// AppKit contract double, not an AppKit execution test. Python appends the
// unchanged production functions from is_mouse_type through the end of the file.
#![allow(dead_code, non_snake_case)]
#[derive(Clone, Copy, PartialEq, Debug)]
enum NSEventType {
    TabletPoint, TabletProximity, LeftMouseDown, LeftMouseUp, LeftMouseDragged,
    RightMouseDown, RightMouseUp, RightMouseDragged, OtherMouseDown, OtherMouseUp,
    OtherMouseDragged, MouseMoved,
}
#[derive(Clone, Copy, PartialEq, Debug)]
enum NSEventSubtype { Default, TabletPoint, TabletProximity }
#[derive(Clone, Copy)]
enum NSPointingDeviceType { Pen, Eraser, Cursor, Unknown }
struct Point { x: f64, y: f64 }
struct NSEvent { kind: NSEventType, subtype: NSEventSubtype, tool: NSPointingDeviceType, enter: bool }
impl NSEvent {
    fn r#type(&self) -> NSEventType { self.kind }
    fn timestamp(&self) -> f64 { 1.0 }
    fn subtype(&self) -> NSEventSubtype {
        assert!(is_mouse_type(self.kind), "subtype on {:?}", self.kind); self.subtype
    }
    fn pressure(&self) -> f32 {
        assert!(self.kind == NSEventType::TabletPoint ||
                (is_mouse_type(self.kind) && self.kind != NSEventType::MouseMoved),
                "pressure on {:?}", self.kind); 0.5
    }
    fn point_data(&self) -> bool {
        self.kind == NSEventType::TabletPoint ||
            (is_mouse_type(self.kind) && self.subtype == NSEventSubtype::TabletPoint)
    }
    fn proximity_data(&self) -> bool {
        self.kind == NSEventType::TabletProximity ||
            (is_mouse_type(self.kind) && self.subtype == NSEventSubtype::TabletProximity)
    }
    fn tilt(&self) -> Point { assert!(self.point_data()); Point { x: 0.5, y: -0.5 } }
    fn rotation(&self) -> f32 { assert!(self.point_data()); 42.0 }
    fn deviceID(&self) -> usize { assert!(self.point_data() || self.proximity_data()); 7 }
    fn pointingDeviceType(&self) -> NSPointingDeviceType { assert!(self.proximity_data()); self.tool }
    fn isEnteringProximity(&self) -> bool { assert!(self.proximity_data()); self.enter }
    fn windowNumber(&self) -> isize { 42 }
    // Apple's locationInWindow value is undefined for non-mouse events. This
    // deliberately does NOT certify coordinates or impose a made-up contract.
    fn locationInWindow(&self) -> Point { Point { x: 12.0, y: 34.0 } }
}
struct NativePacket { kind: &'static str, host_epoch_ms: f64, device_ms: Option<f64>,
    x: f32, y: f32, pressure: Option<f32>, tilt_x: Option<f32>, tilt_y: Option<f32>,
    twist: Option<f32>, tool: String }
struct WirePacket { packet: NativePacket, raw_tilt_x: Option<f64>, raw_tilt_y: Option<f64>,
    device_id: Option<String>, window_number: Option<i64> }
struct Sequencer;
impl Sequencer {
    fn wrap(&self, packet: NativePacket) -> WirePacket {
        WirePacket { packet, raw_tilt_x: None, raw_tilt_y: None, device_id: None, window_number: None }
    }
}
fn epoch_ms() -> f64 { 1000.0 }

#[test]
fn all_monitored_types_and_mouse_subtypes_obey_accessor_contracts() {
    for kind in [NSEventType::TabletPoint, NSEventType::TabletProximity,
        NSEventType::LeftMouseDown, NSEventType::LeftMouseUp, NSEventType::LeftMouseDragged,
        NSEventType::RightMouseDown, NSEventType::RightMouseUp, NSEventType::RightMouseDragged,
        NSEventType::OtherMouseDown, NSEventType::OtherMouseUp, NSEventType::OtherMouseDragged,
        NSEventType::MouseMoved] {
        for subtype in [NSEventSubtype::Default, NSEventSubtype::TabletPoint, NSEventSubtype::TabletProximity] {
            let e = NSEvent { kind, subtype, tool: NSPointingDeviceType::Pen, enter: true };
            let p = capture(&e, &Sequencer).expect("monitored input captured");
            assert_eq!(p.window_number, Some(42));
            if e.point_data() {
                assert_eq!(p.raw_tilt_x, Some(0.5));
                assert_eq!(p.raw_tilt_y, Some(-0.5));
                assert_eq!(p.device_id.as_deref(), Some("7"));
            }
        }
    }
}

#[test]
fn native_proximity_tool_identity_is_carried_to_pose_with_raw_tilt() {
    for (tool, label) in [(NSPointingDeviceType::Pen,"pen"), (NSPointingDeviceType::Eraser,"eraser"),
                          (NSPointingDeviceType::Cursor,"cursor")] {
        let enter = NSEvent { kind: NSEventType::TabletProximity, subtype: NSEventSubtype::Default, tool, enter: true };
        assert_eq!(capture(&enter, &Sequencer).unwrap().packet.kind, "in");
        let pose = NSEvent { kind: NSEventType::TabletPoint, ..enter };
        let p = capture(&pose, &Sequencer).unwrap();
        assert_eq!(p.packet.tool, label);
        assert_eq!(p.packet.pressure, Some(0.5));
        assert_eq!(p.raw_tilt_x, Some(0.5));
        let leave = NSEvent { kind: NSEventType::TabletProximity, enter: false, ..pose };
        assert_eq!(capture(&leave, &Sequencer).unwrap().packet.kind, "out");
    }
}
