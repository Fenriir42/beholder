use std::collections::HashMap;

use beholder_core::character::{
    Character, CharacterChange, CharacterOp, Counter, HitPoints, Item, KnownSpell, Link,
};
use beholder_core::compendium::{Facets, Hit, Kind, SearchQuery};
use beholder_core::rules::{Ability, Proficiency, Skill};
use beholder_core::{CampaignId, CharacterId};
use beholder_ipc::{EntryView, ResolveItem};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::use_app;
use crate::i18n::{I18n, use_i18n};
use crate::screens::compendium::{kind_label, meta};
use crate::screens::sheet::{ABILITY_KEYS, EditSheet, class_line, send, signed, skill_key};
use crate::{ipc, markdown};

#[derive(serde::Serialize)]
struct ResolveArgs {
    items: Vec<ResolveItem>,
    sources: Vec<String>,
    hidden: Vec<String>,
}

#[derive(serde::Serialize)]
struct EntryArgs {
    source: String,
    key: String,
    sources: Vec<String>,
}

#[derive(serde::Serialize)]
struct SearchArgs {
    query: SearchQuery,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Actions,
    Spells,
    Inventory,
    Features,
    Notes,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Owner {
    Spell,
    Item,
    Feat,
    Feature,
    Other,
}

#[derive(Clone, PartialEq, Eq)]
enum Panel {
    Closed,
    Entry { link: Link, owner: Owner },
    Browse(Owner),
}

type Resolved = HashMap<String, Hit>;

fn norm(name: &str) -> String {
    name.trim().to_lowercase()
}

fn sources_of(campaign: CampaignId) -> (Vec<String>, Vec<String>) {
    let ctx = use_app();
    ctx.view.with_untracked(|v| {
        v.campaigns
            .iter()
            .find(|c| c.campaign.id == campaign)
            .map(|c| {
                (
                    c.campaign.sources.clone(),
                    c.campaign.hidden.iter().map(|h| h.to_string()).collect(),
                )
            })
            .unwrap_or_default()
    })
}

fn lookups(ch: &Character) -> Vec<ResolveItem> {
    let item = |name: &str, kinds: &[Kind]| ResolveItem {
        name: name.to_owned(),
        kinds: kinds.to_vec(),
    };
    let mut out = Vec::new();
    let unlinked = |l: &Link| l.key.is_none();
    out.extend(
        ch.spells
            .iter()
            .filter(|s| unlinked(&s.spell))
            .map(|s| item(&s.spell.name, &[Kind::Spell])),
    );
    out.extend(
        ch.inventory
            .iter()
            .filter(|i| unlinked(&i.item))
            .map(|i| item(&i.item.name, &[Kind::MagicItem, Kind::Equipment])),
    );
    out.extend(
        ch.feats
            .iter()
            .filter(|l| unlinked(l))
            .map(|l| item(&l.name, &[Kind::Feat])),
    );
    out.extend(
        ch.features
            .iter()
            .filter(|l| unlinked(l))
            .map(|l| item(&l.name, &[Kind::Feature, Kind::Trait, Kind::Feat])),
    );
    if let Some(l) = ch.species.as_ref().filter(|l| unlinked(l)) {
        out.push(item(&l.name, &[Kind::Species]));
    }
    if let Some(l) = ch.background.as_ref().filter(|l| unlinked(l)) {
        out.push(item(&l.name, &[Kind::Background]));
    }
    for c in &ch.classes {
        if unlinked(&c.class) {
            out.push(item(&c.class.name, &[Kind::Class]));
        }
        if let Some(s) = c.subclass.as_ref().filter(|l| unlinked(l)) {
            out.push(item(&s.name, &[Kind::Subclass]));
        }
    }
    out
}

fn hit_for(resolved: &Resolved, link: &Link) -> Option<(String, String)> {
    match (&link.source, &link.key) {
        (Some(s), Some(k)) => Some((s.clone(), k.clone())),
        _ => resolved
            .get(&norm(&link.name))
            .map(|h| (h.source.clone(), h.key.clone())),
    }
}

fn facets_for(resolved: &Resolved, link: &Link) -> Option<Facets> {
    resolved.get(&norm(&link.name)).map(|h| h.facets.clone())
}

#[component]
pub fn Sheet(id: CampaignId, selected: RwSignal<Option<CharacterId>>) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let tab = RwSignal::new(Tab::Spells);
    let editing = RwSignal::new(false);
    let panel = RwSignal::new(Panel::Closed);
    let resolved = RwSignal::new(Resolved::new());
    let tried = StoredValue::new(std::collections::HashSet::<String>::new());
    let character = Memo::new(move |_| {
        let me = ctx.view.with(|v| v.user.as_ref().map(|u| u.id));
        ctx.view.with(|v| {
            let c = v.campaigns.iter().find(|c| c.campaign.id == id)?;
            match selected.get() {
                Some(sel) => c.campaign.character(sel).cloned(),
                None => c
                    .campaign
                    .characters
                    .iter()
                    .find(|ch| Some(ch.owner) == me && ch.active)
                    .cloned(),
            }
        })
    });
    let wanted = Memo::new(move |_| {
        character.with(|c| {
            c.as_ref()
                .map(lookups)
                .unwrap_or_default()
                .into_iter()
                .filter(|i| !tried.with_value(|t| t.contains(&norm(&i.name))))
                .collect::<Vec<_>>()
        })
    });
    Effect::new(move |_| {
        let items = wanted.get();
        if items.is_empty() {
            return;
        }
        let (sources, hidden) = sources_of(id);
        let names: Vec<String> = items.iter().map(|i| norm(&i.name)).collect();
        tried.update_value(|t| t.extend(names.iter().cloned()));
        spawn_local(async move {
            let args = ResolveArgs {
                items,
                sources,
                hidden,
            };
            if let Ok(hits) = ipc::call::<Vec<Option<Hit>>>("compendium_resolve", args).await {
                resolved.try_update(|r| {
                    for (name, hit) in names.into_iter().zip(hits) {
                        if let Some(h) = hit {
                            r.insert(name, h);
                        }
                    }
                });
            }
        });
    });
    move || match character.get() {
        Some(ch) if editing.get() => view! {
            <section class="home sheet">
                <div class="toolbar">
                    <span class="grow"></span>
                    <button type="button" class="primary" on:click=move |_| editing.set(false)>
                        {move || i18n.t("sheet-done-editing")}
                    </button>
                </div>
                <EditSheet campaign=id ch=ch/>
            </section>
        }
        .into_any(),
        Some(ch) => view! {
            <Play campaign=id ch=ch tab=tab editing=editing panel=panel resolved=resolved/>
        }
        .into_any(),
        None => view! {
            <section class="home">
                <div class="empty"><p>{move || i18n.t("sheet-none")}</p></div>
            </section>
        }
        .into_any(),
    }
}

