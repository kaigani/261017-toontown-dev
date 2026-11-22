//! Tauri build script: generates the application context (config, capabilities, icons, embedded assets).
//!
//! M4 C4.1: the native file pickers (tauri-plugin-dialog) need the Windows common controls v6. Tauri would embed that
//! dependency as a manifest resource of the application binary only; the test binaries of this crate would then fail
//! to load (`STATUS_ENTRYPOINT_NOT_FOUND`). So Tauri adds no manifest resource and every target of this crate (the
//! binary and the test harnesses) gets the same manifest from the linker.

fn main() {
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    if let Err(e) = tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)) {
        // The same failure tauri_build::build() reports (a build script has no other channel).
        println!("cargo:warning=tauri-build failed: {e:#}");
        std::process::exit(1);
    }
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|e| e == "msvc");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|o| o == "windows");
    if windows && msvc {
        let manifest =
            std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("common-controls.manifest");
        let text = concat!(
            "<assembly xmlns=\"urn:schemas-microsoft-com:asm.v1\" manifestVersion=\"1.0\"><dependency><dependentAssembly>",
            "<assemblyIdentity type=\"win32\" name=\"Microsoft.Windows.Common-Controls\" version=\"6.0.0.0\" ",
            "processorArchitecture=\"*\" publicKeyToken=\"6595b64144ccf1df\" language=\"*\"/>",
            "</dependentAssembly></dependency></assembly>"
        );
        if std::fs::write(&manifest, text).is_ok() {
            println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
            println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        }
    }
}
