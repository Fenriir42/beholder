use std::time::Duration;

use beholder_core::compendium::{
    ChallengeRating, Entry, FacetOptions, Facets, Hit, Kind, LIBRARY, RulesVersion, SearchQuery,
    SourceMeta,
};
use beholder_core::library::ImportSummary;
use beholder_core::{CampaignId, LibraryId, Role};
use beholder_ipc::{CompendiumStatus, ConnState, Credits, EntryView};
use beholder_protocol::http::SourceInfo;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::app::{error_key, use_app};
use crate::i18n::{I18n, use_i18n};
use crate::screens::entry_editor::EntryEditor;
use crate::{icons, ipc, markdown};

const TYPE_DELAY: Duration = Duration::from_millis(120);

const CR_STEPS: [u16; 34] = [
    0, 1, 2, 4, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 136, 144, 152,
    160, 168, 176, 184, 192, 200, 208, 216, 224, 232, 240,
];

#[derive(serde::Serialize)]
struct SearchArgs {
    query: SearchQuery,
}

#[derive(serde::Serialize)]
struct EntryArgs {
    source: String,
    key: String,
    sources: Vec<String>,
}

#[derive(serde::Serialize)]
struct LibraryIdArgs {
    id: LibraryId,
}

#[derive(serde::Serialize)]
struct HideArgs {
    campaign_id: String,
    id: LibraryId,
    hidden: bool,
}

#[derive(serde::Serialize)]
struct ExportArgs {
    yaml: bool,
}

#[derive(serde::Serialize)]
struct SourcesArgs {
    campaign_id: String,
    sources: Vec<String>,
}

pub fn rules_label(i18n: I18n, r: RulesVersion) -> String {
    i18n.t(match r {
        RulesVersion::R2014 => "rules-2014",
        RulesVersion::R2024 => "rules-2024",
        RulesVersion::LevelUp => "rules-level-up",
    })
}

pub fn kind_label(i18n: I18n, k: Kind) -> String {
    i18n.t(&format!("kind-{}", k.as_str()))
}

fn meta(i18n: I18n, f: &Facets) -> String {
    match f {
        Facets::Spell {
            level,
            school,
            classes,
            ..
        } => {
            let head = if *level == 0 {
                i18n.tr("cmp-cantrip", &[("school", school.clone().into())])
            } else {
                i18n.tr(
                    "cmp-spell-level",
                    &[
                        ("level", (*level).into()),
                        ("school", school.clone().into()),
                    ],
                )
            };
            if classes.is_empty() {
                head
            } else {
                format!("{head} · {}", classes.join(", "))
            }
        }
        Facets::Monster {
            cr,
            creature_type,
            size,
        } => i18n.tr(
            "cmp-monster",
            &[
                ("cr", cr.to_string().into()),
                ("size", size.clone().into()),
                ("type", creature_type.clone().into()),
            ],
        ),
        Facets::MagicItem {
            rarity,
            category,
            attunement,
        } => {
            let mut s = format!("{rarity} · {category}");
            if *attunement {
                s.push_str(" · ");
                s.push_str(&i18n.t("cmp-attunement"));
            }
            s
        }
        Facets::Equipment { category } | Facets::Feat { category } => {
            format!("{} · {category}", kind_label(i18n, f.kind()))
        }
        Facets::Subclass { class } => i18n.tr("cmp-subclass", &[("class", class.clone().into())]),
        Facets::Feature { class, level } => i18n.tr(
            "cmp-feature",
            &[("class", class.clone().into()), ("level", (*level).into())],
        ),
        Facets::Class
        | Facets::Species
        | Facets::Background
        | Facets::Trait
        | Facets::Condition => kind_label(i18n, f.kind()),
    }
}

fn source_label(i18n: I18n, s: &SourceMeta) -> String {
    format!("{} · {}", s.name, rules_label(i18n, s.rules))
}

#[component]
fn Choice(
    label: Signal<String>,
    value: Signal<String>,
    options: Signal<Vec<(String, String)>>,
    on_pick: Callback<Option<String>>,
) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <label class="filter">
            <span>{label}</span>
            <select prop:value=value on:change=move |ev| {
                let v = event_target_value(&ev);
                on_pick.run((!v.is_empty()).then_some(v));
            }>
                <option value="">{move || i18n.t("cmp-any")}</option>
                {move || options.get().into_iter().map(|(v, l)| view! { <option value=v>{l}</option> }).collect_view()}
            </select>
        </label>
    }
}