#[component]
fn Play(
    campaign: CampaignId,
    ch: Character,
    tab: RwSignal<Tab>,
    editing: RwSignal<bool>,
    panel: RwSignal<Panel>,
    resolved: RwSignal<Resolved>,
) -> impl IntoView {
    let i18n = use_i18n();
    let classes: Vec<(String, u8)> = ch
        .classes
        .iter()
        .map(|c| {
            let sub = c
                .subclass
                .as_ref()
                .map(|s| format!(" ({})", s.name))
                .unwrap_or_default();
            (format!("{}{sub}", c.class.name), c.level)
        })
        .collect();
    let subtitle = [
        ch.species.as_ref().map(|s| s.name.clone()),
        Some(class_line(&classes)),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    let tab_button = move |t: Tab, key: &'static str| {
        view! {
            <button role="tab" class:active=move || tab.get() == t
                aria-selected=move || (tab.get() == t).to_string()
                on:click=move |_| tab.set(t)>
                {move || i18n.t(key)}
            </button>
        }
    };
    let body = {
        let ch = ch.clone();
        move || match tab.get() {
            Tab::Actions => {
                view! { <ActionsTab campaign=campaign ch=ch.clone() panel=panel/> }.into_any()
            }
            Tab::Spells => {
                view! { <SpellsTab campaign=campaign ch=ch.clone() panel=panel resolved=resolved/> }
                    .into_any()
            }
            Tab::Inventory => {
                view! { <InventoryTab campaign=campaign ch=ch.clone() panel=panel/> }.into_any()
            }
            Tab::Features => view! { <FeaturesTab ch=ch.clone() panel=panel/> }.into_any(),
            Tab::Notes => view! { <NotesTab campaign=campaign ch=ch.clone()/> }.into_any(),
        }
    };
    view! {
        <section class="play">
            <header class="play-head">
                <div class="play-title">
                    <h1>{ch.name.clone()}</h1>
                    <p class="muted">{subtitle}</p>
                </div>
                <button type="button" on:click=move |_| { panel.set(Panel::Closed); editing.set(true); }>
                    {move || i18n.t("sheet-edit")}
                </button>
            </header>
            <Vitals campaign=campaign ch=ch.clone()/>
            <div class="play-layout" class:with-panel=move || panel.with(|p| *p != Panel::Closed)>
                <aside class="play-side">
                    <SavesAndSkills ch=ch.clone()/>
                </aside>
                <div class="play-main">
                    <div class="tabs" role="tablist">
                        {tab_button(Tab::Actions, "sheet-tab-actions")}
                        {tab_button(Tab::Spells, "sheet-tab-spells")}
                        {tab_button(Tab::Inventory, "sheet-tab-inventory")}
                        {tab_button(Tab::Features, "sheet-tab-features")}
                        {tab_button(Tab::Notes, "sheet-tab-notes")}
                    </div>
                    {body}
                </div>
                {move || (panel.get() != Panel::Closed).then(|| view! {
                    <DetailPanel campaign=campaign ch=ch.clone() panel=panel resolved=resolved/>
                })}
            </div>
        </section>
    }
}

