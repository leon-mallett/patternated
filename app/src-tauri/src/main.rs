// Windows release builds should not open a console behind the window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pattern_app_lib::run()
}
