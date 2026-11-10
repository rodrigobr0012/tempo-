//! Versão web do Tempo: interface Leptos sobre o mesmo `tempo-core`/`tempo-store` do Android,
//! compilada para WebAssembly. Os dados ficam no navegador (IndexedDB), sem servidor.

mod app;
mod browser;
mod dialogs;
mod pages;
mod state;
mod ui;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