#[component]
fn Vitals(campaign: CampaignId, ch: Character) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let amount = RwSignal::new(String::new());
    let hp = ch.hp;
    let apply = move |f: fn(HitPoints, i16) -> HitPoints| {
        let Ok(n) = amount.get_untracked().trim().parse::<i16>() else {
            return;
        };
        amount.set(String::new());
        send(
            campaign,
            CharacterOp::Change {
                id,
                change: CharacterChange::Hp(f(hp, n.max(0))),
            },
        );
    };
    let damage = move |_| {
        apply(|h, n| {
            let from_temp = n.min(h.temp);
            HitPoints {
                temp: h.temp - from_temp,
                current: (h.current - (n - from_temp)).max(0),
                ..h
            }
        })
    };
    let heal = move |_| {
        apply(|h, n| HitPoints {
            current: (h.current + n).min(h.max.max(h.current)),
            ..h
        })
    };
    let temp = move |_| apply(|h, n| HitPoints { temp: n, ..h });
    let abilities = Ability::ALL
        .into_iter()
        .map(|a| {
            view! {
                <div class="ability-chip">
                    <span class="muted small">{move || i18n.t(ABILITY_KEYS[a.index()])}</span>
                    <strong>{signed(ch.modifier(a))}</strong>
                    <span class="muted small">{ch.abilities[a.index()]}</span>
                </div>
            }
        })
        .collect_view();
    let stat = move |key: &'static str, value: String| {
        view! {
            <div class="stat"><span class="muted small">{move || i18n.t(key)}</span><strong>{value}</strong></div>
        }
    };
    let conditions = ch.conditions.join(", ");
    let exhaustion = ch.exhaustion;
    let penalty = ch.d20_penalty();
    view! {
        <div class="vitals">
            <div class="stats">
                {stat("sheet-ac", ch.armor_class().to_string())}
                {stat("sheet-initiative", signed(ch.initiative()))}
                {stat("sheet-speed", ch.speed.to_string())}
                {stat("sheet-proficiency", signed(ch.proficiency_bonus()))}
                <div class="hp-box">
                    <span class="muted small">{move || i18n.t("sheet-hp")}</span>
                    <strong class="hp-value">{format!("{} / {}", hp.current, hp.max)}</strong>
                    {(hp.temp > 0).then(|| view! {
                        <span class="small">{move || i18n.tr("sheet-temp-hp", &[("temp", hp.temp.into())])}</span>
                    })}
                    <div class="field-row">
                        <input type="number" min="0" class="hp-amount" bind:value=amount
                            aria-label=move || i18n.t("sheet-hp-amount") placeholder="0"/>
                        <button type="button" on:click=damage>{move || i18n.t("sheet-damage")}</button>
                        <button type="button" on:click=heal>{move || i18n.t("sheet-heal")}</button>
                        <button type="button" class="ghost" on:click=temp>{move || i18n.t("sheet-temp")}</button>
                    </div>
                </div>
            </div>
            <div class="abilities-strip">{abilities}</div>
            {(!conditions.is_empty() || exhaustion > 0).then(|| view! {
                <p class="small">
                    {(!conditions.is_empty()).then(|| view! {
                        <span>{move || i18n.t("sheet-conditions-label")}" "{conditions.clone()}</span>
                    })}
                    {(exhaustion > 0).then(|| view! {
                        <span>" · "{move || i18n.tr("sheet-exhaustion-line", &[("level", exhaustion.into()), ("penalty", penalty.into())])}</span>
                    })}
                </p>
            })}
        </div>
    }
}

fn prof_mark(p: Proficiency) -> &'static str {
    match p {
        Proficiency::None => "○",
        Proficiency::Proficient => "●",
        Proficiency::Expertise => "◆",
    }
}

fn prof_key(p: Proficiency) -> &'static str {
    match p {
        Proficiency::None => "prof-not",
        Proficiency::Proficient => "prof-proficient",
        Proficiency::Expertise => "prof-expertise",
    }
}

#[component]
fn SavesAndSkills(ch: Character) -> impl IntoView {
    let i18n = use_i18n();
    let saves = Ability::ALL
        .into_iter()
        .map(|a| {
            let p = if ch.saves[a.index()] {
                Proficiency::Proficient
            } else {
                Proficiency::None
            };
            view! {
                <li>
                    <span class="mark" title=move || i18n.t(prof_key(p))>{prof_mark(p)}</span>
                    <span class="grow">{move || i18n.t(ABILITY_KEYS[a.index()])}</span>
                    <strong>{signed(ch.save(a))}</strong>
                </li>
            }
        })
        .collect_view();
    let skills = Skill::ALL
        .into_iter()
        .map(|s| {
            let p = ch.skills[s.index()];
            view! {
                <li>
                    <span class="mark" title=move || i18n.t(prof_key(p))>{prof_mark(p)}</span>
                    <span class="grow">{move || i18n.t(skill_key(s))}
                        <span class="muted small">" "{move || i18n.t(ABILITY_KEYS[s.ability().index()])}</span>
                    </span>
                    <strong>{signed(ch.skill(s))}</strong>
                </li>
            }
        })
        .collect_view();
    view! {
        <h3>{move || i18n.t("sheet-saves")}</h3>
        <ul class="plain-list">{saves}</ul>
        <h3>{move || i18n.t("sheet-skills")}</h3>
        <ul class="plain-list">{skills}</ul>
        <p class="muted small">
            {move || i18n.tr("sheet-passives", &[
                ("perception", ch.passive(Skill::Perception).into()),
                ("insight", ch.passive(Skill::Insight).into()),
                ("investigation", ch.passive(Skill::Investigation).into()),
            ])}
        </p>
    }
}

