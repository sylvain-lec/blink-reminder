fn main() {
    // Embed the app icon into the Windows .exe. No-op on other platforms.
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            eprintln!("warning: failed to embed Windows icon: {e}");
        }
    }
}
