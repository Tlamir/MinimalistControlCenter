#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
mod ui;
#[cfg(windows)]
fn main() {
    ui::run();
}
#[cfg(not(windows))]
fn main() {
    eprintln!("The native control center UI requires Windows. The diagnostic CLI is portable.");
}
