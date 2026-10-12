//! Beholder desktop UI (Leptos CSR, wasm32). It renders state and forwards
//! user actions to the Tauri shell; it never talks to the network.

mod app;
mod i18n;
mod icons;
mod ipc;
mod screens;

fn main() {
    leptos::mount::mount_to_body(app::App);
}