fn counter_row(
    campaign: CampaignId,
    id: CharacterId,
    all: Vec<Counter>,
    i: usize,
) -> impl IntoView {
    let i18n = use_i18n();
    let c = all[i].clone();
    let stored = StoredValue::new(all);
    let set = move |delta: i32| {
        let mut list = stored.get_value();
        if let Some(c) = list.get_mut(i) {
            let next = (i32::from(c.current) + delta).clamp(0, i32::from(c.max.max(c.current)));
            c.current = next as u16;
        }
        send(
            campaign,
            CharacterOp::Change {
                id,
                change: CharacterChange::Counters(list),
            },
        );
    };
    view! {
        <div class="row">
            <span class="grow">{c.name.clone()}</span>
            <button type="button" class="ghost" aria-label=move || i18n.t("sheet-use-one") on:click=move |_| set(-1)>"−"</button>
            <strong>{format!("{} / {}", c.current, c.max)}</strong>
            <button type="button" class="ghost" aria-label=move || i18n.t("sheet-restore-one") on:click=move |_| set(1)>"+"</button>
        </div>
    }
}

#[component]
fn ActionsTab(campaign: CampaignId, ch: Character, panel: RwSignal<Panel>) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let equipped = ch
        .inventory
        .iter()
        .filter(|i| i.equipped)
        .map(|i| {
            let link = i.item.clone();
            let name = link.name.clone();
            view! {
                <button type="button" class="row link-row"
                    on:click=move |_| panel.set(Panel::Entry { link: link.clone(), owner: Owner::Item })>
                    <span class="grow">{name}</span>
                </button>
            }
        })
        .collect_view();
    let counters = (0..ch.counters.len())
        .map(|i| counter_row(campaign, id, ch.counters.clone(), i))
        .collect_view();
    let (ok, ko) = ch.death_saves;
    let dice_left = ch.level().saturating_sub(ch.hit_dice_spent);
    view! {
        <div class="tab-body">
            <h3>{move || i18n.t("sheet-equipped-items")}</h3>
            <div class="list">{equipped}</div>
            <h3>{move || i18n.t("sheet-limited-uses")}</h3>
            <div class="list">{counters}</div>
            <p class="muted small">
                {move || i18n.tr("sheet-hit-dice-left", &[("left", dice_left.into()), ("total", ch.level().into())])}
                " · "
                {move || i18n.tr("sheet-death-line", &[("ok", ok.into()), ("ko", ko.into())])}
            </p>
        </div>
    }
}

fn spell_badges(i18n: I18n, facets: Option<Facets>) -> String {
    match facets {
        Some(Facets::Spell {
            school,
            ritual,
            concentration,
            ..
        }) => {
            let mut out = school;
            if concentration {
                out.push_str(" · ");
                out.push_str(&i18n.t("sheet-concentration"));
            }
            if ritual {
                out.push_str(" · ");
                out.push_str(&i18n.t("sheet-ritual"));
            }
            out
        }
        _ => String::new(),
    }
}

fn level_label(i18n: I18n, level: u8) -> String {
    if level == 0 {
        i18n.t("sheet-cantrips")
    } else {
        i18n.tr("sheet-spell-level", &[("level", level.into())])
    }
}

