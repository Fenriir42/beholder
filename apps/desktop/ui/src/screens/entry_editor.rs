use beholder_core::compendium::{ChallengeRating, Entry, Facets, Kind, RulesVersion};
use beholder_core::library::LibraryDraft;
use beholder_core::{LibraryId, NoteBody, NoteTitle, ValidationError};
use beholder_ipc::IpcError;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::{error_key, use_app};
use crate::i18n::use_i18n;
use crate::ipc;
use crate::screens::compendium::{kind_label, rules_label};

const SIZES: [&str; 6] = ["Tiny", "Small", "Medium", "Large", "Huge", "Gargantuan"];

const RULES: [RulesVersion; 3] = [
    RulesVersion::R2024,
    RulesVersion::R2014,
    RulesVersion::LevelUp,
];

#[derive(serde::Serialize)]
struct SaveArgs {
    draft: LibraryDraft,
    base_version: u32,
}

fn template_key(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Monster => Some("tmpl-monster"),
        Kind::Spell => Some("tmpl-spell"),
        _ => None,
    }
}

#[derive(Clone, Default)]
struct Fields {
    level: u8,
    school: String,
    classes: String,
    ritual: bool,
    concentration: bool,
    cr: u16,
    creature_type: String,
    size: String,
    rarity: String,
    category: String,
    attunement: bool,
    class: String,
}

impl Fields {
    fn from_facets(f: &Facets) -> Self {
        let mut out = Self {
            size: "Medium".into(),
            level: 1,
            cr: 8,
            ..Self::default()
        };
        match f {
            Facets::Spell {
                level,
                school,
                classes,
                ritual,
                concentration,
            } => {
                out.level = *level;
                out.school = school.clone();
                out.classes = classes.join(", ");
                out.ritual = *ritual;
                out.concentration = *concentration;
            }
            Facets::Monster {
                cr,
                creature_type,
                size,
            } => {
                out.cr = cr.eighths();
                out.creature_type = creature_type.clone();
                out.size = size.clone();
            }
            Facets::MagicItem {
                rarity,
                category,
                attunement,
            } => {
                out.rarity = rarity.clone();
                out.category = category.clone();
                out.attunement = *attunement;
            }
            Facets::Equipment { category } | Facets::Feat { category } => {
                out.category = category.clone();
            }
            Facets::Subclass { class } => out.class = class.clone(),
            Facets::Feature { class, level } => {
                out.class = class.clone();
                out.level = *level;
            }
            Facets::Class
            | Facets::Species
            | Facets::Background
            | Facets::Trait
            | Facets::Condition => {}
        }
        out
    }

    fn facets(&self, kind: Kind) -> Facets {
        let t = |s: &str| s.trim().to_owned();
        match kind {
            Kind::Spell => Facets::Spell {
                level: self.level.min(9),
                school: t(&self.school),
                classes: self
                    .classes
                    .split(',')
                    .map(str::trim)
                    .filter(|c| !c.is_empty())
                    .map(str::to_owned)
                    .collect(),
                ritual: self.ritual,
                concentration: self.concentration,
            },
            Kind::Monster => Facets::Monster {
                cr: ChallengeRating::from_eighths(self.cr),
                creature_type: t(&self.creature_type),
                size: t(&self.size),
            },
            Kind::MagicItem => Facets::MagicItem {
                rarity: t(&self.rarity),
                category: t(&self.category),
                attunement: self.attunement,
            },
            Kind::Equipment => Facets::Equipment {
                category: t(&self.category),
            },
            Kind::Feat => Facets::Feat {
                category: t(&self.category),
            },
            Kind::Subclass => Facets::Subclass {
                class: t(&self.class),
            },
            Kind::Feature => Facets::Feature {
                class: t(&self.class),
                level: self.level.clamp(1, 20),
            },
            Kind::Class => Facets::Class,
            Kind::Species => Facets::Species,
            Kind::Background => Facets::Background,
            Kind::Trait => Facets::Trait,
            Kind::Condition => Facets::Condition,
        }
    }
}

