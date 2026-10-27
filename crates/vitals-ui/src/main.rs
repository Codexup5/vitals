//! The entry of the window: Trunk builds this binary to WebAssembly and loads it into `index.html`.

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(vitals_ui::ui::App);
}
