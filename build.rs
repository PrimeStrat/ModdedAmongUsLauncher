/// Embed the Among Us icon into the exe so Explorer and the taskbar show it.
fn main() -> std::io::Result<()> {
    winresource::WindowsResource::new()
        .set_icon("Assets/Icon.ico")
        .compile()
}
