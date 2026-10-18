// Test prelude for the ACTUAL native_windows::pump body, appended by c02-r1-tooling.py.
// Only the event source and channel are doubles; conversion logic is read from product source on each run.
#![allow(dead_code, unused_imports)]
use std::cell::RefCell;
use std::time::Duration;

#[derive(Clone, Debug)]
struct NativePacket {
    kind: &'static str, host_epoch_ms: f64, device_ms: Option<f64>, x: f32, y: f32,
    pressure: Option<f32>, tilt_x: Option<f32>, tilt_y: Option<f32>, twist: Option<f32>, tool: String,
}
#[derive(Debug)]
struct NativeBatch { packets: Vec<NativePacket> }
#[derive(Default)]
struct Channel<T>(RefCell<Vec<T>>);
impl<T> Channel<T> {
    fn send(&self, value: T) -> Result<(), ()> { self.0.borrow_mut().push(value); Ok(()) }
}
fn epoch_ms() -> f64 { 1_800_000_000_000.0 }
struct Tool { name: Option<String> }
struct Axis(f32);
impl Axis { fn get(&self) -> Option<f32> { Some(self.0) } }
struct Pose { position: [f32; 2], pressure: Axis, tilt: Option<[f32; 2]>, roll: Axis }
enum ToolEvent { Pose(Pose), Frame(Option<octotablet::events::FrameTimestamp>), Down, Up, In {}, Out, Other }
enum Event { Tool { tool: Tool, event: ToolEvent }, Other }
struct Manager { events: Vec<Event> }
impl Manager { fn pump(&mut self) -> Result<Vec<Event>, std::convert::Infallible> { Ok(std::mem::take(&mut self.events)) } }
mod octotablet { pub mod events {
    #[derive(Clone, Copy)] pub struct FrameTimestamp(pub std::time::Duration);
    impl FrameTimestamp { pub fn epoch() -> Self { Self(std::time::Duration::ZERO) } }
    impl std::ops::Sub for FrameTimestamp { type Output = std::time::Duration;
        fn sub(self, rhs: Self) -> Self::Output { self.0 - rhs.0 } }
} }
thread_local! { static MANAGER: RefCell<Option<Manager>> = const { RefCell::new(None) }; }
fn event(event: ToolEvent) -> Event { Event::Tool { tool: Tool { name: Some("pen".into()) }, event } }
fn frame(at: u64) -> Vec<Event> {
    // octotablet 0.1.0 src/platform/ink/mod.rs emits Pose followed by Frame(timestamp).
    vec![event(ToolEvent::Pose(Pose { position: [1.0,2.0], pressure: Axis(0.5), tilt: None, roll: Axis(0.0) })),
         event(ToolEvent::Frame(Some(octotablet::events::FrameTimestamp(Duration::from_millis(at)))))]
}
#[test]
fn c02_r1_02_frame_timestamp_labels_preceding_pose() {
    MANAGER.with_borrow_mut(|slot| *slot = Some(Manager { events: [frame(100), frame(104)].concat_events() }));
    let channel = Channel(RefCell::new(Vec::new()));
    pump(&channel);
    let batches = channel.0.borrow();
    let times: Vec<_> = batches[0].packets.iter().filter(|p| p.kind == "pose").map(|p| p.device_ms).collect();
    assert_eq!(times, vec![Some(100.0), Some(104.0)]);
}
trait ConcatEvents { fn concat_events(self) -> Vec<Event>; }
impl<const N: usize> ConcatEvents for [Vec<Event>; N] {
    fn concat_events(self) -> Vec<Event> { self.into_iter().flatten().collect() }
}
#[test]
fn c02_r1_02_pump_boundary_does_not_erase_device_time() {
    let channel = Channel(RefCell::new(Vec::new()));
    for at in [100,104,108] {
        MANAGER.with_borrow_mut(|slot| *slot = Some(Manager { events: frame(at) }));
        pump(&channel);
    }
    let batches = channel.0.borrow();
    let times: Vec<_> = batches.iter().flat_map(|b| b.packets.iter()).filter(|p| p.kind=="pose").map(|p| p.device_ms).collect();
    assert_eq!(times, vec![Some(100.0),Some(104.0),Some(108.0)]);
}
