//! Finder entry point for Lowfi's terminal application bundle.
//!
//! Open `Resources/Lowfi.command` in Terminal.app; that script executes the bundled
//! `Contents/MacOS/lowfi`. This helper waits for `/usr/bin/open`, not for playback
//! to finish. Resolve the script from this executable so the bundle can be moved.
//!
//! To pass command-line options, invoke the bundled `lowfi` executable directly.

#[cfg(target_os = "macos")]
fn main() -> std::io::Result<()> {
    use std::{env, io, process::Command};

    let executable = env::current_exe()?;
    let macos = executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Lowfi launcher has no MacOS directory",
        )
    })?;
    let contents = macos.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Lowfi launcher has no Contents directory",
        )
    })?;
    let command = contents.join("Resources").join("Lowfi.command");

    if !command.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Lowfi terminal launch script is missing: {}",
                command.display()
            ),
        ));
    }

    let status = Command::new("/usr/bin/open")
        .args(["-b", "com.apple.Terminal"])
        .arg(command)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "could not open Lowfi in Terminal (status {status})"
        )))
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("lowfi-app-launcher is only available on macOS");
    std::process::exit(1);
}
