//! Reviewer negative control: an executable with an actual DirectComposition import.
//! Compile with rustc on Windows; inspecting this executable must fail the headless dependency proof.

#[link(name = "dcomp")]
unsafe extern "system" {
    fn DCompositionCreateDevice(
        device: *mut std::ffi::c_void,
        interface: *const std::ffi::c_void,
        result: *mut *mut std::ffi::c_void,
    ) -> i32;
}

fn main() {
    // Retain the import without calling it or creating a display/GPU device.
    std::hint::black_box(DCompositionCreateDevice as *const ());
}