#[component]
fn SpellsTab(
    campaign: CampaignId,
    ch: Character,
    panel: RwSignal<Panel>,
    resolved: RwSignal<Resolved>,
) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let filter = RwSignal::new(None::<u8>);
    let text = RwSignal::new(String::new());
    let slots = ch.spell_slots();
    let spent = ch.slots_spent;
    let pips = |level: usize, total: u8, used: u8, on: Callback<u8>| {
        (0..total)
            .map(|k| {
                let is_used = k < used;
                view! {
                    <button type="button" class="pip" class:used=is_used
                        aria-pressed=is_used.to_string()
                        aria-label=move || i18n.tr(if is_used { "sheet-slot-used" } else { "sheet-slot-free" }, &[("level", (level as u32).into())])
                        on:click=move |_| on.run(if is_used { used - 1 } else { used + 1 })>
                        {if is_used { "○" } else { "●" }}
                    </button>
                }
            })
            .collect_view()
    };
    let slot_rows = slots
        .iter()
        .enumerate()
        .filter(|(_, n)| **n > 0)
        .map(|(i, &n)| {
            let set = Callback::new(move |v: u8| {
                let mut s = spent;
                s[i] = v.min(n);
                send(
                    campaign,
                    CharacterOp::Change {
                        id,
                        change: CharacterChange::SlotsSpent(s),
                    },
                );
            });
            view! {
                <div class="slot-row">
                    <span class="muted small">{level_label(i18n, i as u8 + 1)}</span>
                    {pips(i + 1, n, spent[i].min(n), set)}
                </div>
            }
        })
        .collect_view();
    let pact = ch.pact_slots().map(|(count, level)| {
        let set = Callback::new(move |v: u8| {
            send(campaign, CharacterOp::Change { id, change: CharacterChange::PactSpent(v.min(count)) });
        });
        view! {
            <div class="slot-row">
                <span class="muted small">{move || i18n.tr("sheet-pact-label", &[("level", level.into())])}</span>
                {pips(level as usize, count, ch.pact_spent.min(count), set)}
            </div>
        }
    });
    let casting = ch
        .classes
        .iter()
        .filter_map(|c| {
            c.spell_ability.map(|a| {
                (
                    c.class.name.clone(),
                    ch.spell_save_dc(a),
                    ch.spell_attack(a),
                )
            })
        })
        .map(|(name, dc, attack)| {
            view! {
                <span class="stat inline">
                    {move || i18n.tr("sheet-casting-line", &[
                        ("class", name.clone().into()),
                        ("dc", dc.into()),
                        ("attack", signed(attack).into()),
                    ])}
                </span>
            }
        })
        .collect_view();
    let spells = ch.spells.clone();
    let mut levels: Vec<u8> = spells.iter().map(|s| s.level).collect();
    levels.sort_unstable();
    levels.dedup();
    let level_filters = levels
        .iter()
        .map(|&l| {
            view! {
                <button type="button" class="level-filter" class:active=move || filter.get() == Some(l)
                    aria-pressed=move || (filter.get() == Some(l)).to_string()
                    on:click=move |_| filter.update(|f| *f = if *f == Some(l) { None } else { Some(l) })>
                    {level_label(i18n, l)}
                </button>
            }
        })
        .collect_view();
    let stored = StoredValue::new(spells.clone());
    let toggle_prepared = move |name: String| {
        let mut list = stored.get_value();
        if let Some(s) = list.iter_mut().find(|s| s.spell.name == name) {
            s.prepared = !s.prepared;
        }
        send(
            campaign,
            CharacterOp::Change {
                id,
                change: CharacterChange::Spells(list),
            },
        );
    };
    let groups = move || {
        let q = text.get().to_lowercase();
        levels
            .iter()
            .filter(|l| filter.get().is_none_or(|f| f == **l))
            .map(|&l| {
                let rows = spells
                    .iter()
                    .filter(|s| s.level == l && (q.is_empty() || s.spell.name.to_lowercase().contains(&q)))
                    .map(|s| {
                        let link = s.spell.clone();
                        let name = link.name.clone();
                        let badges = spell_badges(i18n, resolved.with(|r| facets_for(r, &link)));
                        let prepared = s.prepared;
                        let toggle_name = name.clone();
                        view! {
                            <div class="row spell-row">
                                <button type="button" class="pip" class:used=!prepared
                                    aria-pressed=prepared.to_string()
                                    title=move || i18n.t(if prepared { "sheet-prepared" } else { "sheet-not-prepared" })
                                    on:click=move |_| toggle_prepared(toggle_name.clone())>
                                    {if prepared { "●" } else { "○" }}
                                </button>
                                <button type="button" class="link-row grow"
                                    on:click=move |_| panel.set(Panel::Entry { link: link.clone(), owner: Owner::Spell })>
                                    <strong>{name}</strong>
                                    <span class="muted small">" "{badges}</span>
                                </button>
                            </div>
                        }
                    })
                    .collect_view();
                view! {
                    <h3>{level_label(i18n, l)}</h3>
                    <div class="list">{rows}</div>
                }
            })
            .collect_view()
    };
    view! {
        <div class="tab-body">
            <div class="casting">{casting}</div>
            <div class="slots">{slot_rows}{pact}</div>
            <div class="toolbar">
                <input type="search" placeholder=move || i18n.t("sheet-search-spells")
                    aria-label=move || i18n.t("sheet-search-spells") bind:value=text/>
                {level_filters}
                <span class="grow"></span>
                <button type="button" class="primary" on:click=move |_| panel.set(Panel::Browse(Owner::Spell))>
                    {move || i18n.t("sheet-add-spells")}
                </button>
            </div>
            {groups}
        </div>
    }
}