#[component]
pub fn Compendium(id: CampaignId) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let enabled = Memo::new(move |_| {
        ctx.view.with(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == id)
                .map(|c| c.campaign.sources.clone())
                .unwrap_or_default()
        })
    });
    let me = Memo::new(move |_| ctx.view.with(|v| v.user.as_ref().map(|u| u.id)));
    let roles = Memo::new(move |_| {
        ctx.view.with(|v| {
            let me = v.user.as_ref().map(|u| u.id);
            let role_in = |c: &beholder_ipc::CampaignView| me.and_then(|m| c.campaign.role_of(m));
            let here = v
                .campaigns
                .iter()
                .find(|c| c.campaign.id == id)
                .and_then(role_in);
            let anywhere = v.campaigns.iter().any(|c| role_in(c) == Some(Role::Dm));
            (here == Some(Role::Dm), anywhere)
        })
    });
    let hidden = Memo::new(move |_| {
        ctx.view.with(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == id)
                .map(|c| {
                    c.campaign
                        .hidden
                        .iter()
                        .map(|h| h.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    let editing = RwSignal::new(None::<Option<Entry>>);
    let message = RwSignal::new(None::<String>);
    let invalid = RwSignal::new(Vec::<String>::new());
    let confirm_delete = RwSignal::new(false);
    let query = RwSignal::new(SearchQuery::default());
    let hits = RwSignal::new(Vec::<Hit>::new());
    let credits_data = RwSignal::new(Credits::default());
    let sources = Memo::new(move |_| {
        let on = enabled.get();
        credits_data.with(|c| {
            c.sources
                .iter()
                .filter(|s| on.contains(&s.id))
                .cloned()
                .collect::<Vec<SourceMeta>>()
        })
    });
    let facets = RwSignal::new(FacetOptions::default());
    let selected = RwSignal::new(None::<(String, String)>);
    let detail = RwSignal::new(None::<EntryView>);
    let credits = RwSignal::new(false);
    let generation = StoredValue::new(0u64);

    Effect::new(move |_| {
        ctx.compendium.track();
        spawn_local(async move {
            if let Ok(c) = ipc::call0::<Credits>("compendium_credits").await {
                credits_data.try_set(c);
            }
            if let Ok(f) = ipc::call0::<FacetOptions>("compendium_facets").await {
                facets.try_set(f);
            }
        });
    });
    Effect::new(move |_| {
        let q = SearchQuery {
            sources: enabled.get(),
            hidden: if roles.get().0 {
                Vec::new()
            } else {
                hidden.get()
            },
            ..query.get()
        };
        ctx.compendium.track();
        let g = generation.get_value() + 1;
        generation.set_value(g);
        set_timeout(
            move || {
                if generation.try_get_value() != Some(g) {
                    return;
                }
                spawn_local(async move {
                    if let Ok(h) =
                        ipc::call::<Vec<Hit>>("compendium_search", SearchArgs { query: q }).await
                        && generation.try_get_value() == Some(g)
                    {
                        hits.try_set(h);
                    }
                });
            },
            TYPE_DELAY,
        );
    });

    Effect::new(move |_| {
        let Some((source, key)) = selected.get() else {
            detail.set(None);
            return;
        };
        ctx.compendium.track();
        let sources = enabled.get();
        spawn_local(async move {
            if let Ok(d) = ipc::call::<Option<EntryView>>(
                "compendium_entry",
                EntryArgs {
                    source,
                    key,
                    sources,
                },
            )
            .await
            {
                detail.try_set(d);
            }
        });
    });

    let kind_value = Signal::derive(move || {
        query.with(|q| q.kind.map(|k| k.as_str().to_owned()).unwrap_or_default())
    });
    let pick_kind = move |ev: leptos::ev::Event| {
        let kind = Kind::parse(&event_target_value(&ev));
        query.update(|q| {
            *q = SearchQuery {
                text: std::mem::take(&mut q.text),
                kind,
                ..SearchQuery::default()
            };
        });
    };
    let strings = move |list: fn(&FacetOptions) -> &Vec<String>| {
        Signal::derive(move || {
            facets.with(|f| list(f).iter().map(|s| (s.clone(), s.clone())).collect())
        })
    };
    let crs = Signal::derive(|| {
        CR_STEPS
            .iter()
            .map(|e| (e.to_string(), ChallengeRating::from_eighths(*e).to_string()))
            .collect::<Vec<_>>()
    });
    let levels = Signal::derive(move || {
        (0..=9u8)
            .map(|l| {
                let label = if l == 0 {
                    i18n.t("cmp-cantrips")
                } else {
                    l.to_string()
                };
                (l.to_string(), label)
            })
            .collect::<Vec<_>>()
    });
    let text_label = move |key: &'static str| Signal::derive(move || i18n.t(key));

    let filters = move || {
        let kind = query.with(|q| q.kind);
        let class = view! {
            <Choice label=text_label("cmp-class")
                value=Signal::derive(move || query.with(|q| q.class.clone().unwrap_or_default()))
                options=strings(|f| &f.classes)
                on_pick=Callback::new(move |v| query.update(|q| q.class = v))/>
        };
        match kind {
            Some(Kind::Spell) => view! {
                <Choice label=text_label("cmp-level")
                    value=Signal::derive(move || query.with(|q| q.level.map(|l| l.to_string()).unwrap_or_default()))
                    options=levels
                    on_pick=Callback::new(move |v: Option<String>| query.update(|q| q.level = v.and_then(|s| s.parse().ok())))/>
                <Choice label=text_label("cmp-school")
                    value=Signal::derive(move || query.with(|q| q.school.clone().unwrap_or_default()))
                    options=strings(|f| &f.schools)
                    on_pick=Callback::new(move |v| query.update(|q| q.school = v))/>
                {class}
            }
            .into_any(),
            Some(Kind::Monster) => {
                let cr = |pick: fn(&mut SearchQuery, Option<ChallengeRating>)| {
                    Callback::new(move |v: Option<String>| {
                        query.update(|q| pick(q, v.and_then(|s| s.parse().ok()).map(ChallengeRating::from_eighths)));
                    })
                };
                let shown = |get: fn(&SearchQuery) -> Option<ChallengeRating>| {
                    Signal::derive(move || query.with(|q| get(q).map(|c| c.eighths().to_string()).unwrap_or_default()))
                };
                view! {
                    <Choice label=text_label("cmp-cr-min") value=shown(|q| q.cr_min) options=crs on_pick=cr(|q, v| q.cr_min = v)/>
                    <Choice label=text_label("cmp-cr-max") value=shown(|q| q.cr_max) options=crs on_pick=cr(|q, v| q.cr_max = v)/>
                }
                .into_any()
            }
            Some(Kind::MagicItem) => view! {
                <Choice label=text_label("cmp-rarity")
                    value=Signal::derive(move || query.with(|q| q.rarity.clone().unwrap_or_default()))
                    options=strings(|f| &f.rarities)
                    on_pick=Callback::new(move |v| query.update(|q| q.rarity = v))/>
            }
            .into_any(),
            Some(Kind::Feature | Kind::Subclass) => class.into_any(),
            _ => ().into_any(),
        }
    };
    let status = move || {
        if enabled.with(Vec::is_empty) {
            return Some(view! { <div class="empty">{icons::book()}<p>{move || i18n.t("cmp-no-sources")}</p></div> }.into_any());
        }
        if !sources.with(Vec::is_empty) {
            return None;
        }
        let (key, icon) = match (ctx.compendium.get(), ctx.conn.get()) {
            (Some(CompendiumStatus::Failed), _) => ("cmp-failed", icons::alert().into_any()),
            (_, ConnState::Offline { .. } | ConnState::Reconnecting { .. }) => {
                ("cmp-offline", icons::alert().into_any())
            }
            _ => ("cmp-syncing", icons::refresh().into_any()),
        };
        Some(view! { <div class="empty">{icon}<p>{move || i18n.t(key)}</p></div> }.into_any())
    };

    let results = move || {
        let list = hits.get();
        if list.is_empty() {
            return view! { <p class="muted small">{move || i18n.t("cmp-no-results")}</p> }
                .into_any();
        }
        let full = list.len() >= 300;
        let rows = list
            .into_iter()
            .map(|h| {
                let id = (h.source.clone(), h.key.clone());                let active = id.clone();
                let line = meta(i18n, &h.facets);
                let others = h.others;
                let is_hidden = h.source == LIBRARY && hidden.with(|l| l.contains(&h.key));
                view! {
                    <button class="note-row" class:active=move || selected.with(|s| s.as_ref() == Some(&active))                        on:click=move |_| { credits.set(false); editing.set(None); selected.set(Some(id.clone())); }>
                        <span class="note-row-title">{h.name}</span>
                        <span class="note-row-meta muted">
                            <span class="grow">{line}</span>                            {is_hidden.then(|| view! {
                                <span class="chip dm">{move || i18n.t("lib-hidden")}</span>
                            })}
                            {(others > 0).then(|| view! {
                                <span class="chip shared">{move || i18n.tr("cmp-others", &[("count", others.into())])}</span>
                            })}
                        </span>
                    </button>
                }
            })
            .collect_view();
        view! {
            {rows}
            {full.then(|| view! { <p class="muted small">{move || i18n.t("cmp-refine")}</p> })}
        }
        .into_any()
    };
    let export = move |yaml: bool| {
        spawn_local(async move {
            match ipc::call::<String>("library_export", ExportArgs { yaml }).await {
                Ok(path) => message.set(Some(i18n.tr("lib-exported", &[("path", path.into())]))),
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    };
    let import = move |ev: leptos::ev::Event| {
        let Some(input) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(file) = input.files().and_then(|f| f.get(0)) else {
            return;
        };
        input.set_value("");
        message.set(Some(i18n.t("lib-reading")));
        spawn_local(async move {
            match ipc::import_library::<ImportSummary>(file).await {
                Ok(s) => {
                    message.set(Some(i18n.tr(
                        "lib-imported",
                        &[
                            ("added", s.added.into()),
                            ("skipped", s.skipped.into()),
                            ("invalid", s.invalid.len().into()),
                        ],
                    )));
                    invalid.set(s.invalid);
                }
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    };

    let image_base = ctx.boot.with_untracked(|b| b.image_base.clone());
    let done_editing = Callback::new(move |saved: Option<String>| {
        editing.set(None);
        if let Some(key) = saved {
            selected.set(Some((LIBRARY.to_owned(), key)));
        }
    });
    let pane = move || {
        if let Some(existing) = editing.get() {
            return view! { <EntryEditor existing=existing on_done=done_editing/> }.into_any();
        }
        if credits.get() {
            let licenses = credits_data
                .get()
                .licenses
                .into_iter()
                .map(|l| {
                    view! {
                        <details class="license">
                            <summary>{l.name}</summary>
                            <div class="prose small" inner_html=markdown::render(&l.text, "")></div>
                        </details>
                    }
                })
                .collect_view();
            let list = credits_data
                .get()
                .sources
                .into_iter()
                .map(|s| {
                    let label = source_label(i18n, &s);
                    view! {
                        <section class="credit">
                            <h3>{label}</h3>
                            <p>{s.attribution}</p>
                            <p class="muted small">{move || i18n.tr("cmp-upstream", &[("upstream", s.upstream.clone().into())])}</p>
                        </section>
                    }
                })
                .collect_view();
            return view! {
                <article class="editor">                    <h2 class="note-title-view">{move || i18n.t("cmp-credits")}</h2>
                    <p class="muted">{move || i18n.t("cmp-credits-intro")}</p>
                    {list}
                    <section class="credit">
                        <h3>{move || i18n.t("cmp-licenses")}</h3>
                        {licenses}
                    </section>
                </article>
            }
            .into_any();
        }
        match detail.get() {
            Some(d) => {
                let kind = kind_label(i18n, d.entry.facets.kind());
                let line = meta(i18n, &d.entry.facets);
                let source = source_label(i18n, &d.source);
                let html = markdown::render(&d.entry.text, &image_base);
                let library = (d.source.id == LIBRARY).then(|| {
                    let entry = d.entry.clone();
                    let lib_id = entry.key.parse().ok().map(LibraryId);
                    let (dm_here, dm_anywhere) = roles.get();
                    let author = entry.library.map(|m| m.author);
                    let can_edit = dm_anywhere || (author.is_some() && author == me.get());
                    let is_hidden = hidden.with(|l| l.contains(&entry.key));
                    let book = entry.book.clone();
                    let rules = entry.rules.map(|r| rules_label(i18n, r));
                    let hide = move |_| {
                        let Some(lib_id) = lib_id else { return };
                        let args = HideArgs {
                            campaign_id: id.to_string(),
                            id: lib_id,
                            hidden: !is_hidden,
                        };
                        spawn_local(async move {
                            if let Err(e) = ipc::call::<()>("library_hide", args).await {
                                ctx.notify(error_key(e));
                            }
                        });
                    };
                    view! {
                        <p class="muted small">
                            {book.map(|b| i18n.tr("lib-from-book", &[("book", b.into())]))}
                            {rules.map(|r| format!(" · {r}"))}
                        </p>
                        {is_hidden.then(|| view! {
                            <div class="notice">{icons::lock()}<span>{move || i18n.t("lib-hidden-hint")}</span></div>
                        })}
                        <div class="toolbar">
                            {can_edit.then(|| view! {
                                <button type="button" on:click=move |_| editing.set(Some(Some(entry.clone())))>
                                    {icons::pencil()}{move || i18n.t("lib-edit")}
                                </button>
                            })}
                            {dm_here.then(|| view! {
                                <button type="button" on:click=hide>
                                    {icons::eye()}
                                    {move || if is_hidden { i18n.t("lib-unhide") } else { i18n.t("lib-hide") }}
                                </button>
                            })}
                            <span class="grow"></span>
                            {can_edit.then(|| view! {
                                <button type="button" class="ghost danger" on:click=move |_| confirm_delete.set(true)
                                    aria-label=move || i18n.t("lib-delete") title=move || i18n.t("lib-delete")>
                                    {icons::trash()}
                                </button>
                            })}
                        </div>
                        {move || confirm_delete.get().then(|| view! {
                            <div class="dialog" role="dialog" aria-modal="true">
                                <div class="panel">
                                    <h2>{move || i18n.t("lib-delete")}</h2>
                                    <p>{move || i18n.t("lib-delete-confirm")}</p>
                                    <div class="actions">
                                        <button type="button" on:click=move |_| confirm_delete.set(false)>{move || i18n.t("cancel")}</button>
                                        <button type="button" class="danger-solid" on:click=move |_| {
                                            confirm_delete.set(false);
                                            let Some(lib_id) = lib_id else { return };
                                            selected.set(None);
                                            spawn_local(async move {
                                                if let Err(e) = ipc::call::<()>("library_delete", LibraryIdArgs { id: lib_id }).await {
                                                    ctx.notify(error_key(e));
                                                }
                                            });
                                        }>
                                            {icons::trash()}{move || i18n.t("lib-delete")}
                                        </button>
                                    </div>
                                </div>
                            </div>
                        })}
                    }
                });
                let also = (!d.also.is_empty()).then(|| {
                    let links = d
                        .also
                        .into_iter()
                        .map(|a| {
                            let target = (a.source, a.key);
                            view! {
                                <button class="link" on:click=move |_| selected.set(Some(target.clone()))>
                                    {a.source_name}
                                </button>
                            }
                        })
                        .collect_view();
                    view! {
                        <aside class="notice">
                            {icons::copy()}
                            <span>{move || i18n.t("cmp-also-in")}</span>
                            <span class="also">{links}</span>
                        </aside>
                    }
                });
                view! {
                    <article class="editor">
                        <header class="editor-head">
                            <h2 class="note-title-view">{d.entry.name}</h2>
                            <div class="editor-meta">
                                <span class="chip private">{kind}</span>
                                <span class="muted">{line}</span>                            </div>
                            <p class="muted small">{source}</p>
                        </header>
                        {library}
                        {also}
                        <div class="prose" inner_html=html></div>
                    </article>
                }
                .into_any()
            }
            None => view! {
                <div class="empty">{icons::book()}<p>{move || i18n.t("cmp-none-selected")}</p></div>
            }
            .into_any(),
        }
    };

    view! {
        <div class="notes">
            <aside class="notes-list">
                <div class="notes-head">
                    <h2>{move || i18n.t("nav-compendium")}</h2>                    <button class="ghost" class:active=move || credits.get()
                        on:click=move |_| credits.update(|c| *c = !*c)>
                        {move || i18n.t("cmp-credits")}
                    </button>
                </div>
                <div class="toolbar">
                    <button type="button" class="primary" on:click=move |_| { credits.set(false); editing.set(Some(None)); }>
                        {icons::plus()}{move || i18n.t("lib-new")}
                    </button>
                    <label class="btn file">
                        <span>{move || i18n.t("lib-import")}</span>
                        <input type="file" accept=".json,.yaml,.yml,application/json" on:change=import/>
                    </label>
                    <button type="button" on:click=move |_| export(false)>{move || i18n.t("lib-export-json")}</button>
                    <button type="button" on:click=move |_| export(true)>{move || i18n.t("lib-export-yaml")}</button>
                </div>                {move || message.get().map(|m| view! { <p class="notice" role="status">{m}</p> })}
                {move || {
                    let n = ctx.view.with(|v| v.pending_import);
                    (n > 0).then(|| view! {
                        <p class="notice" role="status">
                            {icons::refresh()}{move || i18n.tr("lib-importing", &[("count", n.into())])}
                        </p>
                    })
                }}
                {move || (!invalid.with(Vec::is_empty)).then(|| view! {
                    <details class="license">
                        <summary>{move || i18n.t("lib-invalid-title")}</summary>
                        <ul class="small">{invalid.get().into_iter().map(|line| view! { <li>{line}</li> }).collect_view()}</ul>
                    </details>
                })}
                <input type="search" placeholder=move || i18n.t("cmp-search") aria-label=move || i18n.t("cmp-search")
                    prop:value=move || query.with(|q| q.text.clone())
                    on:input=move |ev| query.update(|q| q.text = event_target_value(&ev))/>
                <div class="filters">
                    <label class="filter">
                        <span>{move || i18n.t("cmp-kind")}</span>
                        <select prop:value=kind_value on:change=pick_kind>
                            <option value="">{move || i18n.t("cmp-all-kinds")}</option>
                            {Kind::ALL.into_iter().map(|k| view! {
                                <option value=k.as_str()>{move || i18n.t(&format!("kinds-{}", k.as_str()))}</option>
                            }).collect_view()}
                        </select>                    </label>
                    {filters}
                    <Choice label=text_label("cmp-source-filter")
                        value=Signal::derive(move || query.with(|q| q.source.clone().unwrap_or_default()))
                        options=Signal::derive(move || sources.get().into_iter().map(|s| (s.id, s.name)).collect())
                        on_pick=Callback::new(move |v| query.update(|q| q.source = v))/>
                </div>
                {status}
                <div class="list">{results}</div>
            </aside>
            <section class="note-pane">{pane}</section>
        </div>
    }
}

#[component]
pub fn SourcesSetting(
    campaign_id: CampaignId,
    enabled: Vec<String>,
    editable: bool,
) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let failed = RwSignal::new(false);
    if ctx.available_sources.with_untracked(Option::is_none) {
        spawn_local(async move {
            match ipc::call0::<Vec<SourceInfo>>("compendium_available").await {
                Ok(list) => ctx.available_sources.set(Some(list)),
                Err(_) => failed.set(true),
            }
        });
    }
    let enabled = StoredValue::new(enabled);
    let toggle = move |id: String, on: bool| {
        let mut next = enabled.get_value();
        next.retain(|s| *s != id);
        if on {
            next.push(id);
        }
        let args = SourcesArgs {
            campaign_id: campaign_id.to_string(),
            sources: next,
        };
        spawn_local(async move {
            if let Err(e) = ipc::call::<()>("set_sources", args).await {
                ctx.notify(error_key(e));
            }
        });
    };
    let body = move || match ctx.available_sources.get() {
        Some(list) => {
            let mut publishers: Vec<String> = Vec::new();
            for s in &list {
                if !publishers.contains(&s.publisher) {
                    publishers.push(s.publisher.clone());
                }
            }
            publishers
                .into_iter()
                .map(|p| {
                    let rows = list
                        .iter()
                        .filter(|s| s.publisher == p)
                        .filter(|s| editable || enabled.with_value(|e| e.contains(&s.id)))
                        .map(|s| {
                            let on = enabled.with_value(|e| e.contains(&s.id));
                            let id = s.id.clone();
                            let size = i18n.tr("sources-size", &[("kb", s.bytes.div_ceil(1024).into())]);
                            let rules = rules_label(i18n, s.rules);
                            view! {
                                <label class="choice">
                                    <input type="checkbox" checked=on disabled=!editable
                                        on:change=move |ev| toggle(id.clone(), event_target_checked(&ev))/>
                                    <span class="grow">{s.name.clone()}</span>
                                    <span class="muted small">{format!("{rules} · {size}")}</span>
                                </label>
                            }
                        })
                        .collect::<Vec<_>>();
                    (!rows.is_empty()).then(|| view! {
                        <div class="source-group">
                            <h4>{p}</h4>
                            {rows}
                        </div>
                    })
                })
                .collect_view()
                .into_any()
        }
        None => {
            let names = enabled.get_value().join(", ");
            view! {
                <p class="muted">{move || if failed.get() { i18n.t("sources-offline") } else { i18n.t("sources-loading") }}</p>
                <p>{names}</p>
            }
            .into_any()
        }
    };
    view! {
        <fieldset>
            <legend>{move || i18n.t("sources-label")}</legend>
            <p class="muted small">{move || if editable { i18n.t("sources-hint") } else { i18n.t("sources-readonly") }}</p>
            {body}
        </fieldset>
    }
}
