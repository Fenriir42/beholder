use std::time::Duration;

use beholder_core::{Audience, Campaign, CampaignId, Member, Note, NoteId, Role, UserId};
use beholder_ipc::{CampaignView, IpcError};
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::app::{error_key, use_app};
use crate::i18n::use_i18n;
use crate::{icons, ipc, markdown};

const SAVE_AFTER: Duration = Duration::from_millis(1000);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chip {
    Party,
    Shared,
    Private,
    DmOnly,
}

impl Chip {
    pub fn of(note: &Note, campaign: &Campaign) -> Self {
        match note.audience {
            Audience::Party => Self::Party,
            Audience::Users(_) => Self::Shared,
            Audience::Private if campaign.role_of(note.author) == Some(Role::Dm) => Self::DmOnly,
            Audience::Private => Self::Private,
        }
    }

    fn class(self) -> &'static str {
        match self {
            Self::Party => "chip party",
            Self::Shared => "chip shared",
            Self::Private => "chip private",
            Self::DmOnly => "chip dm",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Party => "chip-party",
            Self::Shared => "chip-shared",
            Self::Private => "chip-private",
            Self::DmOnly => "chip-dm",
        }
    }
}

#[component]
fn AudienceChip(chip: Chip) -> impl IntoView {
    let i18n = use_i18n();
    view! { <span class=chip.class()>{move || i18n.t(chip.key())}</span> }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Party,
    Shared,
    Private,
}

impl Filter {
    fn matches(self, chip: Chip) -> bool {
        match self {
            Self::All => true,
            Self::Party => chip == Chip::Party,
            Self::Shared => chip == Chip::Shared,
            Self::Private => matches!(chip, Chip::Private | Chip::DmOnly),
        }
    }
}

#[derive(serde::Serialize)]
struct CreateArgs {
    campaign_id: String,
    title: String,
    audience: Audience,
}

#[derive(serde::Serialize)]
struct EditArgs {
    campaign_id: String,
    note_id: String,
    base_version: u32,
    title: String,
    body: String,
}

#[derive(serde::Serialize)]
struct AudienceArgs {
    campaign_id: String,
    note_id: String,
    audience: Audience,
}

#[derive(serde::Serialize)]
struct NoteArgs {
    campaign_id: String,
    note_id: String,
}

fn author_name(members: &[Member], id: UserId) -> String {
    members
        .iter()
        .find(|m| m.user_id == id)
        .map(|m| m.display_name.as_str().to_owned())
        .unwrap_or_default()
}

#[derive(Clone, PartialEq)]
struct Row {
    id: NoteId,
    title: String,
    chip: Chip,
    author: String,
    pending: bool,
    conflict: bool,
}