#[component]
fn InventoryTab(campaign: CampaignId, ch: Character, panel: RwSignal<Panel>) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let items = ch.inventory.clone();
    let stored = StoredValue::new(items.clone());
    let edit = move |i: usize, f: fn(&mut Item)| {
        let mut list = stored.get_value();
        if let Some(it) = list.get_mut(i) {
            f(it);
        }
        send(
            campaign,
            CharacterOp::Change {
                id,
                change: CharacterChange::Inventory(list),
            },
        );
    };
    let rows = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let link = it.item.clone();
            let name = link.name.clone();
            let note = it.note.clone();
            view! {
                <div class="row">
                    <label class="choice small" title=move || i18n.t("sheet-equipped")>
                        <input type="checkbox" prop:checked=it.equipped
                            aria-label=move || i18n.t("sheet-equipped")
                            on:change=move |_| edit(i, |it| it.equipped = !it.equipped)/>
                    </label>
                    <button type="button" class="link-row grow"
                        on:click=move |_| panel.set(Panel::Entry { link: link.clone(), owner: Owner::Item })>
                        <strong>{name}</strong>
                        {(!note.is_empty()).then(|| view! { <span class="muted small">" · "{note.clone()}</span> })}
                    </button>
                    <button type="button" class="ghost" aria-label=move || i18n.t("sheet-less")
                        on:click=move |_| edit(i, |it| it.quantity = it.quantity.saturating_sub(1))>"−"</button>
                    <strong>{it.quantity}</strong>
                    <button type="button" class="ghost" aria-label=move || i18n.t("sheet-more")
                        on:click=move |_| edit(i, |it| it.quantity = it.quantity.saturating_add(1))>"+"</button>
                </div>
            }
        })
        .collect_view();
    let currency = ch.currency;
    let coins = ["coin-cp", "coin-sp", "coin-ep", "coin-gp", "coin-pp"]
        .into_iter()
        .enumerate()
        .map(|(i, key)| {
            view! {
                <label class="filter coin"><span>{move || i18n.t(key)}</span>
                    <input type="number" min="0" prop:value=currency[i].to_string()
                        on:change=crate::screens::sheet::on_num(move |v: u32| {
                            let mut c = currency;
                            c[i] = v;
                            send(campaign, CharacterOp::Change { id, change: CharacterChange::Currency(c) });
                        })/></label>
            }
        })
        .collect_view();
    view! {
        <div class="tab-body">
            <div class="filters coins">{coins}</div>
            <div class="toolbar">
                <span class="grow"></span>
                <button type="button" class="primary" on:click=move |_| panel.set(Panel::Browse(Owner::Item))>
                    {move || i18n.t("sheet-add-items")}
                </button>
            </div>
            <div class="list">{rows}</div>
        </div>
    }
}

#[component]
fn FeaturesTab(ch: Character, panel: RwSignal<Panel>) -> impl IntoView {
    let i18n = use_i18n();
    let entry = move |link: Link, owner: Owner, label: Option<String>| {
        let name = link.name.clone();
        view! {
            <button type="button" class="row link-row"
                on:click=move |_| panel.set(Panel::Entry { link: link.clone(), owner })>
                <span class="grow"><strong>{name}</strong></span>
                {label.map(|l| view! { <span class="muted small">{l}</span> })}
            </button>
        }
    };
    let mut origin = Vec::new();
    if let Some(s) = &ch.species {
        origin.push(entry(
            s.clone(),
            Owner::Other,
            Some(kind_label(i18n, Kind::Species)),
        ));
    }
    if let Some(b) = &ch.background {
        origin.push(entry(
            b.clone(),
            Owner::Other,
            Some(kind_label(i18n, Kind::Background)),
        ));
    }
    for c in &ch.classes {
        origin.push(entry(
            c.class.clone(),
            Owner::Other,
            Some(format!("{} {}", kind_label(i18n, Kind::Class), c.level)),
        ));
        if let Some(s) = &c.subclass {
            origin.push(entry(
                s.clone(),
                Owner::Other,
                Some(kind_label(i18n, Kind::Subclass)),
            ));
        }
    }
    let feats = ch
        .feats
        .iter()
        .map(|f| entry(f.clone(), Owner::Feat, None))
        .collect_view();
    let features = ch
        .features
        .iter()
        .map(|f| entry(f.clone(), Owner::Feature, None))
        .collect_view();
    view! {
        <div class="tab-body">
            <div class="toolbar">
                <span class="grow"></span>
                <button type="button" class="primary" on:click=move |_| panel.set(Panel::Browse(Owner::Feature))>
                    {move || i18n.t("sheet-add-features")}
                </button>
            </div>
            <h3>{move || i18n.t("sheet-origin")}</h3>
            <div class="list">{origin.into_iter().collect_view()}</div>
            <h3>{move || i18n.t("sheet-feats")}</h3>
            <div class="list">{feats}</div>
            <h3>{move || i18n.t("sheet-features")}</h3>
            <div class="list">{features}</div>
        </div>
    }
}

#[component]
fn NotesTab(campaign: CampaignId, ch: Character) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    view! {
        <div class="tab-body">
            <textarea class="note-body" prop:value=ch.notes.clone()
                aria-label=move || i18n.t("sheet-notes")
                on:change=move |ev| send(campaign, CharacterOp::Change {
                    id,
                    change: CharacterChange::Notes(event_target_value(&ev)),
                })></textarea>
        </div>
    }
}

fn change_list(
    campaign: CampaignId,
    id: CharacterId,
    ch: &Character,
    owner: Owner,
    f: impl FnOnce(&mut Character),
) {
    let mut next = ch.clone();
    f(&mut next);
    let change = match owner {
        Owner::Spell => CharacterChange::Spells(next.spells),
        Owner::Item => CharacterChange::Inventory(next.inventory),
        Owner::Feat => CharacterChange::Feats(next.feats),
        Owner::Feature => CharacterChange::Features(next.features),
        Owner::Other => return,
    };
    send(campaign, CharacterOp::Change { id, change });
}

