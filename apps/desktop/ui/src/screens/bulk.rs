use beholder_core::compendium::{Kind, RulesVersion};
use beholder_core::library::ImportSummary;
use beholder_core::mapping::{Field, Profile};
use beholder_ipc::{BulkOpened, BulkScanView, PasteItem};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::{error_key, use_app};
use crate::i18n::use_i18n;
use crate::ipc;
use crate::screens::compendium::{kind_label, meta, rules_label};
use crate::screens::entry_editor::RULES;

#[derive(serde::Serialize)]
struct ProfileArgs {
    profile: Profile,
}

#[derive(serde::Serialize)]
struct ImportArgs {
    profile: Profile,
    replace: bool,
}

#[derive(serde::Serialize)]
struct NameArgs {
    name: String,
}

#[derive(serde::Serialize)]
struct KeyArgs {
    key: String,
}

fn field_label(f: Field) -> &'static str {
    match f {
        Field::Level => "cmp-level",
        Field::School => "cmp-school",
        Field::Classes => "lib-classes",
        Field::Ritual => "lib-ritual",
        Field::Concentration => "lib-concentration",
        Field::Cr => "lib-cr",
        Field::CreatureType => "lib-creature-type",
        Field::Size => "lib-size",
        Field::Rarity => "cmp-rarity",
        Field::Category => "lib-category",
        Field::Attunement => "lib-attunement",
        Field::Class => "cmp-class",
    }
}

