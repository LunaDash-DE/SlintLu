//! `slintlu`: runs desktop shells written in the Slint language on Wayland compositors.

mod cli;
mod config;
mod scripting;

fn main() -> std::process::ExitCode {
    cli::main()
}