#[component]
fn DetailPanel(
    campaign: CampaignId,
    ch: Character,
    panel: RwSignal<Panel>,
    resolved: RwSignal<Resolved>,
) -> impl IntoView {
    let i18n = use_i18n();
    let close = move |_| panel.set(Panel::Closed);
    let content = move || {
        match panel.get() {
        Panel::Closed => ().into_any(),
        Panel::Entry { link, owner } => {
            view! { <EntryDetail campaign=campaign ch=ch.clone() link=link owner=owner resolved=resolved panel=panel/> }
                .into_any()
        }
        Panel::Browse(owner) => view! { <Browse campaign=campaign ch=ch.clone() owner=owner/> }.into_any(),
    }
    };
    view! {
        <aside class="play-panel" aria-label=move || i18n.t("sheet-details")>
            <div class="toolbar">
                <span class="grow"></span>
                <button type="button" class="ghost" on:click=close>{move || i18n.t("sheet-close")}</button>
            </div>
            {content}
        </aside>
    }
}

#[component]
fn EntryDetail(
    campaign: CampaignId,
    ch: Character,
    link: Link,
    owner: Owner,
    resolved: RwSignal<Resolved>,
    panel: RwSignal<Panel>,
) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let id = ch.id;
    let entry = RwSignal::new(None::<Option<EntryView>>);
    let target = resolved.with_untracked(|r| hit_for(r, &link));
    match target {
        Some((source, key)) => {
            let (sources, _) = sources_of(campaign);
            spawn_local(async move {
                let found = ipc::call::<Option<EntryView>>(
                    "compendium_entry",
                    EntryArgs {
                        source,
                        key,
                        sources,
                    },
                )
                .await
                .ok()
                .flatten();
                entry.try_set(Some(found));
            });
        }
        None => entry.set(Some(None)),
    }
    let image_base = ctx.boot.with_untracked(|b| b.image_base.clone());
    let name = link.name.clone();
    let actions = {
        let ch = ch.clone();
        let name = name.clone();
        let remove = {
            let ch = ch.clone();
            let name = name.clone();
            move |_| {
                change_list(campaign, id, &ch, owner, |c| match owner {
                    Owner::Spell => c.spells.retain(|s| s.spell.name != name),
                    Owner::Item => {
                        if let Some(i) = c.inventory.iter().position(|it| it.item.name == name) {
                            c.inventory.remove(i);
                        }
                    }
                    Owner::Feat => c.feats.retain(|l| l.name != name),
                    Owner::Feature => c.features.retain(|l| l.name != name),
                    Owner::Other => {}
                });
                panel.set(Panel::Closed);
            }
        };
        let prepared = ch
            .spells
            .iter()
            .find(|s| s.spell.name == name)
            .map(|s| s.prepared);
        let toggle = {
            let ch = ch.clone();
            let name = name.clone();
            move |_| {
                change_list(campaign, id, &ch, Owner::Spell, |c| {
                    if let Some(s) = c.spells.iter_mut().find(|s| s.spell.name == name) {
                        s.prepared = !s.prepared;
                    }
                })
            }
        };
        view! {
            <div class="toolbar">
                {(owner == Owner::Spell).then(|| view! {
                    <button type="button" on:click=toggle.clone()>
                        {move || i18n.t(if prepared == Some(true) { "sheet-unprepare" } else { "sheet-prepare" })}
                    </button>
                })}
                {(owner != Owner::Other).then(|| view! {
                    <button type="button" class="ghost" on:click=remove.clone()>{move || i18n.t("sheet-remove")}</button>
                })}
            </div>
        }
    };
    view! {
        <h2>{name.clone()}</h2>
        {actions}
        {move || match entry.get() {
            None => view! { <p class="muted">{move || i18n.t("sheet-loading")}</p> }.into_any(),
            Some(None) => view! { <p class="muted">{move || i18n.t("sheet-not-in-compendium")}</p> }.into_any(),
            Some(Some(view)) => {
                let html = markdown::render(&view.entry.text, &image_base);
                view! {
                    <p class="muted small">{meta(i18n, &view.entry.facets)}" · "{view.source.name.clone()}</p>
                    <div class="prose" inner_html=html></div>
                }
                .into_any()
            }
        }}
    }
}

fn max_spell_level(ch: &Character) -> u8 {
    let slots = ch.spell_slots();
    let normal = slots
        .iter()
        .rposition(|n| *n > 0)
        .map_or(0, |i| i as u8 + 1);
    let pact = ch.pact_slots().map_or(0, |(_, l)| l);
    normal.max(pact)
}

