use std::collections::HashMap;

use beholder_core::actions::{Details, Effect as Fx, Resolution, spell_effect, weapon_effect};
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
struct DetailsArgs {
    refs: Vec<(String, String)>,
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

type DetailsMap = HashMap<(String, String), Details>;

#[derive(Clone, Copy)]
struct Lookups {
    resolved: RwSignal<Resolved>,
    details: RwSignal<DetailsMap>,
}

impl Lookups {
    fn details(&self, link: &Link) -> Option<Details> {
        let key = self.resolved.with(|r| hit_for(r, link))?;
        self.details.with(|d| d.get(&key).cloned())
    }

    fn facets(&self, link: &Link) -> Option<Facets> {
        self.resolved.with(|r| facets_for(r, link))
    }
}

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

fn link_id(link: &Link) -> String {
    match (&link.source, &link.key) {
        (Some(s), Some(k)) => format!("{s}|{k}"),
        _ => norm(&link.name),
    }
}

fn lookups(ch: &Character) -> Vec<(String, ResolveItem)> {
    let item = |link: &Link, kinds: &[Kind], level: Option<u8>| {
        (
            link_id(link),
            ResolveItem {
                name: link.name.clone(),
                kinds: kinds.to_vec(),
                level,
                entry: link.source.clone().zip(link.key.clone()),
            },
        )
    };
    let mut out = Vec::new();
    out.extend(
        ch.spells
            .iter()
            .map(|s| item(&s.spell, &[Kind::Spell], Some(s.level))),
    );
    out.extend(
        ch.inventory
            .iter()
            .map(|i| item(&i.item, &[Kind::MagicItem, Kind::Equipment], None)),
    );
    out.extend(ch.feats.iter().map(|l| item(l, &[Kind::Feat], None)));
    out.extend(
        ch.features
            .iter()
            .map(|l| item(l, &[Kind::Feature, Kind::Trait, Kind::Feat], None)),
    );
    if let Some(l) = &ch.species {
        out.push(item(l, &[Kind::Species], None));
    }
    if let Some(l) = &ch.background {
        out.push(item(l, &[Kind::Background], None));
    }
    for c in &ch.classes {
        out.push(item(&c.class, &[Kind::Class], None));
        if let Some(s) = &c.subclass {
            out.push(item(s, &[Kind::Subclass], None));
        }
    }
    out
}

fn hit_for(resolved: &Resolved, link: &Link) -> Option<(String, String)> {
    match (&link.source, &link.key) {
        (Some(s), Some(k)) => Some((s.clone(), k.clone())),
        _ => resolved
            .get(&link_id(link))
            .map(|h| (h.source.clone(), h.key.clone())),
    }
}

fn facets_for(resolved: &Resolved, link: &Link) -> Option<Facets> {
    resolved.get(&link_id(link)).map(|h| h.facets.clone())
}

fn spell_ability(ch: &Character, facets: Option<&Facets>) -> Ability {
    let casters: Vec<(&str, Ability)> = ch
        .classes
        .iter()
        .filter_map(|c| c.spell_ability.map(|a| (c.class.name.as_str(), a)))
        .collect();
    if let Some(Facets::Spell { classes, .. }) = facets
        && let Some((_, a)) = casters
            .iter()
            .find(|(n, _)| classes.iter().any(|c| c.eq_ignore_ascii_case(n)))
    {
        return *a;
    }
    casters.first().map_or(Ability::Cha, |(_, a)| *a)
}

fn effect_line(i18n: I18n, e: &Fx) -> String {
    let mut parts = Vec::new();
    match e.resolution {
        Resolution::Attack { bonus } => {
            parts.push(i18n.tr("fx-attack", &[("bonus", signed(bonus).into())]));
        }
        Resolution::Save { ability, dc, .. } => parts.push(i18n.tr(
            "fx-save",
            &[
                ("ability", i18n.t(ABILITY_KEYS[ability.index()]).into()),
                ("dc", dc.into()),
            ],
        )),
        Resolution::Automatic => {}
    }
    if let Some(d) = e.damage {
        let times = if e.times > 1 {
            format!("{}× ", e.times)
        } else {
            String::new()
        };
        let kind = e.damage_type.clone().unwrap_or_default();
        parts.push(format!("{times}{d} {kind}").trim().to_owned());
        if matches!(e.resolution, Resolution::Save { half: true, .. }) {
            parts.push(i18n.t("fx-half"));
        }
    }
    if let Some(h) = e.healing {
        parts.push(i18n.tr("fx-heal", &[("roll", h.to_string().into())]));
    }
    parts.join(" · ")
}

fn spell_fx(ch: &Character, look: Lookups, s: &KnownSpell, cast_level: u8) -> Option<Fx> {
    let info = look.details(&s.spell)?.spell?;
    let ability = spell_ability(ch, look.facets(&s.spell).as_ref());
    Some(spell_effect(
        ch,
        &info,
        s.level,
        cast_level.max(s.level),
        ability,
    ))
}

fn meaningful(e: &Fx) -> bool {
    e.damage.is_some() || e.healing.is_some() || e.resolution != Resolution::Automatic
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
    let details = RwSignal::new(DetailsMap::new());
    let fetched = StoredValue::new(std::collections::HashSet::<(String, String)>::new());
    let look = Lookups { resolved, details };
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
                .filter(|(id, _)| !tried.with_value(|t| t.contains(id)))
                .collect::<Vec<_>>()
        })
    });
    Effect::new(move |_| {
        let items = wanted.get();
        if items.is_empty() {
            return;
        }
        let (sources, hidden) = sources_of(id);
        let (names, items): (Vec<String>, Vec<ResolveItem>) = items.into_iter().unzip();
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
    let wanted_details = Memo::new(move |_| {
        let links: Vec<Link> = character.with(|c| {
            c.as_ref()
                .map(|c| {
                    c.spells
                        .iter()
                        .map(|s| s.spell.clone())
                        .chain(c.inventory.iter().map(|i| i.item.clone()))
                        .collect()
                })
                .unwrap_or_default()
        });
        resolved.with(|r| {
            links
                .iter()
                .filter_map(|l| hit_for(r, l))
                .filter(|k| !fetched.with_value(|f| f.contains(k)))
                .collect::<Vec<_>>()
        })
    });
    Effect::new(move |_| {
        let refs = wanted_details.get();
        if refs.is_empty() {
            return;
        }
        fetched.update_value(|f| f.extend(refs.iter().cloned()));
        spawn_local(async move {
            let args = DetailsArgs { refs: refs.clone() };
            if let Ok(found) = ipc::call::<Vec<Option<Details>>>("sheet_details", args).await {
                details.try_update(|d| {
                    for (k, v) in refs.into_iter().zip(found) {
                        if let Some(v) = v {
                            d.insert(k, v);
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
            <Play campaign=id ch=ch tab=tab editing=editing panel=panel look=look/>
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
    look: Lookups,
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
                view! { <ActionsTab campaign=campaign ch=ch.clone() panel=panel look=look/> }
                    .into_any()
            }
            Tab::Spells => {
                view! { <SpellsTab campaign=campaign ch=ch.clone() panel=panel look=look/> }
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
                    <DetailPanel campaign=campaign ch=ch.clone() panel=panel look=look/>
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
fn ActionsTab(
    campaign: CampaignId,
    ch: Character,
    panel: RwSignal<Panel>,
    look: Lookups,
) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let equipped = ch
        .inventory
        .iter()
        .filter(|i| i.equipped)
        .map(|i| {
            let link = i.item.clone();
            let name = link.name.clone();
            let weapon = look.details(&link).and_then(|d| d.weapon);
            let line = weapon.as_ref().map(|w| {
                let mut l = effect_line(i18n, &weapon_effect(&ch, w, false));
                if w.versatile.is_some() {
                    let two = weapon_effect(&ch, w, true);
                    if let Some(d) = two.damage {
                        l.push_str(&i18n.tr("fx-two-handed", &[("roll", d.to_string().into())]));
                    }
                }
                l
            });
            view! {
                <button type="button" class="row link-row"
                    on:click=move |_| panel.set(Panel::Entry { link: link.clone(), owner: Owner::Item })>
                    <span class="spell-name"><strong>{name}</strong></span>
                    {line.map(|l| view! { <span class="effect">{l}</span> })}
                </button>
            }
        })
        .collect_view();
    let spell_attacks = ch
        .spells
        .iter()
        .filter(|s| s.level == 0 || s.prepared)
        .filter_map(|s| {
            let fx = spell_fx(&ch, look, s, s.level).filter(meaningful)?;
            let link = s.spell.clone();
            let name = link.name.clone();
            let line = effect_line(i18n, &fx);
            Some(view! {
                <button type="button" class="row link-row"
                    on:click=move |_| panel.set(Panel::Entry { link: link.clone(), owner: Owner::Spell })>
                    <span class="spell-name"><strong>{name}</strong></span>
                    <span class="effect">{line}</span>
                </button>
            })
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
            <h3>{move || i18n.t("sheet-spell-actions")}</h3>
            <div class="list">{spell_attacks}</div>
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
    look: Lookups,
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
                        let badges = spell_badges(i18n, look.facets(&link));
                        let fx = spell_fx(&ch, look, s, s.level)
                            .filter(meaningful)
                            .map(|e| effect_line(i18n, &e));
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
                                    <span class="spell-name">
                                        <strong>{name}</strong>
                                        <span class="muted small">" "{badges}</span>
                                    </span>
                                    {fx.map(|f| view! { <span class="effect">{f}</span> })}
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
    look: Lookups,
) -> impl IntoView {
    let i18n = use_i18n();
    let close = move |_| panel.set(Panel::Closed);
    let content = move || {
        match panel.get() {
        Panel::Closed => ().into_any(),
        Panel::Entry { link, owner } => {
            view! { <EntryDetail campaign=campaign ch=ch.clone() link=link owner=owner look=look panel=panel/> }
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
    look: Lookups,
    panel: RwSignal<Panel>,
) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let id = ch.id;
    let entry = RwSignal::new(None::<Option<EntryView>>);
    let target = look.resolved.with_untracked(|r| hit_for(r, &link));
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
    let sheet = StoredValue::new(ch.clone());
    let name_for_pick = StoredValue::new(name.clone());
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
    let computed = match owner {
        Owner::Spell => ch.spells.iter().find(|s| s.spell.name == name).and_then(|known| {
            let info = look.details(&link)?.spell?;
            let facts = [
                info.casting_time.clone(),
                info.range.clone(),
                info.duration.clone(),
                info.components.clone(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
            let top = max_spell_level(&ch).max(known.level);
            let scales = info.upcast.is_some() || info.extra_projectile_per_level;
            let levels: Vec<u8> = if known.level == 0 || !scales {
                vec![known.level]
            } else {
                (known.level..=top).collect()
            };
            let rows = levels
                .into_iter()
                .filter_map(|l| {
                    let fx = spell_fx(&ch, look, known, l).filter(meaningful)?;
                    let label = if known.level == 0 {
                        level_label(i18n, 0)
                    } else {
                        level_label(i18n, l)
                    };
                    Some(view! { <li><span class="muted small">{label}</span>" "{effect_line(i18n, &fx)}</li> })
                })
                .collect::<Vec<_>>();
            Some(view! {
                <div class="computed">
                    <p class="small">{facts}</p>
                    <ul class="plain-list">{rows}</ul>
                </div>
            }
            .into_any())
        }),
        Owner::Item => look.details(&link).and_then(|d| d.weapon).map(|w| {
            let one = effect_line(i18n, &weapon_effect(&ch, &w, false));
            let two = w
                .versatile
                .is_some()
                .then(|| effect_line(i18n, &weapon_effect(&ch, &w, true)));
            view! {
                <div class="computed">
                    <p>{one}</p>
                    {two.map(|t| view! { <p>{move || i18n.t("fx-two-handed-label")}" "{t}</p> })}
                </div>
            }
            .into_any()
        }),
        _ => None,
    };
    view! {
        <h2>{name.clone()}</h2>
        {actions}
        {computed}
        {move || match entry.get() {
            None => view! { <p class="muted">{move || i18n.t("sheet-loading")}</p> }.into_any(),
            Some(None) => view! { <p class="muted">{move || i18n.t("sheet-not-in-compendium")}</p> }.into_any(),
            Some(Some(view)) => {
                let html = markdown::render(&view.entry.text, &image_base);
                let from = view.entry.book.clone().unwrap_or_else(|| view.source.name.clone());
                let others = (owner != Owner::Other && !view.also.is_empty()).then(|| {
                    let rows = view
                        .also
                        .iter()
                        .map(|alt| {
                            let label = format!(
                                "{} · {}",
                                alt.book.clone().unwrap_or_else(|| alt.source_name.clone()),
                                meta(i18n, &alt.facets)
                            );
                            let (source, key) = (alt.source.clone(), alt.key.clone());
                            let pick = move |_| {
                                let target = name_for_pick.get_value();
                                let (source, key) = (Some(source.clone()), Some(key.clone()));
                                change_list(campaign, id, &sheet.get_value(), owner, move |c| {
                                    let relink = |l: &mut Link| {
                                        if l.name == target {
                                            l.source = source.clone();
                                            l.key = key.clone();
                                        }
                                    };
                                    match owner {
                                        Owner::Spell => c.spells.iter_mut().for_each(|s| relink(&mut s.spell)),
                                        Owner::Item => c.inventory.iter_mut().for_each(|i| relink(&mut i.item)),
                                        Owner::Feat => c.feats.iter_mut().for_each(relink),
                                        Owner::Feature => c.features.iter_mut().for_each(relink),
                                        Owner::Other => {}
                                    }
                                });
                                panel.set(Panel::Closed);
                            };
                            view! {
                                <li>
                                    <span class="grow small">{label}</span>
                                    <button type="button" class="ghost" on:click=pick>{move || i18n.t("sheet-use-version")}</button>
                                </li>
                            }
                        })
                        .collect_view();
                    view! {
                        <details class="license">
                            <summary>{move || i18n.tr("sheet-other-versions", &[("count", view.also.len().into())])}</summary>
                            <ul class="plain-list">{rows}</ul>
                        </details>
                    }
                });
                view! {
                    <p class="muted small">{meta(i18n, &view.entry.facets)}" · "{from}</p>
                    {others}
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
