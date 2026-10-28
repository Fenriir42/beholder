use beholder_core::{CampaignId, CampaignSettings, CharacterId, CombatVisibility, NoteId, Role};
use beholder_ipc::CampaignView;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::app::{ConnBanner, ConnPill, Screen, error_key, image_url, use_app};
use crate::i18n::use_i18n;
use crate::screens::compendium::{Compendium, SourcesSetting};
use crate::screens::notes::{Notes, NotesPreview};
use crate::screens::sheet::{CharacterList, summary_line};
use crate::screens::sheet_view::Sheet;
use crate::{icons, ipc};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Home,
    Sheet,
    Notes,
    Compendium,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Characters,
    Notes,
    Party,
    Settings,
}

#[derive(serde::Serialize)]
struct IdArgs {
    campaign_id: String,
}

#[derive(serde::Serialize)]
struct RenameArgs {
    campaign_id: String,
    title: String,
}

#[derive(serde::Serialize)]
struct SettingsArgs {
    campaign_id: String,
    settings: CampaignSettings,
}

#[component]
pub fn CampaignScreen(id: CampaignId) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let tab = RwSignal::new(Tab::Characters);
    let section = RwSignal::new(Section::Home);
    let selected_note = RwSignal::new(None::<NoteId>);
    let selected_character = RwSignal::new(None::<CharacterId>);
    let open_character = Callback::new(move |id: CharacterId| {
        selected_character.set(Some(id));
        section.set(Section::Sheet);
    });
    let open_note = Callback::new(move |id: NoteId| {
        selected_note.set(Some(id));
        section.set(Section::Notes);
    });
    let campaign = Memo::new(move |_| {
        ctx.view
            .get()
            .campaigns
            .into_iter()
            .find(|c| c.campaign.id == id)
    });
    let me = move || ctx.view.get().user.map(|u| u.id);
    let is_dm = Memo::new(move |_| {
        campaign
            .get()
            .zip(me())
            .and_then(|(c, me)| c.campaign.role_of(me))
            == Some(Role::Dm)
    });

    let nav_item = move |label: &'static str, icon: AnyView, target: Option<Section>| {
        let enabled = target.is_some();
        view! {
            <button class:active=move || target == Some(section.get()) disabled=!enabled
                aria-current=move || (target == Some(section.get())).then_some("page")
                title=move || if enabled { String::new() } else { i18n.t("coming-soon") }
                on:click=move |_| if let Some(t) = target { section.set(t) }>
                {icon}
                <span>{move || i18n.t(label)}</span>
            </button>
        }
    };

    view! {
        <div class="shell">
            <aside class="sidebar">
                <button class="ghost" on:click=move |_| ctx.screen.set(Screen::Library)>
                    {icons::back()}
                    {move || i18n.t("all-campaigns")}
                </button>
                {move || campaign.get().map(|c| {
                    let cover = c.campaign.cover.map(|h| image_url(&ctx, &h, "thumb"));
                    view! {
                        <div class="mini">
                            {match cover {
                                Some(src) => view! { <img src=src alt=""/> }.into_any(),
                                None => view! { <div class="cover-empty">{icons::image()}</div> }.into_any(),
                            }}
                            <strong>{c.campaign.title.as_str().to_owned()}</strong>
                        </div>
                    }
                })}
                <nav>
                    {nav_item("nav-home", icons::home().into_any(), Some(Section::Home))}
                    {nav_item("nav-sheet", icons::sheet().into_any(), Some(Section::Sheet))}
                    {nav_item("nav-notes", icons::notes().into_any(), Some(Section::Notes))}
                    {nav_item("nav-compendium", icons::book().into_any(), Some(Section::Compendium))}
                    {nav_item("nav-combat", icons::swords().into_any(), None)}
                    {nav_item("nav-dice", icons::dice().into_any(), None)}
                </nav>
                <div class="bottom"><ConnPill/></div>
            </aside>
            <div class="main">
                <ConnBanner/>
                {move || match section.get() {
                    Section::Home => (move || campaign.get().map(|c| view! {
                        <Home c=c tab=tab is_dm=is_dm open_note=open_note open_character=open_character/>
                    })).into_any(),
                    Section::Sheet => view! { <Sheet id=id selected=selected_character/> }.into_any(),
                    Section::Notes => view! { <Notes id=id selected=selected_note/> }.into_any(),
                    Section::Compendium => view! { <Compendium id=id/> }.into_any(),
                }}
            </div>
        </div>
    }
}