#[component]
fn Browse(campaign: CampaignId, ch: Character, owner: Owner) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let text = RwSignal::new(String::new());
    let mine = RwSignal::new(true);
    let hits = RwSignal::new(Vec::<Hit>::new());
    let max_level = max_spell_level(&ch);
    let caster_classes: Vec<String> = ch
        .classes
        .iter()
        .filter(|c| c.spell_ability.is_some())
        .map(|c| c.class.name.clone())
        .collect();
    let kinds: &'static [Kind] = match owner {
        Owner::Spell => &[Kind::Spell],
        Owner::Item => &[Kind::MagicItem, Kind::Equipment],
        _ => &[Kind::Feat, Kind::Feature, Kind::Trait],
    };
    let classes = StoredValue::new(caster_classes);
    let run = move || {
        let q = text.get_untracked();
        let (sources, hidden) = sources_of(campaign);
        let only_mine = mine.get_untracked() && owner == Owner::Spell;
        spawn_local(async move {
            let mut out: Vec<Hit> = Vec::new();
            let class_filters: Vec<Option<String>> =
                if only_mine && !classes.with_value(Vec::is_empty) {
                    classes.get_value().into_iter().map(Some).collect()
                } else {
                    vec![None]
                };
            for kind in kinds {
                for class in &class_filters {
                    let query = SearchQuery {
                        text: q.clone(),
                        kind: Some(*kind),
                        class: class.clone(),
                        sources: sources.clone(),
                        hidden: hidden.clone(),
                        ..SearchQuery::default()
                    };
                    if let Ok(h) =
                        ipc::call::<Vec<Hit>>("compendium_search", SearchArgs { query }).await
                    {
                        out.extend(h);
                    }
                }
            }
            out.retain(|h| match (&h.facets, owner) {
                (Facets::Spell { level, .. }, Owner::Spell) => !only_mine || *level <= max_level,
                _ => true,
            });
            out.sort_by(|a, b| a.name.cmp(&b.name));
            out.dedup_by(|a, b| a.source == b.source && a.key == b.key);
            hits.try_set(out.into_iter().take(60).collect());
        });
    };
    run();
    let stored = StoredValue::new(ch);
    let add = move |h: Hit| {
        let link = Link {
            name: h.name.clone(),
            source: Some(h.source.clone()),
            key: Some(h.key.clone()),
        };
        let current = stored.get_value();
        let level = match h.facets {
            Facets::Spell { level, .. } => level,
            _ => 0,
        };
        let target = match (owner, &h.facets) {
            (Owner::Spell, _) => Owner::Spell,
            (Owner::Item, _) => Owner::Item,
            (_, Facets::Feat { .. }) => Owner::Feat,
            _ => Owner::Feature,
        };
        change_list(campaign, id, &current, target, |c| match target {
            Owner::Spell => c.spells.push(KnownSpell {
                spell: link,
                level,
                prepared: level == 0,
            }),
            Owner::Item => c.inventory.push(Item {
                item: link,
                quantity: 1,
                equipped: false,
                note: String::new(),
            }),
            Owner::Feat => c.feats.push(link),
            Owner::Feature | Owner::Other => c.features.push(link),
        });
    };
    let custom = move |_| {
        let name = text.get_untracked().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let link = Link {
            name,
            source: None,
            key: None,
        };
        let current = stored.get_value();
        let target = if owner == Owner::Other {
            Owner::Feature
        } else {
            owner
        };
        change_list(campaign, id, &current, target, |c| match target {
            Owner::Spell => c.spells.push(KnownSpell {
                spell: link,
                level: 0,
                prepared: false,
            }),
            Owner::Item => c.inventory.push(Item {
                item: link,
                quantity: 1,
                equipped: false,
                note: String::new(),
            }),
            Owner::Feat => c.feats.push(link),
            Owner::Feature | Owner::Other => c.features.push(link),
        });
        text.set(String::new());
    };
    let title = match owner {
        Owner::Spell => "sheet-add-spells",
        Owner::Item => "sheet-add-items",
        _ => "sheet-add-features",
    };
    view! {
        <h2>{move || i18n.t(title)}</h2>
        <div class="toolbar">
            <input type="search" class="grow" placeholder=move || i18n.t("cmp-search") aria-label=move || i18n.t("cmp-search")
                bind:value=text on:input=move |_| run()/>
        </div>
        {(owner == Owner::Spell).then(|| view! {
            <label class="choice small">
                <input type="checkbox" prop:checked=move || mine.get()
                    on:change=move |ev| { mine.set(event_target_checked(&ev)); run(); }/>
                {move || i18n.tr("sheet-only-castable", &[("level", max_level.into())])}
            </label>
        })}
        <div class="list">
            {move || hits.get().into_iter().map(|h| {
                let label = meta(i18n, &h.facets);
                let name = h.name.clone();
                let hit = h.clone();
                view! {
                    <div class="row">
                        <span class="grow"><strong>{name}</strong><span class="muted small">" "{label}</span></span>
                        <button type="button" on:click=move |_| add(hit.clone())>{move || i18n.t("sheet-add")}</button>
                    </div>
                }
            }).collect_view()}
        </div>
        <div class="toolbar">
            <span class="muted small grow">{move || i18n.t("sheet-custom-hint")}</span>
            <button type="button" class="ghost" on:click=custom>{move || i18n.t("sheet-add-custom")}</button>
        </div>
    }
}
