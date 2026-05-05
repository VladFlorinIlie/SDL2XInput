use std::io;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set_icon_with_id("assets/icon.ico", "icon");
        res.compile()?;
    }
    Ok(())
}