#[component]
fn Home(
    c: CampaignView,
    tab: RwSignal<Tab>,
    is_dm: Memo<bool>,
    open_note: Callback<NoteId>,
    open_character: Callback<CharacterId>,
) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let hero = c
        .campaign
        .cover
        .map(|h| {
            format!(
                "background-image: url('{}')",
                image_url(&ctx, &h, "display")
            )
        })
        .unwrap_or_default();
    let count = c.campaign.members.len();
    let dm_name = c
        .campaign
        .dm()
        .map(|m| m.display_name.as_str().to_owned())
        .unwrap_or_default();
    let meta = move || {
        if is_dm.get() {
            i18n.tr("meta-you-dm", &[("count", count.into())])
        } else {
            i18n.tr(
                "meta-dm",
                &[("name", dm_name.clone().into()), ("count", count.into())],
            )
        }
    };
    let tab_button = move |t: Tab, label: Signal<&'static str>| {
        view! {
            <button role="tab" class:active=move || tab.get() == t
                aria-selected=move || (tab.get() == t).to_string()
                on:click=move |_| tab.set(t)>
                {move || i18n.t(label.get())}
            </button>
        }
    };
    let settings_label = Signal::derive(move || {
        if is_dm.get() {
            "tab-settings"
        } else {
            "tab-rules"
        }
    });
    let c2 = c.clone();
    view! {
        <div class="hero" style=hero role="img"></div>
        <section class="home">
            <h1>{c.campaign.title.as_str().to_owned()}</h1>
            <p class="muted">{meta}</p>
            <div class="tabs" role="tablist">
                {tab_button(Tab::Characters, Signal::derive(|| "tab-characters"))}
                {tab_button(Tab::Notes, Signal::derive(|| "tab-notes"))}
                {tab_button(Tab::Party, Signal::derive(|| "tab-party"))}
                {tab_button(Tab::Settings, settings_label)}
            </div>
            {move || match tab.get() {
                Tab::Characters => view! {
                    <CharacterList c=c2.clone() me=ctx.view.with_untracked(|v| v.user.as_ref().map(|u| u.id)) open=open_character/>
                }.into_any(),
                Tab::Notes => view! { <NotesPreview c=c2.clone() on_open=open_note/> }.into_any(),
                Tab::Party => view! { <Party c=c2.clone()/> }.into_any(),
                Tab::Settings => view! { <Settings c=c2.clone() editable=is_dm.get()/> }.into_any(),
            }}
        </section>
    }
}

#[component]
fn Party(c: CampaignView) -> impl IntoView {
    let i18n = use_i18n();
    let rows = c
        .campaign
        .members
        .iter()
        .map(|m| {
            let online = c.online.contains(&m.user_id);
            let dm = m.role == Role::Dm;
            let active = c
                .campaign
                .characters
                .iter()
                .find(|ch| ch.owner == m.user_id && ch.active)
                .map(|ch| {
                    let classes: Vec<(String, u8)> =
                        ch.classes.iter().map(|k| (k.class.name.clone(), k.level)).collect();
                    summary_line(i18n, &ch.name, &classes, ch.armor_class(), ch.hp)
                })
                .or_else(|| {
                    c.campaign
                        .party
                        .iter()
                        .find(|s| s.owner == m.user_id && s.active)
                        .map(|s| summary_line(i18n, &s.name, &s.classes, s.armor_class, s.hp))
                });
            view! {
                <div class="row">
                    <span class=if online { "dot online" } else { "dot" }></span>
                    <span class="grow">
                        {m.display_name.as_str().to_owned()}
                        {active.map(|line| view! { <span class="muted small">" · "{line}</span> })}
                    </span>
                    {dm.then(|| view! { <span class="badge">{move || i18n.t("dm-badge")}</span> })}
                    <span class="muted">
                        {move || if online { i18n.t("presence-online") } else { i18n.t("presence-offline") }}
                    </span>
                </div>
            }
        })
        .collect_view();
    view! { <div class="list">{rows}</div> }
}

