use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::{Brand, LangSwitch, Screen, error_key, use_app};
use crate::i18n::use_i18n;
use crate::ipc;

#[derive(serde::Serialize)]
struct LoginArgs {
    username: String,
    password: String,
}

#[derive(serde::Serialize)]
struct RegisterArgs {
    username: String,
    display_name: String,
    password: String,
}

#[component]
pub fn AuthScreen() -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let register = RwSignal::new(false);
    let username = RwSignal::new(String::new());
    let display_name = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(None::<&'static str>);
    let busy = RwSignal::new(false);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            let result = if register.get_untracked() {
                ipc::call::<()>(
                    "register",
                    RegisterArgs {
                        username: username.get_untracked(),
                        display_name: display_name.get_untracked(),
                        password: password.get_untracked(),
                    },
                )
                .await
            } else {
                ipc::call::<()>(
                    "login",
                    LoginArgs {
                        username: username.get_untracked(),
                        password: password.get_untracked(),
                    },
                )
                .await
            };
            busy.set(false);
            match result {
                Ok(()) => {
                    password.set(String::new());
                    ctx.boot.update(|b| b.signed_in = true);
                    ctx.refresh();
                    ctx.screen.set(Screen::Library);
                }
                Err(e) => error.set(Some(error_key(e))),
            }
        });
    };

    view! {
        <main class="centered">
            <form class="panel" on:submit=submit>
                <div class="field-row">
                    <Brand/>
                    <span class="grow"></span>
                    <LangSwitch/>
                </div>
                <div class="tabs" role="tablist">
                    <button type="button" role="tab" class:active=move || !register.get()
                        aria-selected=move || (!register.get()).to_string()
                        on:click=move |_| register.set(false)>
                        {move || i18n.t("auth-login")}
                    </button>
                    <button type="button" role="tab" class:active=move || register.get()
                        aria-selected=move || register.get().to_string()
                        on:click=move |_| register.set(true)>
                        {move || i18n.t("auth-register")}
                    </button>
                </div>
                <label>
                    {move || i18n.t("username")}
                    <input type="text" required autocomplete="username" bind:value=username/>
                </label>
                <Show when=move || register.get()>
                    <label>
                        {move || i18n.t("display-name")}
                        <input type="text" required autocomplete="nickname" bind:value=display_name/>
                    </label>
                </Show>
                <label>
                    {move || i18n.t("password")}
                    <input type="password" required minlength="8"
                        autocomplete=move || if register.get() { "new-password" } else { "current-password" }
                        bind:value=password/>
                </label>
                {move || error.get().map(|k| view! { <p class="error" role="alert">{move || i18n.t(k)}</p> })}
                <button class="primary" type="submit" disabled=busy>
                    {move || if register.get() { i18n.t("auth-register") } else { i18n.t("auth-login") }}
                </button>
                <button type="button" disabled title=move || i18n.t("auth-discord-soon")>
                    {move || i18n.t("auth-discord")}
                </button>
                <p class="muted">{move || i18n.t("auth-discord-soon")}</p>
                <button type="button" class="link" on:click=move |_| ctx.screen.set(Screen::Server)>
                    {move || i18n.t("change-server")}
                </button>
            </form>
        </main>
    }
}