#[component]
pub fn BulkImport(on_done: Callback<Option<ImportSummary>>) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let opened = RwSignal::new(None::<BulkOpened>);
    let profile = RwSignal::new(None::<Profile>);
    let saved = RwSignal::new(Vec::<Profile>::new());
    let values = RwSignal::new(Vec::<String>::new());
    let preview = RwSignal::new(Vec::<PasteItem>::new());
    let scan = RwSignal::new(None::<BulkScanView>);
    let replace = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let notice = RwSignal::new(None::<String>);

    spawn_local(async move {
        if let Ok(list) = ipc::call0::<Vec<Profile>>("mapping_profiles").await {
            saved.set(list);
        }
    });

    let refresh = move || {
        let Some(p) = profile.get_untracked() else {
            return;
        };
        scan.set(None);
        let key = p.kind_key.clone();
        spawn_local(async move {
            match key {
                Some(key) => {
                    if let Ok(v) = ipc::call::<Vec<String>>("bulk_values", KeyArgs { key }).await {
                        values.set(v);
                    }
                }
                None => values.set(Vec::new()),
            }
            match ipc::call::<Vec<PasteItem>>("bulk_preview", ProfileArgs { profile: p }).await {
                Ok(items) => preview.set(items),
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    };

    let pick = move |_| {
        busy.set(true);
        spawn_local(async move {
            match ipc::call0::<Option<BulkOpened>>("bulk_open").await {
                Ok(Some(o)) => {
                    profile.set(Some(o.profile.clone()));
                    opened.set(Some(o));
                    refresh();
                }
                Ok(None) => {}
                Err(e) => ctx.notify(error_key(e)),
            }
            busy.try_set(false);
        });
    };

    let edit = move |f: Box<dyn FnOnce(&mut Profile)>| {
        profile.update(|p| {
            if let Some(p) = p.as_mut() {
                f(p);
            }
        });
        scan.set(None);
    };

    let check = move |_| {
        let Some(p) = profile.get_untracked() else {
            return;
        };
        busy.set(true);
        spawn_local(async move {
            match ipc::call::<BulkScanView>("bulk_scan", ProfileArgs { profile: p }).await {
                Ok(s) => scan.set(Some(s)),
                Err(e) => ctx.notify(error_key(e)),
            }
            busy.try_set(false);
        });
    };

    let import = move |_| {
        let Some(p) = profile.get_untracked() else {
            return;
        };
        busy.set(true);
        let args = ImportArgs {
            profile: p,
            replace: replace.get_untracked(),
        };
        spawn_local(async move {
            match ipc::call::<ImportSummary>("bulk_import", args).await {
                Ok(s) => on_done.run(Some(s)),
                Err(e) => {
                    busy.try_set(false);
                    ctx.notify(error_key(e));
                }
            }
        });
    };

    let save_profile = move |_| {
        let Some(p) = profile.get_untracked() else {
            return;
        };
        spawn_local(async move {
            match ipc::call::<Vec<Profile>>("mapping_profile_save", ProfileArgs { profile: p })
                .await
            {
                Ok(list) => saved.set(list),
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    };
    let export_profile = move |_| {
        let name =
            profile.with_untracked(|p| p.as_ref().map(|p| p.name.clone()).unwrap_or_default());
        spawn_local(async move {
            match ipc::call::<String>("mapping_profile_export", NameArgs { name }).await {
                Ok(path) => notice.set(Some(
                    i18n.tr("bulk-profile-exported", &[("path", path.into())]),
                )),
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    };
    let delete_profile = move |_| {
        let name =
            profile.with_untracked(|p| p.as_ref().map(|p| p.name.clone()).unwrap_or_default());
        spawn_local(async move {
            match ipc::call::<Vec<Profile>>("mapping_profile_delete", NameArgs { name }).await {
                Ok(list) => saved.set(list),
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    };
    let import_profile = move |_| {
        spawn_local(async move {
            match ipc::call0::<Vec<Profile>>("mapping_profile_import").await {
                Ok(list) => saved.set(list),
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    };
    let use_saved = move |ev: leptos::ev::Event| {
        let name = event_target_value(&ev);
        if let Some(p) = saved.with_untracked(|s| s.iter().find(|p| p.name == name).cloned()) {
            profile.set(Some(p));
            refresh();
        }
    };

    let keys = move || opened.with(|o| o.as_ref().map(|o| o.keys.clone()).unwrap_or_default());
    let key_options = move |current: Option<String>| {
        let mut all = keys();
        if let Some(c) = current.filter(|c| !all.contains(c)) {
            all.push(c);
        }
        view! {
            <option value="">{move || i18n.t("bulk-none")}</option>
            {all.into_iter().map(|k| view! { <option value=k.clone()>{k.clone()}</option> }).collect_view()}
        }
    };
    let key_select = move |label: &'static str,
                           get: fn(&Profile) -> Option<String>,
                           set: fn(&mut Profile, Option<String>)| {
        view! {
            <label class="filter">
                <span>{move || i18n.t(label)}</span>
                <select
                    prop:value=move || profile.with(|p| p.as_ref().and_then(get).unwrap_or_default())
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        edit(Box::new(move |p| set(p, (!v.is_empty()).then_some(v))));
                        refresh();
                    }>
                    {move || key_options(profile.with(|p| p.as_ref().and_then(get)))}
                </select>
            </label>
        }
    };
    let kind_select = move |value: Kind, on_pick: Callback<Kind>| {
        view! {
            <select prop:value=value.as_str()
                on:change=move |ev| {
                    if let Some(k) = Kind::parse(&event_target_value(&ev)) {
                        on_pick.run(k);
                    }
                }>
                {Kind::ALL.into_iter().map(|k| view! {
                    <option value=k.as_str()>{move || kind_label(i18n, k)}</option>
                }).collect_view()}
            </select>
        }
    };

    let mapping = move || {
        let Some(p) = profile.get() else {
            return ().into_any();
        };
        let kinds_table = values
            .get()
            .into_iter()
            .map(|value| {
                let current = p.kinds.get(&value).copied().unwrap_or(p.default_kind);
                let key = value.clone();
                let pick = Callback::new(move |k: Kind| {
                    let key = key.clone();
                    edit(Box::new(move |p| {
                        p.kinds.insert(key, k);
                    }));
                    refresh();
                });
                view! {
                    <label class="filter">
                        <span>{value.clone()}</span>
                        {kind_select(current, pick)}
                    </label>
                }
            })
            .collect_view();
        let fields = Field::ALL
            .into_iter()
            .map(|f| {
                let current = p.fields.get(&f).cloned();
                let selected = current.clone().unwrap_or_default();
                view! {
                    <label class="filter">
                        <span>{move || i18n.t(field_label(f))}</span>
                        <select prop:value=selected
                            on:change=move |ev| {
                                let v = event_target_value(&ev);
                                edit(Box::new(move |p| {
                                    if v.is_empty() {
                                        p.fields.remove(&f);
                                    } else {
                                        p.fields.insert(f, v);
                                    }
                                }));
                                refresh();
                            }>
                            {key_options(current)}
                        </select>
                    </label>
                }
            })
            .collect_view();
        let default_pick = Callback::new(move |k: Kind| {
            edit(Box::new(move |p| p.default_kind = k));
            refresh();
        });
        view! {
            <h3>{move || i18n.t("bulk-profile")}</h3>
            <div class="filters">
                <label class="filter">
                    <span>{move || i18n.t("bulk-profile-saved")}</span>
                    <select prop:value=p.name.clone() on:change=use_saved>
                        <option value="">{move || i18n.t("bulk-profile-suggested")}</option>
                        {saved.get().into_iter().map(|s| view! {
                            <option value=s.name.clone()>{s.name.clone()}</option>
                        }).collect_view()}
                    </select>
                </label>
                <label class="filter">
                    <span>{move || i18n.t("bulk-profile-name")}</span>
                    <input type="text" maxlength="80" prop:value=p.name.clone()
                        on:change=move |ev| {
                            let v = event_target_value(&ev);
                            edit(Box::new(move |p| p.name = v));
                        }/>
                </label>
            </div>
            <div class="toolbar">
                <button type="button" disabled=p.name.trim().is_empty() on:click=save_profile>
                    {move || i18n.t("bulk-profile-save")}
                </button>
                <button type="button" class="ghost" on:click=export_profile
                    disabled=!saved.with(|s| s.iter().any(|x| x.name == p.name))>
                    {move || i18n.t("bulk-profile-export")}
                </button>
                <button type="button" class="ghost" on:click=delete_profile
                    disabled=!saved.with(|s| s.iter().any(|x| x.name == p.name))>
                    {move || i18n.t("bulk-profile-delete")}
                </button>
                <button type="button" class="ghost" on:click=import_profile>
                    {move || i18n.t("bulk-profile-import")}
                </button>
            </div>
            <div class="filters">
                {key_select("bulk-name-key", |p| Some(p.name_key.clone()), |p, v| p.name_key = v.unwrap_or_default())}
                <label class="filter">
                    <span>{move || i18n.t("bulk-text-keys")}</span>
                    <input type="text" list="bulk-keys" prop:value=p.text_keys.join(", ")
                        on:change=move |ev| {
                            let v = event_target_value(&ev);
                            edit(Box::new(move |p| {
                                p.text_keys = v.split(',').map(|k| k.trim().to_owned()).filter(|k| !k.is_empty()).collect();
                            }));
                            refresh();
                        }/>
                    <datalist id="bulk-keys">
                        {keys().into_iter().map(|k| view! { <option value=k/> }).collect_view()}
                    </datalist>
                </label>
                {key_select("bulk-kind-key", |p| p.kind_key.clone(), |p, v| p.kind_key = v)}
                <label class="filter">
                    <span>{move || i18n.t("bulk-default-kind")}</span>
                    {kind_select(p.default_kind, default_pick)}
                </label>
                {key_select("bulk-book-key", |p| p.book_key.clone(), |p, v| p.book_key = v)}
                <label class="filter">
                    <span>{move || i18n.t("bulk-book-fixed")}</span>
                    <input type="text" maxlength="120" prop:value=p.book.clone().unwrap_or_default()
                        placeholder=move || i18n.t("lib-book-hint")
                        on:change=move |ev| {
                            let v = event_target_value(&ev);
                            edit(Box::new(move |p| p.book = (!v.trim().is_empty()).then_some(v)));
                            refresh();
                        }/>
                </label>
                <label class="filter">
                    <span>{move || i18n.t("lib-rules")}</span>
                    <select prop:value=format!("{:?}", p.rules)
                        on:change=move |ev| {
                            let v = event_target_value(&ev);
                            if let Some(r) = RULES.into_iter().find(|r| format!("{r:?}") == v) {
                                edit(Box::new(move |p| p.rules = r));
                                refresh();
                            }
                        }>
                        {RULES.into_iter().map(|r: RulesVersion| view! {
                            <option value=format!("{r:?}")>{move || rules_label(i18n, r)}</option>
                        }).collect_view()}
                    </select>
                </label>
            </div>
            <label class="choice small">
                <input type="checkbox" prop:checked=p.rules_from_book
                    on:change=move |ev| {
                        let on = event_target_checked(&ev);
                        edit(Box::new(move |p| p.rules_from_book = on));
                        refresh();
                    }/>
                {move || i18n.t("bulk-rules-from-book")}
            </label>
            {(!values.with(Vec::is_empty)).then(|| view! {
                <h3>{move || i18n.t("bulk-kinds")}</h3>
                <div class="filters">{kinds_table}</div>
            })}
            <h3>{move || i18n.t("bulk-fields")}</h3>
            <div class="filters">{fields}</div>
        }
        .into_any()
    };

    let preview_list = move || {
        preview
            .get()
            .into_iter()
            .map(|item| {
                let e = item.entry;
                view! {
                    <div class="note-row">
                        <span class="note-row-title">{e.name.clone()}</span>
                        <span class="note-row-meta">
                            <span class="muted">{meta(i18n, &e.facets)}</span>
                            {(!e.complete).then(|| view! { <span class="badge">{move || i18n.t("paste-check")}</span> })}
                            {item.duplicate.then(|| view! { <span class="badge">{move || i18n.t("paste-duplicate")}</span> })}
                        </span>
                    </div>
                }
            })
            .collect_view()
    };

    let summary = move || {
        scan.get().map(|s| {
            let kinds = s
                .kinds
                .iter()
                .map(|(k, n)| view! { <li>{format!("{} · {n}", kind_label(i18n, *k))}</li> })
                .collect_view();
            let books = s
                .books
                .iter()
                .map(|(b, n)| {
                    let name = if b.is_empty() { i18n.t("bulk-no-book") } else { b.clone() };
                    view! { <li>{format!("{name} · {n}")}</li> }
                })
                .collect_view();
            let invalid = s.invalid.clone();
            view! {
                <p class="notice" role="status">
                    {i18n.tr("bulk-summary", &[
                        ("total", s.total.into()),
                        ("duplicates", s.duplicates.into()),
                        ("partial", s.partial.into()),
                        ("invalid", s.invalid_count.into()),
                    ])}
                </p>
                <div class="filters">
                    <div><h3>{move || i18n.t("bulk-by-kind")}</h3><ul class="small">{kinds}</ul></div>
                    <div><h3>{move || i18n.t("bulk-by-book")}</h3><ul class="small">{books}</ul></div>
                </div>
                {(!invalid.is_empty()).then(|| view! {
                    <details class="license">
                        <summary>{move || i18n.t("lib-invalid-title")}</summary>
                        <ul class="small">{invalid.into_iter().map(|l| view! { <li>{l}</li> }).collect_view()}</ul>
                    </details>
                })}
                <div class="toolbar">
                    <label class="choice small">
                        <input type="checkbox" prop:checked=move || replace.get()
                            on:change=move |ev| replace.set(event_target_checked(&ev))/>
                        {move || i18n.t("lib-replace")}
                    </label>
                    <span class="grow"></span>
                    <button type="button" class="primary" disabled=move || busy.get() on:click=import>
                        {move || i18n.tr("bulk-import", &[("count", (s.total - s.invalid_count).into())])}
                    </button>
                </div>
            }
        })
    };

    view! {
        <article class="editor">
            <header class="editor-head">
                <h2 class="note-title-view">{move || i18n.t("bulk-title")}</h2>
                <p class="muted small">{move || i18n.t("bulk-hint")}</p>
            </header>
            <div class="toolbar">
                <button type="button" class="primary" disabled=move || busy.get() on:click=pick>
                    {move || i18n.t("bulk-pick")}
                </button>
                {move || opened.get().map(|o| {
                    let at = if o.profile.entries.is_empty() {
                        i18n.t("bulk-entries-root")
                    } else {
                        i18n.tr("bulk-entries-at", &[("path", o.profile.entries.join(" › ").into())])
                    };
                    view! {
                        <span class="muted small">
                            {format!("{} · {:.1} MB · {at}", o.file_name, o.bytes as f64 / 1_000_000.0)}
                        </span>
                    }
                })}
                <span class="grow"></span>
                <button type="button" on:click=move |_| on_done.run(None)>{move || i18n.t("cancel")}</button>
            </div>
            {move || notice.get().map(|n| view! { <p class="notice" role="status">{n}</p> })}
            {move || opened.get().map(|o| view! {
                <details class="license">
                    <summary>{move || i18n.t("bulk-sample")}</summary>
                    {o.sample.into_iter().map(|s| view! { <pre class="small">{s}</pre> }).collect_view()}
                </details>
            })}
            {mapping}
            {move || opened.with(Option::is_some).then(|| view! {
                <h3>{move || i18n.t("paste-preview")}</h3>
                <div class="list">{preview_list}</div>
                <div class="toolbar">
                    <span class="grow"></span>
                    <button type="button" disabled=move || busy.get() on:click=check>
                        {move || if busy.get() { i18n.t("bulk-scanning") } else { i18n.t("bulk-scan") }}
                    </button>
                </div>
            })}
            {summary}
        </article>
    }
}
