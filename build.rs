// Embeds the app icon into the Windows executable, so Explorer, the taskbar,
// pinned shortcuts and the portable ZIP show it (the runtime window icon is
// set separately in main.rs). No-op on other platforms.
fn main() {
    #[cfg(windows)]
    {
        winresource::WindowsResource::new()
            .set_icon("packaging/icons/icon.ico")
            .compile()
            .expect("embed Windows icon");
    }
}