#[component]
pub fn Notes(id: CampaignId, selected: RwSignal<Option<NoteId>>) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let query = RwSignal::new(String::new());
    let filter = RwSignal::new(Filter::All);
    let fresh = RwSignal::new(None::<NoteId>);

    let rows = Memo::new(move |_| {
        let q = query.get().to_lowercase();
        let f = filter.get();
        ctx.view.with(|v| {
            let Some(c) = v.campaigns.iter().find(|c| c.campaign.id == id) else {
                return Vec::new();
            };
            c.campaign
                .notes
                .iter()
                .rev()
                .filter(|n| f.matches(Chip::of(n, &c.campaign)))
                .filter(|n| {
                    q.is_empty()
                        || n.title.as_str().to_lowercase().contains(&q)
                        || n.body.as_str().to_lowercase().contains(&q)
                })
                .map(|n| Row {
                    id: n.id,
                    title: n.title.as_str().to_owned(),
                    chip: Chip::of(n, &c.campaign),
                    author: author_name(&c.campaign.members, n.author),
                    pending: c.pending_notes.contains(&n.id),
                    conflict: n.conflict_of.is_some(),
                })
                .collect::<Vec<_>>()
        })
    });

    let new_note = move |_| {
        let args = CreateArgs {
            campaign_id: id.to_string(),
            title: i18n.t("note-untitled"),
            audience: Audience::Private,
        };
        spawn_local(async move {
            match ipc::call::<NoteId>("create_note", args).await {
                Ok(nid) => {
                    fresh.set(Some(nid));
                    selected.set(Some(nid));
                }
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    };

    let filter_button = move |f: Filter, key: &'static str| {
        view! {
            <button type="button" class:active=move || filter.get() == f
                aria-pressed=move || (filter.get() == f).to_string()
                on:click=move |_| filter.set(f)>
                {move || i18n.t(key)}
            </button>
        }
    };

    view! {
        <div class="notes">
            <aside class="notes-list">
                <div class="notes-head">
                    <h2>{move || i18n.t("notes-title")}</h2>
                    <button class="primary" on:click=new_note>
                        {icons::plus()}
                        {move || i18n.t("note-new")}
                    </button>
                </div>
                <input type="search" bind:value=query placeholder=move || i18n.t("notes-search")
                    aria-label=move || i18n.t("notes-search")/>
                <div class="segmented">
                    {filter_button(Filter::All, "notes-filter-all")}
                    {filter_button(Filter::Party, "chip-party")}
                    {filter_button(Filter::Shared, "chip-shared")}
                    {filter_button(Filter::Private, "chip-private")}
                </div>
                {move || {
                    let rows = rows.get();
                    if rows.is_empty() {
                        return view! {
                            <div class="empty small">{icons::notes()}<p>{move || i18n.t("notes-list-empty")}</p></div>
                        }.into_any();
                    }
                    rows.into_iter().map(|r| {
                        let rid = r.id;
                        let by = i18n.tr("note-by", &[("name", r.author.into())]);
                        view! {
                            <button class="note-row" class:active=move || selected.get() == Some(rid)
                                on:click=move |_| selected.set(Some(rid))>
                                <span class="note-row-title">{r.title}</span>
                                <span class="note-row-meta">
                                    <AudienceChip chip=r.chip/>
                                    {r.conflict.then(|| view! {
                                        <span class="chip private">{icons::copy()}{move || i18n.t("chip-conflict")}</span>
                                    })}
                                    <span class="muted grow">{by}</span>
                                    {r.pending.then(|| view! {
                                        <span class="saving"><span class="dot warning"></span>{move || i18n.t("note-saving")}</span>
                                    })}
                                </span>
                            </button>
                        }
                    }).collect_view().into_any()
                }}
            </aside>
            <section class="note-pane">
                {move || match selected.get() {
                    Some(nid) => view! {
                        <NoteEditor campaign_id=id note_id=nid selected=selected
                            start_editing=fresh.get_untracked() == Some(nid)/>
                    }.into_any(),
                    None => view! {
                        <div class="empty">{icons::notes()}<p>{move || i18n.t("notes-none-selected")}</p></div>
                    }.into_any(),
                }}
            </section>
        </div>
    }
}

#[derive(Clone, PartialEq)]
struct NoteState {
    note: Note,
    chip: Chip,
    pending: bool,
    my_copy: Option<NoteId>,
    original: Option<NoteId>,
    members: Vec<Member>,
}

fn utf16_to_byte(s: &str, utf16: u32) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units >= utf16 as usize {
            return i;
        }
        units += c.len_utf16();
    }
    s.len()
}

