use beholder_core::compendium::{Entry, Kind, RulesVersion};
use beholder_core::library::ImportSummary;
use beholder_core::paste::Pasted;
use beholder_ipc::PasteItem;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::{error_key, use_app};
use crate::i18n::use_i18n;
use crate::screens::compendium::{kind_label, meta, rules_label};
use crate::screens::entry_editor::{EntryEditor, RULES};
use crate::{ipc, markdown};

#[derive(serde::Serialize)]
struct PasteArgs {
    text: String,
    book: Option<String>,
    rules: RulesVersion,
    kind: Kind,
}

#[derive(serde::Serialize)]
struct AcceptArgs {
    entries: Vec<Pasted>,
    book: Option<String>,
    rules: RulesVersion,
    replace: bool,
}

#[derive(Clone)]
struct Row {
    item: PasteItem,
    selected: bool,
    open: bool,
}

#[component]
pub fn PasteImport(on_done: Callback<Option<String>>) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let text = RwSignal::new(String::new());
    let book = RwSignal::new(String::new());
    let rules = RwSignal::new(RulesVersion::R2024);
    let kind = RwSignal::new(Kind::Trait);
    let rows = RwSignal::new(None::<Vec<Row>>);
    let editing = RwSignal::new(None::<usize>);
    let replace = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let image_base = ctx.boot.with_untracked(|b| b.image_base.clone());
    let book_value =
        move || book.with_untracked(|b| (!b.trim().is_empty()).then(|| b.trim().to_owned()));

    let read = move |_| {
        busy.set(true);
        let args = PasteArgs {
            text: text.get_untracked(),
            book: book_value(),
            rules: rules.get_untracked(),
            kind: kind.get_untracked(),
        };
        spawn_local(async move {
            match ipc::call::<Vec<PasteItem>>("library_paste", args).await {
                Ok(items) => rows.set(Some(
                    items
                        .into_iter()
                        .map(|item| Row {
                            selected: !item.duplicate,
                            item,
                            open: false,
                        })
                        .collect(),
                )),
                Err(e) => ctx.notify(error_key(e)),
            }
            busy.try_set(false);
        });
    };

    let accept = move |all: bool| {
        let entries: Vec<Pasted> = rows.with_untracked(|r| {
            r.iter()
                .flatten()
                .filter(|row| all || row.selected)
                .map(|row| row.item.entry.clone())
                .collect()
        });
        if entries.is_empty() {
            return;
        }
        busy.set(true);
        let args = AcceptArgs {
            entries,
            book: book_value(),
            rules: rules.get_untracked(),
            replace: replace.get_untracked(),
        };
        spawn_local(async move {
            match ipc::call::<ImportSummary>("library_accept", args).await {
                Ok(s) => on_done.run(Some(i18n.tr(
                    "lib-imported",
                    &[
                        ("added", s.added.into()),
                        ("replaced", s.replaced.into()),
                        ("skipped", s.skipped.into()),
                        ("notyours", s.not_yours.into()),
                        ("invalid", s.invalid.len().into()),
                    ],
                ))),
                Err(e) => {
                    busy.try_set(false);
                    ctx.notify(error_key(e));
                }
            }
        });
    };

    let edited = Callback::new(move |saved: Option<String>| {
        if let (Some(_), Some(i)) = (saved, editing.get_untracked()) {
            rows.update(|r| {
                if let Some(r) = r.as_mut().filter(|r| i < r.len()) {
                    r.remove(i);
                }
            });
        }
        editing.set(None);
    });

    let form = move || {
        view! {
            <header class="editor-head">
                <h2 class="note-title-view">{move || i18n.t("paste-title")}</h2>
                <p class="muted small">{move || i18n.t("paste-hint")}</p>
            </header>
            <div class="filters">
                <label class="filter">
                    <span>{move || i18n.t("lib-book")}</span>
                    <input type="text" maxlength="120" prop:value=move || book.get()
                        placeholder=move || i18n.t("lib-book-hint")
                        on:input=move |ev| book.set(event_target_value(&ev))/>
                </label>
                <label class="filter">
                    <span>{move || i18n.t("lib-rules")}</span>
                    <select prop:value=move || format!("{:?}", rules.get())
                        on:change=move |ev| {
                            let v = event_target_value(&ev);
                            if let Some(r) = RULES.into_iter().find(|r| format!("{r:?}") == v) {
                                rules.set(r);
                            }
                        }>
                        {RULES.into_iter().map(|r| view! {
                            <option value=format!("{r:?}")>{move || rules_label(i18n, r)}</option>
                        }).collect_view()}
                    </select>
                </label>
                <label class="filter">
                    <span>{move || i18n.t("paste-default-kind")}</span>
                    <select prop:value=move || kind.get().as_str()
                        on:change=move |ev| {
                            if let Some(k) = Kind::parse(&event_target_value(&ev)) {
                                kind.set(k);
                            }
                        }>
                        {Kind::ALL.into_iter().map(|k| view! {
                            <option value=k.as_str()>{move || kind_label(i18n, k)}</option>
                        }).collect_view()}
                    </select>
                </label>
            </div>
            <label class="filter">
                <span>{move || i18n.t("paste-text")}</span>
                <textarea class="note-body" prop:value=move || text.get()
                    on:input=move |ev| text.set(event_target_value(&ev))></textarea>
            </label>
            <div class="toolbar">
                <span class="grow"></span>
                <button type="button" on:click=move |_| on_done.run(None)>{move || i18n.t("cancel")}</button>
                <button type="button" class="primary"
                    disabled=move || busy.get() || text.with(|t| t.trim().is_empty())
                    on:click=read>
                    {move || i18n.t("paste-read")}
                </button>
            </div>
        }
    };

    let row_view = move |i: usize, row: Row| {
        let entry = row.item.entry.clone();
        let flags = view! {
            {(!entry.complete).then(|| view! { <span class="badge">{move || i18n.t("paste-check")}</span> })}
            {row.item.duplicate.then(|| view! { <span class="badge">{move || i18n.t("paste-duplicate")}</span> })}
        };
        let preview = row
            .open
            .then(|| markdown::render(&entry.text, &image_base))
            .map(|html| view! { <div class="prose small" inner_html=html></div> });
        let toggle = move |f: fn(&mut Row)| {
            rows.update(|r| {
                if let Some(row) = r.as_mut().and_then(|r| r.get_mut(i)) {
                    f(row);
                }
            });
        };
        view! {
            <div class="note-row">
                <label class="choice">
                    <input type="checkbox" prop:checked=row.selected
                        on:change=move |_| toggle(|r| r.selected = !r.selected)/>
                    <span class="note-row-title">{entry.name.clone()}</span>
                </label>
                <span class="note-row-meta">
                    <span class="muted">{meta(i18n, &entry.facets)}</span>
                    {flags}
                </span>
                <div class="toolbar">
                    <button type="button" class="ghost" aria-expanded=row.open.to_string()
                        on:click=move |_| toggle(|r| r.open = !r.open)>
                        {move || i18n.t("paste-preview")}
                    </button>
                    <button type="button" class="ghost" on:click=move |_| editing.set(Some(i))>
                        {move || i18n.t("lib-edit")}
                    </button>
                </div>
                {preview}
            </div>
        }
    };

    let review = move |list: Vec<Row>| {
        let selected = list.iter().filter(|r| r.selected).count();
        let total = list.len();
        view! {
            <header class="editor-head">
                <h2 class="note-title-view">{move || i18n.t("paste-title")}</h2>
                <p class="muted small" role="status">
                    {move || i18n.tr("paste-found", &[("count", total.into())])}
                </p>
            </header>
            <div class="list">
                {list.into_iter().enumerate().map(|(i, row)| row_view(i, row)).collect_view()}
            </div>
            <div class="toolbar">
                <button type="button" on:click=move |_| rows.set(None)>{move || i18n.t("paste-back")}</button>
                <label class="choice small">
                    <input type="checkbox" prop:checked=move || replace.get()
                        on:change=move |ev| replace.set(event_target_checked(&ev))/>
                    {move || i18n.t("lib-replace")}
                </label>
                <span class="grow"></span>
                <button type="button" disabled=move || busy.get() || total == 0
                    on:click=move |_| accept(true)>
                    {move || i18n.t("paste-accept-all")}
                </button>
                <button type="button" class="primary" disabled=move || busy.get() || selected == 0
                    on:click=move |_| accept(false)>
                    {move || i18n.tr("paste-accept-selected", &[("count", selected.into())])}
                </button>
            </div>
        }
    };

    move || {
        if let Some(i) = editing.get() {
            let entry = rows.with_untracked(|r| {
                r.as_ref().and_then(|r| r.get(i)).map(|row| Entry {
                    key: String::new(),
                    name: row.item.entry.name.clone(),
                    facets: row.item.entry.facets.clone(),
                    text: row.item.entry.text.clone(),
                    book: book_value(),
                    rules: Some(rules.get_untracked()),
                    library: None,
                })
            });
            return view! { <EntryEditor existing=entry on_done=edited/> }.into_any();
        }
        let body = match rows.get() {
            Some(list) => review(list).into_any(),
            None => form().into_any(),
        };
        view! { <article class="editor">{body}</article> }.into_any()
    }
}