#[component]
pub fn EntryEditor(existing: Option<Entry>, on_done: Callback<Option<String>>) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let id = existing
        .as_ref()
        .and_then(|e| e.key.parse().ok().map(LibraryId))
        .unwrap_or_else(|| LibraryId::from_bytes([0; 16]));
    let base_version = existing
        .as_ref()
        .and_then(|e| e.library)
        .map_or(0, |m| m.version);
    let is_new = existing.is_none();
    let start_kind = existing.as_ref().map_or(Kind::Monster, |e| e.facets.kind());
    let kind = RwSignal::new(start_kind);
    let name = RwSignal::new(
        existing
            .as_ref()
            .map(|e| e.name.clone())
            .unwrap_or_default(),
    );
    let book = RwSignal::new(
        existing
            .as_ref()
            .and_then(|e| e.book.clone())
            .unwrap_or_default(),
    );
    let rules = RwSignal::new(
        existing
            .as_ref()
            .and_then(|e| e.rules)
            .unwrap_or(RulesVersion::R2024),
    );
    let text = RwSignal::new(
        existing
            .as_ref()
            .map(|e| e.text.clone())
            .unwrap_or_else(|| {
                template_key(start_kind)
                    .map(|k| i18n.t(k))
                    .unwrap_or_default()
            }),
    );
    let fields = RwSignal::new(existing.as_ref().map_or_else(
        || Fields::from_facets(&Facets::Condition),
        |e| Fields::from_facets(&e.facets),
    ));
    let error = RwSignal::new(None::<&'static str>);
    let busy = RwSignal::new(false);

    let pick_kind = move |ev: leptos::ev::Event| {
        let Some(next) = Kind::parse(&event_target_value(&ev)) else {
            return;
        };
        let previous = kind.get_untracked();
        let old_template = template_key(previous)
            .map(|k| i18n.t(k))
            .unwrap_or_default();
        if text.with_untracked(|t| t.trim().is_empty() || *t == old_template) {
            text.set(template_key(next).map(|k| i18n.t(k)).unwrap_or_default());
        }
        kind.set(next);
    };

    let save = move |_| {
        let validation = |e: ValidationError| error_key(IpcError::Invalid(e));
        let built = (|| {
            let draft = LibraryDraft {
                id,
                name: NoteTitle::new(&name.get_untracked()).map_err(validation)?,
                facets: fields.with_untracked(|f| f.facets(kind.get_untracked())),
                text: NoteBody::new(text.get_untracked()).map_err(validation)?,
                book: book
                    .with_untracked(|b| (!b.trim().is_empty()).then(|| NoteTitle::new(b)))
                    .transpose()
                    .map_err(validation)?,
                rules: rules.get_untracked(),
            };
            draft.check_facets().map_err(validation)?;
            Ok::<_, &'static str>(draft)
        })();
        let draft = match built {
            Ok(d) => d,
            Err(key) => {
                error.set(Some(key));
                return;
            }
        };
        error.set(None);
        busy.set(true);
        spawn_local(async move {
            match ipc::call::<String>(
                "library_save",
                SaveArgs {
                    draft,
                    base_version,
                },
            )
            .await
            {
                Ok(saved) => on_done.run(Some(saved)),
                Err(e) => {
                    busy.try_set(false);
                    ctx.notify(error_key(e));
                }
            }
        });
    };

    let text_input =
        move |get: fn(&Fields) -> String, set: fn(&mut Fields, String), label: &'static str| {
            view! {
                <label class="filter">
                    <span>{move || i18n.t(label)}</span>
                    <input type="text" prop:value=move || fields.with(get)
                        on:input=move |ev| fields.update(|f| set(f, event_target_value(&ev)))/>
                </label>
            }
        };
    let check = move |get: fn(&Fields) -> bool, set: fn(&mut Fields, bool), label: &'static str| {
        view! {
            <label class="choice">
                <input type="checkbox" prop:checked=move || fields.with(get)
                    on:change=move |ev| fields.update(|f| set(f, event_target_checked(&ev)))/>
                {move || i18n.t(label)}
            </label>
        }
    };
    let number = move |get: fn(&Fields) -> u8,
                       set: fn(&mut Fields, u8),
                       range: std::ops::RangeInclusive<u8>,
                       label: &'static str| {
        let options = range
            .map(|l| view! { <option value=l.to_string()>{l.to_string()}</option> })
            .collect_view();
        view! {
            <label class="filter">
                <span>{move || i18n.t(label)}</span>
                <select prop:value=move || fields.with(get).to_string()
                    on:change=move |ev| fields.update(|f| set(f, event_target_value(&ev).parse().unwrap_or_default()))>
                    {options}
                </select>
            </label>
        }
    };

    let facet_fields = move || match kind.get() {
        Kind::Spell => view! {
            {number(|f| f.level, |f, v| f.level = v, 0..=9, "cmp-level")}
            {text_input(|f| f.school.clone(), |f, v| f.school = v, "cmp-school")}
            {text_input(|f| f.classes.clone(), |f, v| f.classes = v, "lib-classes")}
            {check(|f| f.ritual, |f, v| f.ritual = v, "lib-ritual")}
            {check(|f| f.concentration, |f, v| f.concentration = v, "lib-concentration")}
        }
        .into_any(),
        Kind::Monster => {
            let crs = [0u16, 1, 2, 4]
                .into_iter()
                .chain((1..=30).map(|n| n * 8))
                .map(|e| {
                    view! { <option value=e.to_string()>{ChallengeRating::from_eighths(e).to_string()}</option> }
                })
                .collect_view();
            let sizes = SIZES
                .map(|s| view! { <option value=s>{s}</option> })
                .into_iter()
                .collect_view();
            view! {
                <label class="filter">
                    <span>{move || i18n.t("lib-cr")}</span>
                    <select prop:value=move || fields.with(|f| f.cr.to_string())
                        on:change=move |ev| fields.update(|f| f.cr = event_target_value(&ev).parse().unwrap_or_default())>
                        {crs}
                    </select>
                </label>
                {text_input(|f| f.creature_type.clone(), |f, v| f.creature_type = v, "lib-creature-type")}
                <label class="filter">
                    <span>{move || i18n.t("lib-size")}</span>
                    <select prop:value=move || fields.with(|f| f.size.clone())
                        on:change=move |ev| fields.update(|f| f.size = event_target_value(&ev))>
                        {sizes}
                    </select>
                </label>
            }
            .into_any()
        }
        Kind::MagicItem => view! {
            {text_input(|f| f.rarity.clone(), |f, v| f.rarity = v, "cmp-rarity")}
            {text_input(|f| f.category.clone(), |f, v| f.category = v, "lib-category")}
            {check(|f| f.attunement, |f, v| f.attunement = v, "lib-attunement")}
        }
        .into_any(),
        Kind::Equipment | Kind::Feat => text_input(
            |f| f.category.clone(),
            |f, v| f.category = v,
            "lib-category",
        )
        .into_any(),
        Kind::Subclass => {
            text_input(|f| f.class.clone(), |f, v| f.class = v, "cmp-class").into_any()
        }
        Kind::Feature => view! {
            {text_input(|f| f.class.clone(), |f, v| f.class = v, "cmp-class")}
            {number(|f| f.level, |f, v| f.level = v, 1..=20, "cmp-level")}
        }
        .into_any(),
        Kind::Class | Kind::Species | Kind::Background | Kind::Trait | Kind::Condition => {
            ().into_any()
        }
    };

    let pending = Memo::new(move |_| ctx.view.with(|v| v.pending_library.contains(&id)));

    view! {
        <article class="editor">
            <header class="editor-head">
                <h2 class="note-title-view">
                    {move || if is_new { i18n.t("lib-editor-new") } else { i18n.t("lib-editor-edit") }}
                </h2>
                <p class="muted small">{move || i18n.t("lib-editor-hint")}</p>
            </header>
            <div class="filters">
                <label class="filter">
                    <span>{move || i18n.t("cmp-kind")}</span>
                    <select prop:value=move || kind.get().as_str() on:change=pick_kind>
                        {Kind::ALL.into_iter().map(|k| view! {
                            <option value=k.as_str()>{move || kind_label(i18n, k)}</option>
                        }).collect_view()}
                    </select>
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
            </div>
            <label class="filter">
                <span>{move || i18n.t("lib-name")}</span>
                <input type="text" maxlength="120" required prop:value=move || name.get()
                    on:input=move |ev| name.set(event_target_value(&ev))/>
            </label>
            <label class="filter">
                <span>{move || i18n.t("lib-book")}</span>
                <input type="text" maxlength="120" prop:value=move || book.get()
                    placeholder=move || i18n.t("lib-book-hint")
                    on:input=move |ev| book.set(event_target_value(&ev))/>
            </label>
            <div class="filters">{facet_fields}</div>
            <label class="filter">
                <span>{move || i18n.t("lib-text")}</span>
                <textarea class="note-body" prop:value=move || text.get()
                    on:input=move |ev| text.set(event_target_value(&ev))></textarea>
            </label>
            {move || error.get().map(|k| view! { <p class="error" role="alert">{move || i18n.t(k)}</p> })}
            <div class="toolbar">
                <span class="grow saving">
                    {move || (busy.get() || pending.get()).then(|| view! {
                        <span class="dot warning"></span><span>{i18n.t("note-saving")}</span>
                    })}
                </span>
                <button type="button" on:click=move |_| on_done.run(None)>{move || i18n.t("cancel")}</button>
                <button type="button" class="primary" disabled=move || busy.get() on:click=save>
                    {move || i18n.t("save")}
                </button>
            </div>
        </article>
    }
}