#[component]
fn NoteEditor(
    campaign_id: CampaignId,
    note_id: NoteId,
    selected: RwSignal<Option<NoteId>>,
    start_editing: bool,
) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let me = ctx.view.with_untracked(|v| v.user.as_ref().map(|u| u.id));
    let state = Memo::new(move |_| {
        ctx.view.with(|v| {
            let c = v.campaigns.iter().find(|c| c.campaign.id == campaign_id)?;
            let note = c.campaign.note(note_id)?.clone();
            Some(NoteState {
                chip: Chip::of(&note, &c.campaign),
                pending: c.pending_notes.contains(&note_id),
                my_copy: c
                    .campaign
                    .notes
                    .iter()
                    .rev()
                    .find(|n| n.conflict_of == Some(note_id) && Some(n.author) == me)
                    .map(|n| n.id),
                original: note.conflict_of.filter(|o| c.campaign.note(*o).is_some()),
                members: c.campaign.members.clone(),
                note,
            })
        })
    });

    let title = RwSignal::new(String::new());
    let body = RwSignal::new(String::new());
    let base = RwSignal::new(0u32);
    let dirty = RwSignal::new(false);
    let loaded = RwSignal::new(false);
    let editing = RwSignal::new(start_editing);
    let show_audience = RwSignal::new(false);
    let confirm_delete = RwSignal::new(false);
    let uploading = RwSignal::new(false);
    let generation = StoredValue::new(0u64);
    let textarea = NodeRef::<leptos::html::Textarea>::new();

    // Own edits bump `base` before the server confirms them, so an older view (version < base) never overwrites local text.
    Effect::new(move |_| {
        let Some(s) = state.get() else { return };
        if !loaded.get_untracked() {
            loaded.set(true);
            if s.note.body.as_str().is_empty() {
                editing.set(true);
            }
        } else if dirty.get_untracked() || s.pending || s.note.version < base.get_untracked() {
            return;
        }
        if title.get_untracked() != s.note.title.as_str() {
            title.set(s.note.title.as_str().to_owned());
        }
        if body.get_untracked() != s.note.body.as_str() {
            body.set(s.note.body.as_str().to_owned());
        }
        base.set(s.note.version);
    });

    let save = move || {
        let (Some(true), Some(t), Some(b), Some(v)) = (
            dirty.try_get_untracked(),
            title.try_get_untracked(),
            body.try_get_untracked(),
            base.try_get_untracked(),
        ) else {
            return;
        };
        if t.trim().is_empty() {
            return;
        }
        let args = EditArgs {
            campaign_id: campaign_id.to_string(),
            note_id: note_id.to_string(),
            base_version: v,
            title: t,
            body: b,
        };
        dirty.set(false);
        base.set(v + 1);
        spawn_local(async move {
            if let Err(e) = ipc::call::<()>("edit_note", args).await {
                ctx.notify(error_key(e));
                base.try_set(v);
                dirty.try_set(true);
            }
        });
    };
    let changed = move || {
        dirty.set(true);
        let g = generation.get_value() + 1;
        generation.set_value(g);
        set_timeout(
            move || {
                if generation.try_get_value() == Some(g) {
                    save();
                }
            },
            SAVE_AFTER,
        );
    };
    on_cleanup(save);

    let report = move |r: Result<(), IpcError>| {
        if let Err(e) = r {
            ctx.notify(error_key(e));
        }
    };
    let set_audience = move |audience: Audience| {
        let args = AudienceArgs {
            campaign_id: campaign_id.to_string(),
            note_id: note_id.to_string(),
            audience,
        };
        spawn_local(async move { report(ipc::call("set_note_audience", args).await) });
    };
    let delete = move |_| {
        let args = NoteArgs {
            campaign_id: campaign_id.to_string(),
            note_id: note_id.to_string(),
        };
        dirty.set(false);
        selected.set(None);
        spawn_local(async move { report(ipc::call("delete_note", args).await) });
    };
    let pick_image = move |ev: leptos::ev::Event| {
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
        let cursor = textarea
            .get_untracked()
            .and_then(|t| t.selection_start().ok().flatten());
        uploading.set(true);
        spawn_local(async move {
            match ipc::upload_image(file).await {
                Ok(hex) => {
                    body.update(|b| {
                        let at = cursor.map_or(b.len(), |c| utf16_to_byte(b, c));
                        b.insert_str(at, &format!("\n![](img:{hex})\n"));
                    });
                    changed();
                }
                Err(e) => ctx.notify(error_key(e)),
            }
            uploading.set(false);
        });
    };

    let image_base = ctx.boot.with_untracked(|b| b.image_base.clone());
    let info = Memo::new(move |_| {
        state.with(|s| {
            s.as_ref().map(|s| {
                let is_author = Some(s.note.author) == me;
                let my_role = me
                    .and_then(|u| s.members.iter().find(|m| m.user_id == u))
                    .map(|m| m.role);
                Info {
                    chip: s.chip,
                    is_author,
                    can_edit: is_author || me.is_some_and(|u| s.note.audience.includes(u)),
                    is_dm: my_role == Some(Role::Dm),
                    author: author_name(&s.members, s.note.author),
                    audience: s.note.audience.clone(),
                    original: s.original,
                    my_copy: s.my_copy,
                    others: s
                        .members
                        .iter()
                        .filter(|m| m.user_id != s.note.author)
                        .cloned()
                        .collect(),
                }
            })
        })
    });
    let exists = Memo::new(move |_| info.with(Option::is_some));
    let can_edit = Memo::new(move |_| info.with(|i| i.as_ref().is_some_and(|i| i.can_edit)));
    let pending = Memo::new(move |_| state.with(|s| s.as_ref().is_some_and(|s| s.pending)));
    let version = Memo::new(move |_| state.with(|s| s.as_ref().map_or(0, |s| s.note.version)));
    let writing = Memo::new(move |_| editing.get() && can_edit.get());

    let notices = move || {
        info.get().map(|i| {
            view! {
                {i.original.map(|o| view! {
                    <div class="notice">
                        {icons::copy()}
                        <span class="grow">{move || i18n.t("note-conflict-copy")}</span>
                        <button class="link" on:click=move |_| selected.set(Some(o))>{move || i18n.t("note-open-original")}</button>
                    </div>
                })}
                {i.my_copy.map(|o| view! {
                    <div class="notice">
                        {icons::alert()}
                        <span class="grow">{move || i18n.t("note-has-copy")}</span>
                        <button class="link" on:click=move |_| selected.set(Some(o))>{move || i18n.t("note-open-copy")}</button>
                    </div>
                })}
                {(!i.can_edit).then(|| {
                    let key = if i.is_dm && !i.is_author && i.chip == Chip::Private {
                        "note-dm-reading"
                    } else {
                        "note-readonly-hint"
                    };
                    view! { <div class="notice">{icons::lock()}<span>{move || i18n.t(key)}</span></div> }
                })}
            }
        })
    };
    let remote_notice = move || {
        (dirty.get() && version.get() > base.get()).then(|| view! {
            <div class="notice warning">{icons::alert()}<span>{move || i18n.t("note-changed-remotely")}</span></div>
        })
    };
    let title_view = move || {
        if writing.get() {
            view! {
                <input type="text" class="note-title" maxlength="120" required
                    aria-label=move || i18n.t("note-title-label")
                    prop:value=move || title.get()
                    on:input=move |ev| { title.set(event_target_value(&ev)); changed(); }
                    on:blur=move |_| save()/>
            }
            .into_any()
        } else {
            view! { <h2 class="note-title-view">{move || title.get()}</h2> }.into_any()
        }
    };
    let meta = move || {
        info.get().map(|i| {
            let by = i18n.tr("note-by", &[("name", i.author.into())]);
            view! {
                <AudienceChip chip=i.chip/>
                <span class="muted">{by}</span>
                <span class="saving grow">
                    {move || if !can_edit.get() {
                        view! { {icons::lock()}<span>{i18n.t("note-readonly")}</span> }.into_any()
                    } else if dirty.get() || pending.get() {
                        view! { <span class="dot warning"></span><span>{i18n.t("note-saving")}</span> }.into_any()
                    } else {
                        view! { <span class="dot online"></span><span>{i18n.t("note-saved")}</span> }.into_any()
                    }}
                </span>
            }
        })
    };
    let toolbar = move || {
        info.get().map(|i| {
            view! {
                {i.can_edit.then(|| view! {
                    <div class="segmented">
                        <button type="button" class:active=move || editing.get()
                            aria-pressed=move || editing.get().to_string()
                            on:click=move |_| editing.set(true)>
                            {icons::pencil()}{move || i18n.t("note-edit")}
                        </button>
                        <button type="button" class:active=move || !editing.get()
                            aria-pressed=move || (!editing.get()).to_string()
                            on:click=move |_| { save(); editing.set(false); }>
                            {icons::eye()}{move || i18n.t("note-preview")}
                        </button>
                    </div>
                })}
                {move || writing.get().then(|| view! {
                    <label class="btn file">
                        {icons::image()}
                        <span>{move || if uploading.get() { i18n.t("note-uploading") } else { i18n.t("note-insert-image") }}</span>
                        <input type="file" accept="image/jpeg,image/png,image/webp"
                            on:change=pick_image disabled=uploading/>
                    </label>
                })}
                <span class="grow"></span>
                {(i.is_author && i.audience != Audience::Party).then(|| view! {
                    <button type="button" on:click=move |_| set_audience(Audience::Party)>
                        {icons::eye()}{move || i18n.t("note-reveal")}
                    </button>
                })}
                {i.is_author.then(|| view! {
                    <button type="button" class:active=move || show_audience.get()
                        aria-expanded=move || show_audience.get().to_string()
                        on:click=move |_| show_audience.update(|v| *v = !*v)>
                        {icons::users()}{move || i18n.t("audience-label")}
                    </button>
                    <button type="button" class="ghost danger" on:click=move |_| confirm_delete.set(true)
                        aria-label=move || i18n.t("note-delete") title=move || i18n.t("note-delete")>
                        {icons::trash()}
                    </button>
                })}
            }
        })
    };
    let audience_panel = move || {
        let i = info.get().filter(|i| i.is_author && show_audience.get())?;
        let picked: Vec<UserId> = match &i.audience {
            Audience::Users(u) => u.clone(),
            _ => Vec::new(),
        };
        let radio = move |a: Audience, key: &'static str, checked: bool| {
            view! {
                <label class="choice">
                    <input type="radio" name="audience" checked=checked
                        on:change=move |_| set_audience(a.clone())/>
                    {move || i18n.t(key)}
                </label>
            }
        };
        let private_key = if i.is_dm {
            "audience-dm-only"
        } else {
            "audience-private"
        };
        let checks = i
            .others
            .iter()
            .map(|m| {
                let uid = m.user_id;
                let on = picked.contains(&uid);
                let picked = picked.clone();
                view! {
                    <label class="choice">
                        <input type="checkbox" checked=on on:change=move |ev| {
                            let mut next: Vec<UserId> = picked.iter().copied().filter(|u| *u != uid).collect();
                            if event_target_checked(&ev) {
                                next.push(uid);
                            }
                            set_audience(if next.is_empty() { Audience::Private } else { Audience::Users(next) });
                        }/>
                        {m.display_name.as_str().to_owned()}
                    </label>
                }
            })
            .collect_view();
        Some(view! {
            <fieldset class="audience">
                <legend>{move || i18n.t("audience-label")}</legend>
                {radio(Audience::Private, private_key, i.audience == Audience::Private)}
                {radio(Audience::Party, "audience-party", i.audience == Audience::Party)}
                <p class="muted">{move || i18n.t("audience-users")}</p>
                <div class="audience-users">{checks}</div>
                <p class="muted small">{move || i18n.t("audience-users-hint")}</p>
            </fieldset>
        })
    };
    let content = move || {
        if writing.get() {
            view! {
                <textarea class="note-body" node_ref=textarea
                    aria-label=move || i18n.t("note-body-label")
                    placeholder=move || i18n.t("note-body-placeholder")
                    prop:value=move || body.get()
                    on:input=move |ev| { body.set(event_target_value(&ev)); changed(); }
                    on:blur=move |_| save()></textarea>
            }
            .into_any()
        } else {
            let image_base = image_base.clone();
            view! {
                {move || body.with(|b| b.trim().is_empty()).then(|| view! {
                    <p class="muted">{move || i18n.t("note-empty-body")}</p>
                })}
                <article class="prose" inner_html=move || body.with(|b| markdown::render(b, &image_base))></article>
            }
            .into_any()
        }
    };
    let dialog = move || {
        confirm_delete.get().then(|| view! {
            <div class="dialog" role="dialog" aria-modal="true">
                <div class="panel">
                    <h2>{move || i18n.t("note-delete")}</h2>
                    <p>{move || i18n.tr("note-delete-confirm", &[("title", title.get().into())])}</p>
                    <div class="actions">
                        <button type="button" on:click=move |_| confirm_delete.set(false)>{move || i18n.t("cancel")}</button>
                        <button type="button" class="danger-solid" on:click=delete>
                            {icons::trash()}{move || i18n.t("note-delete")}
                        </button>
                    </div>
                </div>
            </div>
        })
    };

    move || {
        if !exists.get() {
            return view! {
                <div class="empty">{icons::notes()}<p>{move || i18n.t("note-gone")}</p></div>
            }
            .into_any();
        }
        view! {
            <div class="editor">
                {notices}
                {remote_notice}
                <header class="editor-head">
                    {title_view}
                    <div class="editor-meta">{meta}</div>
                    <div class="toolbar">{toolbar}</div>
                </header>
                {audience_panel}
                {content.clone()}
                {dialog}
            </div>
        }
        .into_any()
    }
}

