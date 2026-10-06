#![forbid(unsafe_code)]

fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("packaging/windows/mdns-tui-browser.ico");
        resource.set("FileDescription", env!("CARGO_PKG_DESCRIPTION"));
        resource.set("LegalCopyright", "Copyright 2026 hrzlgnm");
        resource.set("OriginalFilename", "mdns-tui-browser.exe");
        if let Err(error) = resource.compile() {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
