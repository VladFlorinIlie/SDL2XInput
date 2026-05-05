use std::io::{self, Write};
use std::env;
use std::path::Path;
use std::process::Command;

fn main() -> io::Result<()> {
    // Tell Cargo to re-run this script if any viiper source files change
    println!("cargo:rerun-if-changed=viiper/");

    // Build libVIIPER using the Go toolchain
    let status = Command::new("go")
        .current_dir("viiper/lib/viiper")
        .env("CGO_ENABLED", "1")
        .args(&["build", "-buildmode=c-shared", "-o", "libviiper.dll", "."])
        .status()
        .expect("Failed to execute `go build`. Ensure the Go toolchain is installed.");

    if !status.success() {
        panic!("Failed to build libviiper.dll. Go compiler returned an error.");
    }

    // Copy the generated DLL to the target directory (alongside the executable)
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = Path::new(&out_dir).ancestors().nth(3).unwrap();
    std::fs::copy(
        "viiper/lib/viiper/libviiper.dll",
        target_dir.join("libviiper.dll"),
    )?;

    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        embed_icon(&out_dir)?;
    }
    Ok(())
}

/// Embeds the application icon by writing a .rc file and compiling it with
/// windres (MinGW) or the MSVC resource compiler. Using a hand-written .rc
/// file avoids the syntax incompatibilities winres generates for windres.
fn embed_icon(out_dir: &str) -> io::Result<()> {
    // Resolve the icon path relative to the manifest directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let icon_path = Path::new(&manifest_dir).join("assets").join("icon.ico");

    // Write a minimal .rc file. ID 1 is what Windows Explorer looks for.
    let rc_path = Path::new(out_dir).join("icon.rc");
    let mut rc_file = std::fs::File::create(&rc_path)?;
    writeln!(rc_file, "1 ICON \"{}\"", icon_path.to_str().unwrap().replace('\\', "/"))?;
    drop(rc_file);

    // Compile the .rc file into a .res object
    let res_path = Path::new(out_dir).join("icon.res");

    let compiler = find_resource_compiler();
    let status = Command::new(&compiler)
        .args(&[
            rc_path.to_str().unwrap(),
            "-O", "coff",
            "-o", res_path.to_str().unwrap(),
        ])
        .status()
        .unwrap_or_else(|_| panic!("Failed to run resource compiler: {}", compiler));

    if !status.success() {
        panic!("Resource compiler failed to process icon.rc");
    }

    // Tell rustc to link this resource object
    println!("cargo:rustc-link-arg={}", res_path.to_str().unwrap());
    Ok(())
}

/// Returns the resource compiler to use: windres.exe on MinGW, rc.exe on MSVC.
fn find_resource_compiler() -> String {
    // Check for windres in PATH (MinGW)
    if Command::new("windres").arg("--version").output().is_ok() {
        return "windres".to_string();
    }
    // Try explicit MinGW location from Chocolatey
    let mingw = r"C:\ProgramData\mingw64\mingw64\bin\windres.exe";
    if Path::new(mingw).exists() {
        return mingw.to_string();
    }
    // Fall back to MSVC rc.exe
    "rc".to_string()
}