#[component]
fn Settings(c: CampaignView, editable: bool) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let id = c.campaign.id.to_string();
    let settings = c.campaign.settings;
    let title = RwSignal::new(c.campaign.title.as_str().to_owned());
    let uploading = RwSignal::new(false);

    let report = move |r: Result<(), beholder_ipc::IpcError>| {
        if let Err(e) = r {
            ctx.notify(error_key(e));
        }
    };
    let save_settings = {
        let id = id.clone();
        move |s: CampaignSettings| {
            let campaign_id = id.clone();
            spawn_local(async move {
                report(
                    ipc::call(
                        "update_settings",
                        SettingsArgs {
                            campaign_id,
                            settings: s,
                        },
                    )
                    .await,
                );
            });
        }
    };
    let rename = {
        let id = id.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let args = RenameArgs {
                campaign_id: id.clone(),
                title: title.get_untracked(),
            };
            spawn_local(async move { report(ipc::call("rename_campaign", args).await) });
        }
    };
    let regenerate = {
        let id = id.clone();
        move |_| {
            let campaign_id = id.clone();
            spawn_local(async move {
                report(ipc::call("regenerate_invite", IdArgs { campaign_id }).await);
            });
        }
    };
    let pick_cover = {
        let id = id.clone();
        move |ev: leptos::ev::Event| {
            let Some(file) = ev
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                .and_then(|i| i.files())
                .and_then(|f| f.get(0))
            else {
                return;
            };
            let id = id.clone();
            uploading.set(true);
            spawn_local(async move {
                report(ipc::upload_cover(&id, file).await);
                uploading.set(false);
            });
        }
    };

    let visibility = [
        (CombatVisibility::Exact, "combat-exact"),
        (CombatVisibility::Vague, "combat-vague"),
        (CombatVisibility::Hidden, "combat-hidden"),
    ];

    if !editable {
        let current = visibility
            .iter()
            .find(|(v, _)| *v == settings.combat_visibility)
            .map_or("combat-exact", |(_, k)| *k);
        let dm_reads = settings.dm_can_read_private_notes;
        return view! {
            <div class="settings">
                <p class="muted">{move || i18n.t("rules-readonly")}</p>
                <div class="row">
                    <span class="grow">{move || i18n.t("rule-dm-notes")}</span>
                    <strong>{move || if dm_reads { i18n.t("yes") } else { i18n.t("no") }}</strong>
                </div>                <div class="row">
                    <span class="grow">{move || i18n.t("rule-combat")}</span>
                    <strong>{move || i18n.t(current)}</strong>
                </div>
                <SourcesSetting campaign_id=c.campaign.id enabled=c.campaign.sources.clone() editable=false/>
            </div>
        }
        .into_any();
    }

    let code = c
        .campaign
        .invite_code
        .map(|c| c.to_string())
        .unwrap_or_default();
    let save_dm_notes = save_settings.clone();
    let radios = visibility
        .into_iter()
        .map(|(v, key)| {
            let save = save_settings.clone();
            view! {
                <label class="choice">
                    <input type="radio" name="combat" checked=settings.combat_visibility == v
                        on:change=move |_| save(CampaignSettings { combat_visibility: v, ..settings })/>
                    {move || i18n.t(key)}
                </label>
            }
        })
        .collect_view();

    view! {
        <div class="settings">
            <form class="field-row" on:submit=rename>
                <label class="grow">
                    {move || i18n.t("title-label")}
                    <input type="text" required maxlength="80" bind:value=title/>
                </label>
                <button type="submit">{move || i18n.t("save")}</button>
            </form>
            <fieldset>
                <legend>{move || i18n.t("cover")}</legend>
                <label class="choice">
                    {icons::image()}
                    <span>{move || if uploading.get() { i18n.t("cover-uploading") } else { i18n.t("cover-change") }}</span>
                    <input type="file" accept="image/jpeg,image/png,image/webp" on:change=pick_cover
                        disabled=uploading/>
                </label>
            </fieldset>
            <fieldset>
                <legend>{move || i18n.t("invite-label")}</legend>
                <span class="code">{code}</span>
                <p class="muted">{move || i18n.t("invite-hint")}</p>
                <div><button type="button" on:click=regenerate>{icons::refresh()}{move || i18n.t("invite-regenerate")}</button></div>
            </fieldset>
            <fieldset>
                <legend>{move || i18n.t("rule-dm-notes")}</legend>
                <label class="choice">
                    <input type="checkbox" checked=settings.dm_can_read_private_notes
                        on:change=move |ev| save_dm_notes(CampaignSettings {
                            dm_can_read_private_notes: event_target_checked(&ev),
                            ..settings
                        })/>
                    {move || i18n.t("rule-dm-notes")}
                </label>
            </fieldset>            <fieldset>
                <legend>{move || i18n.t("rule-combat")}</legend>
                {radios}
            </fieldset>
            <SourcesSetting campaign_id=c.campaign.id enabled=c.campaign.sources.clone() editable=true/>
        </div>
    }
    .into_any()
}
