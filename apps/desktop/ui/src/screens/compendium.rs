use std::time::Duration;

use beholder_core::compendium::{
    ChallengeRating, FacetOptions, Facets, Hit, Kind, RulesVersion, SearchQuery, SourceMeta,
};
use beholder_ipc::{CompendiumStatus, ConnState, EntryView};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::use_app;
use crate::i18n::{I18n, use_i18n};
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
    let rules = match s.rules {
        RulesVersion::R2014 => "2014",
        RulesVersion::R2024 => "2024",
    };
    i18n.tr(
        "cmp-source",
        &[("name", s.name.clone().into()), ("rules", rules.into())],
    )
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
pub fn Compendium() -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let query = RwSignal::new(SearchQuery::default());
    let hits = RwSignal::new(Vec::<Hit>::new());
    let sources = RwSignal::new(Vec::<SourceMeta>::new());
    let facets = RwSignal::new(FacetOptions::default());
    let selected = RwSignal::new(None::<(String, String)>);
    let detail = RwSignal::new(None::<EntryView>);
    let credits = RwSignal::new(false);
    let generation = StoredValue::new(0u64);

    Effect::new(move |_| {
        ctx.compendium.track();
        spawn_local(async move {
            if let Ok(s) = ipc::call0::<Vec<SourceMeta>>("compendium_sources").await {
                sources.try_set(s);
            }
            if let Ok(f) = ipc::call0::<FacetOptions>("compendium_facets").await {
                facets.try_set(f);
            }
        });
    });

    Effect::new(move |_| {
        let q = query.get();
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
        spawn_local(async move {
            if let Ok(d) =
                ipc::call::<Option<EntryView>>("compendium_entry", EntryArgs { source, key }).await
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
        Some(view! { <div class="empty">{icon}<p>{move || i18n.t(key)}</p></div> })
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
                let id = (h.source.clone(), h.key.clone());
                let active = id.clone();
                let line = meta(i18n, &h.facets);
                view! {
                    <button class="note-row" class:active=move || selected.with(|s| s.as_ref() == Some(&active))
                        on:click=move |_| { credits.set(false); selected.set(Some(id.clone())); }>
                        <span class="note-row-title">{h.name}</span>
                        <span class="note-row-meta muted">{line}</span>
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

    let image_base = ctx.boot.with_untracked(|b| b.image_base.clone());
    let pane = move || {
        if credits.get() {
            let list = sources
                .get()
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
                <article class="editor">
                    <h2 class="note-title-view">{move || i18n.t("cmp-credits")}</h2>
                    <p class="muted">{move || i18n.t("cmp-credits-intro")}</p>
                    {list}
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
                view! {
                    <article class="editor">
                        <header class="editor-head">
                            <h2 class="note-title-view">{d.entry.name}</h2>
                            <div class="editor-meta">
                                <span class="chip private">{kind}</span>
                                <span class="muted">{line}</span>
                            </div>
                            <p class="muted small">{source}</p>
                        </header>
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
                    <h2>{move || i18n.t("nav-compendium")}</h2>
                    <button class="ghost" class:active=move || credits.get()
                        on:click=move |_| credits.update(|c| *c = !*c)>
                        {move || i18n.t("cmp-credits")}
                    </button>
                </div>
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
                        </select>
                    </label>
                    {filters}
                </div>
                {status}
                <div class="list">{results}</div>
            </aside>
            <section class="note-pane">{pane}</section>
        </div>
    }
}
