mod app;
mod i18n;
mod icons;
mod ipc;
mod markdown;
mod screens;

fn main() {
    leptos::mount::mount_to_body(app::App);
}
