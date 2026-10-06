#![forbid(unsafe_code)]

fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("packaging/windows/mdns-tui-browser.ico");
        if let Err(error) = resource.compile() {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
