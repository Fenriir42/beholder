use beholder_ipc::CampaignView;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::{Screen, TopBar, error_key, image_url, use_app};
use crate::i18n::use_i18n;
use crate::{icons, ipc};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dialog {
    None,
    Join,
    Create,
}

#[derive(serde::Serialize)]
struct TitleArgs {
    title: String,
}

#[derive(serde::Serialize)]
struct CodeArgs {
    code: String,
}

#[component]
pub fn LibraryScreen() -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let query = RwSignal::new(String::new());
    let dialog = RwSignal::new(Dialog::None);

    let campaigns = move || {
        let q = query.get().to_lowercase();
        ctx.view
            .get()
            .campaigns
            .into_iter()
            .filter(|c| q.is_empty() || c.campaign.title.as_str().to_lowercase().contains(&q))
            .collect::<Vec<_>>()
    };
    let pending_joins = move || ctx.view.get().pending_joins;

    view! {
        <TopBar/>
        <main class="page">
            <h1>{move || i18n.t("library-title")}</h1>
            <div class="toolbar">
                <input type="search" placeholder=move || i18n.t("search")
                    aria-label=move || i18n.t("search") bind:value=query/>
                <span class="grow"></span>
                <button on:click=move |_| dialog.set(Dialog::Join)>
                    {icons::key()}
                    {move || i18n.t("join-with-code")}
                </button>
                <button class="primary" on:click=move |_| dialog.set(Dialog::Create)>
                    {icons::plus()}
                    {move || i18n.t("new-campaign")}
                </button>
            </div>
            <Show when=move || { pending_joins() > 0 }>
                <p class="muted" role="status">
                    {move || i18n.tr("joining", &[("count", pending_joins().into())])}
                </p>
            </Show>
            <Show when=move || ctx.view.get().campaigns.is_empty() && pending_joins() == 0>
                <p class="muted">{move || i18n.t("no-campaigns")}</p>
            </Show>
            <div class="grid">
                <For each=campaigns key=|c| (c.campaign.id, c.campaign.title.clone(), c.campaign.cover, c.pending)
                    children=move |c| view! { <Card c=c/> }/>
                <button class="card add" on:click=move |_| dialog.set(Dialog::Create)>
                    {icons::plus()}
                    <span>{move || i18n.t("join-or-create")}</span>
                </button>
            </div>
        </main>
        {move || match dialog.get() {
            Dialog::None => ().into_any(),
            Dialog::Join => view! { <JoinDialog close=move || dialog.set(Dialog::None)/> }.into_any(),
            Dialog::Create => view! { <CreateDialog close=move || dialog.set(Dialog::None)/> }.into_any(),
        }}
    }
}

#[component]
fn Card(c: CampaignView) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let id = c.campaign.id;
    let cover = c.campaign.cover.map(|h| image_url(&ctx, &h, "thumb"));
    let pending = c.pending;
    view! {
        <button class="card" on:click=move |_| ctx.screen.set(Screen::Campaign(id))>
            {match cover {
                Some(src) => view! { <img class="cover" src=src alt="" loading="lazy"/> }.into_any(),
                None => view! { <div class="cover-empty">{icons::image()}</div> }.into_any(),
            }}
            <span class="title">
                <span class="grow">{c.campaign.title.as_str().to_owned()}</span>
                {pending.then(|| view! {
                    <span class="pill" title=move || i18n.t("pending")>
                        <span class="dot warning"></span>
                        {move || i18n.t("pending")}
                    </span>
                })}
            </span>
        </button>
    }
}

#[component]
fn JoinDialog(close: impl Fn() + Copy + 'static) -> impl IntoView {
    let i18n = use_i18n();
    let code = RwSignal::new(String::new());
    let error = RwSignal::new(None::<&'static str>);
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            match ipc::call::<()>(
                "join_campaign",
                CodeArgs {
                    code: code.get_untracked(),
                },
            )
            .await
            {
                Ok(()) => close(),
                Err(e) => error.set(Some(error_key(e))),
            }
        });
    };
    view! {
        <div class="dialog" role="dialog" aria-modal="true">
            <form class="panel" on:submit=submit>
                <h2>{move || i18n.t("join-with-code")}</h2>
                <label>
                    {move || i18n.t("invite-code")}
                    <input type="text" required placeholder="ABCD-EFGH" autocomplete="off" bind:value=code autofocus/>
                </label>
                {move || error.get().map(|k| view! { <p class="error" role="alert">{move || i18n.t(k)}</p> })}
                <div class="actions">
                    <button type="button" on:click=move |_| close()>{move || i18n.t("cancel")}</button>
                    <button type="submit" class="primary">{move || i18n.t("join")}</button>
                </div>
            </form>
        </div>
    }
}

#[component]
fn CreateDialog(close: impl Fn() + Copy + 'static) -> impl IntoView {
    let i18n = use_i18n();
    let title = RwSignal::new(String::new());
    let error = RwSignal::new(None::<&'static str>);
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            match ipc::call::<String>(
                "create_campaign",
                TitleArgs {
                    title: title.get_untracked(),
                },
            )
            .await
            {
                Ok(_) => close(),
                Err(e) => error.set(Some(error_key(e))),
            }
        });
    };
    view! {
        <div class="dialog" role="dialog" aria-modal="true">
            <form class="panel" on:submit=submit>
                <h2>{move || i18n.t("new-campaign")}</h2>
                <label>
                    {move || i18n.t("campaign-title")}
                    <input type="text" required maxlength="80" bind:value=title autofocus/>
                </label>
                {move || error.get().map(|k| view! { <p class="error" role="alert">{move || i18n.t(k)}</p> })}
                <div class="actions">
                    <button type="button" on:click=move |_| close()>{move || i18n.t("cancel")}</button>
                    <button type="submit" class="primary">{move || i18n.t("create")}</button>
                </div>
            </form>
        </div>
    }
}