#[derive(Clone, PartialEq)]
struct Info {
    chip: Chip,
    is_author: bool,
    can_edit: bool,
    is_dm: bool,
    author: String,
    audience: Audience,
    original: Option<NoteId>,
    my_copy: Option<NoteId>,
    others: Vec<Member>,
}

#[component]
pub fn NotesPreview(c: CampaignView, on_open: Callback<NoteId>) -> impl IntoView {
    let i18n = use_i18n();
    let cards: Vec<_> = c
        .campaign
        .notes
        .iter()
        .rev()
        .filter(|n| n.audience == Audience::Party)
        .map(|n| {
            let id = n.id;
            let excerpt = markdown::excerpt(n.body.as_str(), 160);
            let by = i18n.tr(
                "note-by",
                &[("name", author_name(&c.campaign.members, n.author).into())],
            );
            view! {
                <button class="note-card" on:click=move |_| on_open.run(id)>
                    <span class="note-row-title">{n.title.as_str().to_owned()}</span>
                    <span class="muted note-excerpt">{excerpt}</span>
                    <span class="note-row-meta">
                        <AudienceChip chip=Chip::Party/>
                        <span class="muted">{by}</span>
                    </span>
                </button>
            }
        })
        .collect();
    if cards.is_empty() {
        return view! {
            <div class="empty">{icons::notes()}<p>{move || i18n.t("notes-empty")}</p></div>
        }
        .into_any();
    }
    view! { <div class="note-cards">{cards}</div> }.into_any()
}
