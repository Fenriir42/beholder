//! First launch: which server to use.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::{Brand, Screen, error_key, use_app};
use crate::i18n::use_i18n;
use crate::ipc;

#[derive(serde::Serialize)]
struct Args {
    url: String,
}

/// Asks for the server address and remembers it.
#[component]
pub fn ServerScreen() -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let url = RwSignal::new(ctx.boot.get_untracked().server_url.unwrap_or_default());
    let error = RwSignal::new(None::<&'static str>);
    let busy = RwSignal::new(false);
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        spawn_local(async move {
            match ipc::call::<String>(
                "set_server_url",
                Args {
                    url: url.get_untracked(),
                },
            )
            .await
            {
                Ok(normalized) => {
                    ctx.boot.update(|b| b.server_url = Some(normalized));
                    ctx.screen.set(Screen::Auth);
                }
                Err(e) => error.set(Some(error_key(e))),
            }
            busy.set(false);
        });
    };
    view! {
        <main class="centered">
            <form class="panel" on:submit=submit>
                <Brand/>
                <h1>{move || i18n.t("server-title")}</h1>
                <p class="muted">{move || i18n.t("server-hint")}</p>
                <label>
                    {move || i18n.t("server-url")}
                    <input type="url" required placeholder="https://" bind:value=url autofocus/>
                </label>
                {move || error.get().map(|k| view! { <p class="error" role="alert">{move || i18n.t(k)}</p> })}
                <button class="primary" type="submit" disabled=busy>{move || i18n.t("continue")}</button>
            </form>
        </main>
    }
}
