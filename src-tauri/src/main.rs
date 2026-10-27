// No console window next to the panel in a release build
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    vitals_lib::run()
}
