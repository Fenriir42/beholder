use beholder_core::character::{
    Character, CharacterChange, CharacterOp, ClassLevel, Counter, HitPoints, Link,
};
use beholder_core::compendium::{Hit, Kind, RulesVersion, SearchQuery};
use beholder_core::rules::{Ability, ArmorFormula, Caster, Proficiency, Skill, class_defaults};
use beholder_core::{CampaignId, CharacterId, UserId};
use beholder_ipc::CampaignView;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::{error_key, use_app};
use crate::i18n::{I18n, use_i18n};
use crate::ipc;
use crate::screens::compendium::rules_label;
use crate::screens::entry_editor::RULES;

#[derive(serde::Serialize)]
struct OpArgs {
    campaign_id: String,
    op: CharacterOp,
}

#[derive(serde::Serialize)]
struct CreateArgs {
    campaign_id: String,
    name: String,
    rules: RulesVersion,
}

#[derive(serde::Serialize)]
struct CampaignArgs {
    campaign_id: String,
}

#[derive(serde::Serialize)]
struct ExportArgs {
    campaign_id: String,
    id: CharacterId,
}

#[derive(serde::Serialize)]
struct SearchArgs {
    query: SearchQuery,
}

pub(crate) const ABILITY_KEYS: [&str; 6] =
    ["ab-str", "ab-dex", "ab-con", "ab-int", "ab-wis", "ab-cha"];

const CASTERS: [(Caster, &str); 6] = [
    (Caster::None, "caster-none"),
    (Caster::Full, "caster-full"),
    (Caster::Half, "caster-half"),
    (Caster::HalfRoundedUp, "caster-half-up"),
    (Caster::Third, "caster-third"),
    (Caster::Pact, "caster-pact"),
];

pub(crate) fn skill_key(s: Skill) -> &'static str {
    match s {
        Skill::Acrobatics => "skill-acrobatics",
        Skill::AnimalHandling => "skill-animal-handling",
        Skill::Arcana => "skill-arcana",
        Skill::Athletics => "skill-athletics",
        Skill::Deception => "skill-deception",
        Skill::History => "skill-history",
        Skill::Insight => "skill-insight",
        Skill::Intimidation => "skill-intimidation",
        Skill::Investigation => "skill-investigation",
        Skill::Medicine => "skill-medicine",
        Skill::Nature => "skill-nature",
        Skill::Perception => "skill-perception",
        Skill::Performance => "skill-performance",
        Skill::Persuasion => "skill-persuasion",
        Skill::Religion => "skill-religion",
        Skill::SleightOfHand => "skill-sleight-of-hand",
        Skill::Stealth => "skill-stealth",
        Skill::Survival => "skill-survival",
    }
}

pub(crate) fn signed(v: impl Into<i32>) -> String {
    let v = v.into();
    if v >= 0 {
        format!("+{v}")
    } else {
        v.to_string()
    }
}

pub fn class_line(classes: &[(String, u8)]) -> String {
    classes
        .iter()
        .map(|(c, l)| format!("{c} {l}"))
        .collect::<Vec<_>>()
        .join(" / ")
}

pub(crate) fn ability_from(v: &str) -> Option<Ability> {
    Ability::ALL.into_iter().find(|a| format!("{a:?}") == v)
}

pub(crate) fn num<T: std::str::FromStr>(ev: &leptos::ev::Event) -> Option<T> {
    event_target_value(ev).trim().parse().ok()
}

pub(crate) fn on_num<T: std::str::FromStr>(
    f: impl Fn(T) + 'static,
) -> impl Fn(leptos::ev::Event) + 'static {
    move |ev| {
        if let Some(v) = num::<T>(&ev) {
            f(v);
        }
    }
}

pub(crate) fn send(campaign: CampaignId, op: CharacterOp) {
    let ctx = use_app();
    spawn_local(async move {
        let args = OpArgs {
            campaign_id: campaign.to_string(),
            op,
        };
        if let Err(e) = ipc::call::<()>("character_command", args).await {
            ctx.notify(error_key(e));
        }
    });
}

#[component]
pub fn CharacterList(
    c: CampaignView,
    me: Option<UserId>,
    open: Callback<CharacterId>,
) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let campaign = c.campaign.id;
    let name = RwSignal::new(String::new());
    let rules = RwSignal::new(RulesVersion::R2024);
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let args = CreateArgs {
            campaign_id: campaign.to_string(),
            name: name.get_untracked(),
            rules: rules.get_untracked(),
        };
        spawn_local(async move {
            match ipc::call::<String>("character_create", args).await {
                Ok(id) => {
                    name.set(String::new());
                    if let Ok(uuid) = id.parse() {
                        open.run(CharacterId(uuid));
                    }
                }
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    };
    let import = move |_| {
        spawn_local(async move {
            let args = CampaignArgs {
                campaign_id: campaign.to_string(),
            };
            match ipc::call::<Option<String>>("character_import", args).await {
                Ok(Some(id)) => {
                    if let Ok(uuid) = id.parse() {
                        open.run(CharacterId(uuid));
                    }
                }
                Ok(None) => {}
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    };
    let owner_name = |owner: UserId| {
        c.campaign
            .members
            .iter()
            .find(|m| m.user_id == owner)
            .map(|m| m.display_name.as_str().to_owned())
            .unwrap_or_default()
    };
    let rows = c
        .campaign
        .characters
        .iter()
        .map(|ch| {
            let id = ch.id;
            let mine = Some(ch.owner) == me;
            let classes: Vec<(String, u8)> = ch
                .classes
                .iter()
                .map(|k| (k.class.name.clone(), k.level))
                .collect();
            let owner = (!mine).then(|| owner_name(ch.owner));
            let active = ch.active;
            let pending = c.pending_characters.contains(&id);
            view! {
                <div class="row">
                    <span class="grow">
                        <strong>{ch.name.clone()}</strong>
                        <span class="muted small">" "{class_line(&classes)}</span>
                        {owner.map(|o| view! { <span class="muted small">" · "{o}</span> })}
                    </span>
                    {active.then(|| view! { <span class="badge">{move || i18n.t("sheet-active")}</span> })}
                    {pending.then(|| view! { <span class="dot warning" title=move || i18n.t("pending")></span> })}
                    {(!active).then(|| view! {
                        <button type="button" class="ghost"
                            on:click=move |_| send(campaign, CharacterOp::SetActive { id })>
                            {move || i18n.t("sheet-set-active")}
                        </button>
                    })}
                    <button type="button" on:click=move |_| open.run(id)>{move || i18n.t("sheet-open")}</button>
                </div>
            }
        })
        .collect_view();
    view! {
        <div class="list">
            {c.campaign.characters.is_empty().then(|| view! {
                <p class="muted">{move || i18n.t("characters-empty")}</p>
            })}
            {rows}
            <form class="field-row" on:submit=create>
                <input type="text" required maxlength="80" class="grow" bind:value=name
                    placeholder=move || i18n.t("sheet-new-name") aria-label=move || i18n.t("sheet-new-name")/>
                <select aria-label=move || i18n.t("lib-rules") prop:value=move || format!("{:?}", rules.get())
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
                <button type="submit" class="primary">{move || i18n.t("sheet-new")}</button>
                <button type="button" on:click=import>{move || i18n.t("sheet-import")}</button>
            </form>
        </div>
    }
}

#[component]
fn LinkInput(
    campaign: CampaignId,
    kind: Option<Kind>,
    value: Option<Link>,
    on_set: Callback<Option<Link>>,
    label: &'static str,
) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let hits = RwSignal::new(Vec::<Hit>::new());
    let list_id = format!("links-{}", (js_sys::Math::random() * 1e12) as u64);
    let linked = value.as_ref().is_some_and(|l| l.key.is_some());
    let search = move |ev: leptos::ev::Event| {
        let text = event_target_value(&ev);
        if text.trim().chars().count() < 2 {
            return;
        }
        let (sources, hidden) = ctx.view.with_untracked(|v| {
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
        });
        let query = SearchQuery {
            text,
            kind,
            sources,
            hidden,
            ..SearchQuery::default()
        };
        spawn_local(async move {
            if let Ok(h) = ipc::call::<Vec<Hit>>("compendium_search", SearchArgs { query }).await {
                hits.try_set(h.into_iter().take(8).collect());
            }
        });
    };
    let pick = move |ev: leptos::ev::Event| {
        let text = event_target_value(&ev).trim().to_owned();
        if text.is_empty() {
            on_set.run(None);
            return;
        }
        let found = hits.with_untracked(|h| {
            h.iter()
                .find(|x| x.name.eq_ignore_ascii_case(&text))
                .map(|x| Link {
                    name: x.name.clone(),
                    source: Some(x.source.clone()),
                    key: Some(x.key.clone()),
                })
        });
        on_set.run(Some(found.unwrap_or(Link {
            name: text,
            source: None,
            key: None,
        })));
    };
    view! {
        <label class="filter">
            <span>
                {move || i18n.t(label)}
                {linked.then(|| view! { <span class="muted small">" · "{move || i18n.t("sheet-linked")}</span> })}
            </span>
            <input type="text" maxlength="120" list=list_id.clone()
                prop:value=value.map(|l| l.name).unwrap_or_default()
                on:input=search on:change=pick/>
            <datalist id=list_id>
                {move || hits.get().into_iter().map(|h| view! { <option value=h.name/> }).collect_view()}
            </datalist>
        </label>
    }
}

#[component]
pub fn EditSheet(campaign: CampaignId, ch: Character) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let exported = RwSignal::new(None::<String>);
    let id = ch.id;
    let change = move |c: CharacterChange| send(campaign, CharacterOp::Change { id, change: c });
    let pb = ch.proficiency_bonus();

    let level = ch.level();
    let header = {
        let ch = ch.clone();
        view! {
            <header class="sheet-head">
                <input type="text" class="sheet-name" maxlength="80" prop:value=ch.name.clone()
                    aria-label=move || i18n.t("sheet-name")
                    on:change=move |ev| change(CharacterChange::Name(event_target_value(&ev)))/>
                <span class="muted">
                    {move || i18n.tr("sheet-level", &[("level", level.into()), ("pb", signed(pb).into())])}
                </span>
                <select aria-label=move || i18n.t("lib-rules") prop:value=format!("{:?}", ch.rules)
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        if let Some(r) = RULES.into_iter().find(|r| format!("{r:?}") == v) {
                            change(CharacterChange::Rules(r));
                        }
                    }>
                    {RULES.into_iter().map(|r| view! {
                        <option value=format!("{r:?}")>{move || rules_label(i18n, r)}</option>
                    }).collect_view()}
                </select>
            </header>
            <div class="filters">
                <LinkInput campaign=campaign kind=Some(Kind::Species) value=ch.species.clone() label="kind-species"
                    on_set=Callback::new(move |l| change(CharacterChange::Species(l)))/>
                <LinkInput campaign=campaign kind=Some(Kind::Background) value=ch.background.clone() label="kind-background"
                    on_set=Callback::new(move |l| change(CharacterChange::Background(l)))/>
            </div>
        }
    };

    view! {
        <section class="sheet-edit">
            {header}
            <Classes campaign=campaign ch=ch.clone()/>
            <Abilities campaign=campaign ch=ch.clone()/>
            <Combat campaign=campaign ch=ch.clone()/>
            <Skills campaign=campaign ch=ch.clone()/>
            <Counters campaign=campaign ch=ch.clone()/>
            <Links campaign=campaign ch=ch.clone()/>
            <div class="toolbar">
                {move || exported.get().map(|p| view! { <span class="muted small">{p}</span> })}
                <span class="grow"></span>
                <button type="button" on:click=move |_| {
                    spawn_local(async move {
                        let args = ExportArgs { campaign_id: campaign.to_string(), id };
                        match ipc::call::<String>("character_export", args).await {
                            Ok(path) => exported.set(Some(i18n.tr("sheet-exported", &[("path", path.into())]))),
                            Err(e) => ctx.notify(error_key(e)),
                        }
                    });
                }>{move || i18n.t("sheet-export")}</button>
                <button type="button" class="ghost"
                    on:click=move |_| send(campaign, CharacterOp::Delete { id })>
                    {move || i18n.t("sheet-delete")}
                </button>
            </div>
        </section>
    }
}

#[component]
fn Classes(campaign: CampaignId, ch: Character) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let classes = ch.classes.clone();
    let set = move |list: Vec<ClassLevel>| {
        send(
            campaign,
            CharacterOp::Change {
                id,
                change: CharacterChange::Classes(list),
            },
        )
    };
    let stored = StoredValue::new(classes.clone());
    let rows = classes
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let list = stored;
            let edit = move |f: Box<dyn FnOnce(&mut ClassLevel)>| {
                let mut l = list.get_value();
                if let Some(c) = l.get_mut(i) {
                    f(c);
                }
                set(l);
            };
            let on_class = Callback::new(move |link: Option<Link>| {
                edit(Box::new(move |c| {
                    if let Some(link) = link {
                        if let Some((die, caster, ability)) = class_defaults(&link.name) {
                            c.hit_die = die;
                            c.caster = caster;
                            c.spell_ability = ability;
                        }
                        c.class = link;
                    }
                }));
            });
            let on_sub = Callback::new(move |link: Option<Link>| edit(Box::new(move |c| c.subclass = link)));
            let remove = {
                let list = stored;
                move |_| {
                    let mut l = list.get_value();
                    l.remove(i);
                    set(l);
                }
            };
            view! {
                <div class="filters sheet-class">
                    <LinkInput campaign=campaign kind=Some(Kind::Class) value=Some(c.class.clone()) label="kind-class" on_set=on_class/>
                    <LinkInput campaign=campaign kind=Some(Kind::Subclass) value=c.subclass.clone() label="kind-subclass" on_set=on_sub/>
                    <label class="filter">
                        <span>{move || i18n.t("cmp-level")}</span>
                        <input type="number" min="1" max="20" prop:value=c.level.to_string()
                            on:change=on_num(move |v: u8| {
                                edit(Box::new(move |c| c.level = v.clamp(1, 20)));
                            })/>
                    </label>
                    <label class="filter">
                        <span>{move || i18n.t("sheet-hit-die")}</span>
                        <select prop:value=c.hit_die.to_string()
                            on:change=on_num(move |v: u8| {
                                edit(Box::new(move |c| c.hit_die = v));
                            })>
                            {[6u8, 8, 10, 12].into_iter().map(|d| view! { <option value=d.to_string()>{format!("d{d}")}</option> }).collect_view()}
                        </select>
                    </label>
                    <label class="filter">
                        <span>{move || i18n.t("sheet-caster")}</span>
                        <select prop:value=format!("{:?}", c.caster)
                            on:change=move |ev| {
                                let v = event_target_value(&ev);
                                if let Some((k, _)) = CASTERS.into_iter().find(|(k, _)| format!("{k:?}") == v) {
                                    edit(Box::new(move |c| c.caster = k));
                                }
                            }>
                            {CASTERS.into_iter().map(|(k, key)| view! {
                                <option value=format!("{k:?}")>{move || i18n.t(key)}</option>
                            }).collect_view()}
                        </select>
                    </label>
                    <label class="filter">
                        <span>{move || i18n.t("sheet-spell-ability")}</span>
                        <select prop:value=c.spell_ability.map(|a| format!("{a:?}")).unwrap_or_default()
                            on:change=move |ev| {
                                let a = ability_from(&event_target_value(&ev));
                                edit(Box::new(move |c| c.spell_ability = a));
                            }>
                            <option value="">"—"</option>
                            {Ability::ALL.into_iter().map(|a| view! {
                                <option value=format!("{a:?}")>{move || i18n.t(ABILITY_KEYS[a.index()])}</option>
                            }).collect_view()}
                        </select>
                    </label>
                    <button type="button" class="ghost" on:click=remove aria-label=move || i18n.t("sheet-remove")>
                        {move || i18n.t("sheet-remove")}
                    </button>
                </div>
            }
        })
        .collect_view();
    let add = {
        let list = stored;
        move |_| {
            let mut l = list.get_value();
            l.push(ClassLevel {
                class: Link {
                    name: i18n.t("kind-class"),
                    source: None,
                    key: None,
                },
                subclass: None,
                level: 1,
                hit_die: 8,
                caster: Caster::None,
                spell_ability: None,
            });
            set(l);
        }
    };
    view! {
        <fieldset class="sheet-section">
            <legend>{move || i18n.t("sheet-classes")}</legend>
            {rows}
            <div><button type="button" on:click=add>{move || i18n.t("sheet-add-class")}</button></div>
        </fieldset>
    }
}

#[component]
fn Abilities(campaign: CampaignId, ch: Character) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let boxes = Ability::ALL
        .into_iter()
        .map(|a| {
            let (scores, saves) = (ch.abilities, ch.saves);
            let score = scores[a.index()];
            let proficient = saves[a.index()];
            view! {
                <div class="ability">
                    <span class="ability-name">{move || i18n.t(ABILITY_KEYS[a.index()])}</span>
                    <input type="number" min="1" max="30" prop:value=score.to_string()
                        aria-label=move || i18n.t(ABILITY_KEYS[a.index()])
                        on:change=on_num(move |v: u8| {
                            let mut s = scores;
                            s[a.index()] = v.clamp(1, 30);
                            send(campaign, CharacterOp::Change { id, change: CharacterChange::Abilities(s) });
                        })/>
                    <strong class="ability-mod">{signed(ch.modifier(a))}</strong>
                    <label class="choice small">
                        <input type="checkbox" prop:checked=proficient
                            on:change=move |ev| {
                                let mut s = saves;
                                s[a.index()] = event_target_checked(&ev);
                                send(campaign, CharacterOp::Change { id, change: CharacterChange::Saves(s) });
                            }/>
                        {move || i18n.t("sheet-save")}" "{signed(ch.save(a))}
                    </label>
                </div>
            }
        })
        .collect_view();
    view! {
        <fieldset class="sheet-section">
            <legend>{move || i18n.t("sheet-abilities")}</legend>
            <div class="abilities">{boxes}</div>
        </fieldset>
    }
}

#[component]
fn Skills(campaign: CampaignId, ch: Character) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let rows = Skill::ALL
        .into_iter()
        .map(|s| {
            let current = ch.skills[s.index()];
            view! {
                <div class="skill-row">
                    <select prop:value=format!("{current:?}") aria-label=move || i18n.t(skill_key(s))
                        on:change=move |ev| {
                            let p = match event_target_value(&ev).as_str() {
                                "Proficient" => Proficiency::Proficient,
                                "Expertise" => Proficiency::Expertise,
                                _ => Proficiency::None,
                            };
                            send(campaign, CharacterOp::Change { id, change: CharacterChange::Skill(s, p) });
                        }>
                        <option value="None">{move || i18n.t("prof-none")}</option>
                        <option value="Proficient">{move || i18n.t("prof-proficient")}</option>
                        <option value="Expertise">{move || i18n.t("prof-expertise")}</option>
                    </select>
                    <span class="grow">{move || i18n.t(skill_key(s))}
                        <span class="muted small">" "{move || i18n.t(ABILITY_KEYS[s.ability().index()])}</span>
                    </span>
                    <strong>{signed(ch.skill(s))}</strong>
                </div>
            }
        })
        .collect_view();
    let jack = ch.jack_of_all_trades;
    view! {
        <fieldset class="sheet-section">
            <legend>{move || i18n.t("sheet-skills")}</legend>
            <label class="choice small">
                <input type="checkbox" prop:checked=jack
                    on:change=move |ev| send(campaign, CharacterOp::Change {
                        id,
                        change: CharacterChange::JackOfAllTrades(event_target_checked(&ev)),
                    })/>
                {move || i18n.t("sheet-jack")}
            </label>
            <p class="muted small">
                {move || i18n.tr("sheet-passives", &[
                    ("perception", ch.passive(Skill::Perception).into()),
                    ("insight", ch.passive(Skill::Insight).into()),
                    ("investigation", ch.passive(Skill::Investigation).into()),
                ])}
            </p>
            <div class="skills">{rows}</div>
        </fieldset>
    }
}

fn armor_kind(f: ArmorFormula) -> &'static str {
    match f {
        ArmorFormula::Unarmored => "unarmored",
        ArmorFormula::Armor { .. } => "armor",
        ArmorFormula::UnarmoredDefense { .. } => "defense",
        ArmorFormula::Natural { .. } => "natural",
        ArmorFormula::Fixed { .. } => "fixed",
    }
}

#[component]
fn Combat(campaign: CampaignId, ch: Character) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let change = move |c: CharacterChange| send(campaign, CharacterOp::Change { id, change: c });
    let (formula, shield, bonus) = (ch.armor, ch.shield, ch.ac_bonus);
    let set_armor = move |f: ArmorFormula, s: bool, b: i8| {
        change(CharacterChange::Armor {
            formula: f,
            shield: s,
            bonus: b,
        })
    };
    let hp = ch.hp;
    let set_hp = move |f: fn(&mut HitPoints, i16), v: i16| {
        let mut h = hp;
        f(&mut h, v);
        change(CharacterChange::Hp(h));
    };
    let formula_inputs = match formula {
        ArmorFormula::Armor { base, dex_cap } => view! {
            <label class="filter"><span>{move || i18n.t("sheet-armor-base")}</span>
                <input type="number" min="0" max="30" prop:value=base.to_string()
                    on:change=on_num(move |v: u8| {
                        set_armor(ArmorFormula::Armor { base: v, dex_cap }, shield, bonus);
                    })/></label>
            <label class="filter"><span>{move || i18n.t("sheet-dex-cap")}</span>
                <input type="number" min="0" max="10" prop:value=dex_cap.map(|c| c.to_string()).unwrap_or_default()
                    placeholder=move || i18n.t("sheet-no-cap")
                    on:change=move |ev| {
                        let cap = num::<u8>(&ev);
                        set_armor(ArmorFormula::Armor { base, dex_cap: cap }, shield, bonus);
                    }/></label>
        }
        .into_any(),
        ArmorFormula::UnarmoredDefense { plus } => view! {
            <label class="filter"><span>{move || i18n.t("sheet-defense-plus")}</span>
                <select prop:value=format!("{plus:?}")
                    on:change=move |ev| if let Some(a) = ability_from(&event_target_value(&ev)) {
                        set_armor(ArmorFormula::UnarmoredDefense { plus: a }, shield, bonus);
                    }>
                    {Ability::ALL.into_iter().map(|a| view! {
                        <option value=format!("{a:?}")>{move || i18n.t(ABILITY_KEYS[a.index()])}</option>
                    }).collect_view()}
                </select></label>
        }
        .into_any(),
        ArmorFormula::Natural { base } => view! {
            <label class="filter"><span>{move || i18n.t("sheet-armor-base")}</span>
                <input type="number" min="0" max="30" prop:value=base.to_string()
                    on:change=on_num(move |v: u8| {
                        set_armor(ArmorFormula::Natural { base: v }, shield, bonus);
                    })/></label>
        }
        .into_any(),
        ArmorFormula::Fixed { ac } => view! {
            <label class="filter"><span>{move || i18n.t("sheet-ac")}</span>
                <input type="number" min="0" max="40" prop:value=ac.to_string()
                    on:change=on_num(move |v: u8| {
                        set_armor(ArmorFormula::Fixed { ac: v }, shield, bonus);
                    })/></label>
        }
        .into_any(),
        ArmorFormula::Unarmored => ().into_any(),
    };
    let average = ch.average_max_hp();
    let penalty = ch.d20_penalty();
    let total_dice = ch.level();
    let (saves_ok, saves_ko) = ch.death_saves;
    view! {
        <fieldset class="sheet-section">
            <legend>{move || i18n.t("sheet-combat")}</legend>
            <div class="stats">
                <div class="stat"><span class="muted small">{move || i18n.t("sheet-ac")}</span><strong>{ch.armor_class()}</strong></div>
                <div class="stat"><span class="muted small">{move || i18n.t("sheet-initiative")}</span><strong>{signed(ch.initiative())}</strong></div>
                <div class="stat"><span class="muted small">{move || i18n.t("sheet-speed")}</span><strong>{ch.speed}</strong></div>
                <div class="stat"><span class="muted small">{move || i18n.t("sheet-proficiency")}</span><strong>{signed(ch.proficiency_bonus())}</strong></div>
            </div>
            <div class="filters">
                <label class="filter"><span>{move || i18n.t("sheet-armor")}</span>
                    <select prop:value=armor_kind(formula)
                        on:change=move |ev| {
                            let f = match event_target_value(&ev).as_str() {
                                "armor" => ArmorFormula::Armor { base: 11, dex_cap: None },
                                "defense" => ArmorFormula::UnarmoredDefense { plus: Ability::Con },
                                "natural" => ArmorFormula::Natural { base: 13 },
                                "fixed" => ArmorFormula::Fixed { ac: 10 },
                                _ => ArmorFormula::Unarmored,
                            };
                            set_armor(f, shield, bonus);
                        }>
                        <option value="unarmored">{move || i18n.t("armor-unarmored")}</option>
                        <option value="armor">{move || i18n.t("armor-armor")}</option>
                        <option value="defense">{move || i18n.t("armor-defense")}</option>
                        <option value="natural">{move || i18n.t("armor-natural")}</option>
                        <option value="fixed">{move || i18n.t("armor-fixed")}</option>
                    </select></label>
                {formula_inputs}
                <label class="filter"><span>{move || i18n.t("sheet-ac-bonus")}</span>
                    <input type="number" min="-10" max="10" prop:value=bonus.to_string()
                        on:change=on_num(move |v: i8| { set_armor(formula, shield, v) })/></label>
                <label class="filter"><span>{move || i18n.t("sheet-initiative-bonus")}</span>
                    <input type="number" min="-10" max="20" prop:value=ch.initiative_bonus.to_string()
                        on:change=on_num(move |v: i8| { change(CharacterChange::InitiativeBonus(v)) })/></label>
                <label class="filter"><span>{move || i18n.t("sheet-speed")}</span>
                    <input type="number" min="0" max="1000" step="5" prop:value=ch.speed.to_string()
                        on:change=on_num(move |v: u16| { change(CharacterChange::Speed(v)) })/></label>
            </div>
            <label class="choice small">
                <input type="checkbox" prop:checked=shield
                    on:change=move |ev| set_armor(formula, event_target_checked(&ev), bonus)/>
                {move || i18n.t("sheet-shield")}
            </label>
            <div class="filters">
                <label class="filter"><span>{move || i18n.t("sheet-hp-current")}</span>
                    <input type="number" prop:value=hp.current.to_string()
                        on:change=on_num(move |v: i16| { set_hp(|h, v| h.current = v, v) })/></label>
                <label class="filter"><span>{move || i18n.t("sheet-hp-max")}</span>
                    <input type="number" prop:value=hp.max.to_string()
                        on:change=on_num(move |v: i16| { set_hp(|h, v| h.max = v, v) })/></label>
                <label class="filter"><span>{move || i18n.t("sheet-hp-temp")}</span>
                    <input type="number" prop:value=hp.temp.to_string()
                        on:change=on_num(move |v: i16| { set_hp(|h, v| h.temp = v, v) })/></label>
                <label class="filter"><span>{move || i18n.tr("sheet-hit-dice", &[("total", total_dice.into())])}</span>
                    <input type="number" min="0" max="20" prop:value=ch.hit_dice_spent.to_string()
                        on:change=on_num(move |v: u8| { change(CharacterChange::HitDiceSpent(v.min(20))) })/></label>
            </div>
            <div class="toolbar">
                <button type="button" class="ghost" on:click=move |_| set_hp(|h, v| h.max = v, average)>
                    {move || i18n.tr("sheet-use-average", &[("hp", average.into())])}
                </button>
            </div>
            <div class="filters">
                <label class="filter"><span>{move || i18n.t("sheet-death-ok")}</span>
                    <input type="number" min="0" max="3" prop:value=saves_ok.to_string()
                        on:change=on_num(move |v: u8| { change(CharacterChange::DeathSaves(v.min(3), saves_ko)) })/></label>
                <label class="filter"><span>{move || i18n.t("sheet-death-ko")}</span>
                    <input type="number" min="0" max="3" prop:value=saves_ko.to_string()
                        on:change=on_num(move |v: u8| { change(CharacterChange::DeathSaves(saves_ok, v.min(3))) })/></label>
                <label class="filter"><span>{move || i18n.t("sheet-exhaustion")}</span>
                    <input type="number" min="0" max="6" prop:value=ch.exhaustion.to_string()
                        on:change=on_num(move |v: u8| { change(CharacterChange::Exhaustion(v.min(6))) })/></label>
                <label class="filter"><span>{move || i18n.t("sheet-conditions")}</span>
                    <input type="text" prop:value=ch.conditions.join(", ")
                        on:change=move |ev| {
                            let list = event_target_value(&ev)
                                .split(',')
                                .map(|c| c.trim().to_owned())
                                .filter(|c| !c.is_empty())
                                .collect();
                            change(CharacterChange::Conditions(list));
                        }/></label>
            </div>
            {(penalty != 0).then(|| view! {
                <p class="muted small">{move || i18n.tr("sheet-penalty", &[("penalty", penalty.into())])}</p>
            })}
        </fieldset>
    }
}

#[component]
fn Counters(campaign: CampaignId, ch: Character) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let counters = ch.counters.clone();
    let set = move |list: Vec<Counter>| {
        send(
            campaign,
            CharacterOp::Change {
                id,
                change: CharacterChange::Counters(list),
            },
        )
    };
    let stored = StoredValue::new(counters.clone());
    let rows = counters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let list = stored;
            let edit = move |f: Box<dyn FnOnce(&mut Counter)>| {
                let mut l = list.get_value();
                if let Some(c) = l.get_mut(i) {
                    f(c);
                }
                set(l);
            };
            let remove = {
                let list = stored;
                move |_| {
                    let mut l = list.get_value();
                    l.remove(i);
                    set(l);
                }
            };
            view! {
                <div class="filters sheet-class">
                    <label class="filter"><span>{move || i18n.t("sheet-counter-name")}</span>
                        <input type="text" maxlength="120" prop:value=c.name.clone()
                            on:change=move |ev| { let v = event_target_value(&ev); edit(Box::new(move |c| c.name = v)) }/></label>
                    <label class="filter"><span>{move || i18n.t("sheet-counter-current")}</span>
                        <input type="number" min="0" prop:value=c.current.to_string()
                            on:change=on_num(move |v: u16| { edit(Box::new(move |c| c.current = v)) })/></label>
                    <label class="filter"><span>{move || i18n.t("sheet-counter-max")}</span>
                        <input type="number" min="0" prop:value=c.max.to_string()
                            on:change=on_num(move |v: u16| { edit(Box::new(move |c| c.max = v)) })/></label>
                    <button type="button" class="ghost" on:click=remove>{move || i18n.t("sheet-remove")}</button>
                </div>
            }
        })
        .collect_view();
    let add = {
        let list = stored;
        move |_| {
            let mut l = list.get_value();
            l.push(Counter {
                name: i18n.t("sheet-counter-new"),
                current: 0,
                max: 0,
            });
            set(l);
        }
    };
    view! {
        <fieldset class="sheet-section">
            <legend>{move || i18n.t("sheet-counters")}</legend>
            {rows}
            <div><button type="button" on:click=add>{move || i18n.t("sheet-add-counter")}</button></div>
        </fieldset>
    }
}

#[component]
fn Links(campaign: CampaignId, ch: Character) -> impl IntoView {
    let i18n = use_i18n();
    let id = ch.id;
    let section = move |legend: &'static str,
                        add_key: &'static str,
                        kind: Kind,
                        list: Vec<Link>,
                        wrap: fn(Vec<Link>) -> CharacterChange| {
        let set = move |l: Vec<Link>| {
            send(
                campaign,
                CharacterOp::Change {
                    id,
                    change: wrap(l),
                },
            )
        };
        let stored = StoredValue::new(list.clone());
        let rows = list
            .iter()
            .enumerate()
            .map(|(i, link)| {
                let all = stored;
                let on_set = Callback::new(move |l: Option<Link>| {
                    let mut next = all.get_value();
                    match l {
                        Some(l) => next[i] = l,
                        None => {
                            next.remove(i);
                        }
                    }
                    set(next);
                });
                view! {
                    <LinkInput campaign=campaign kind=Some(kind) value=Some(link.clone()) label=legend on_set=on_set/>
                }
            })
            .collect_view();
        let add = {
            move |_| {
                let mut next = stored.get_value();
                next.push(Link {
                    name: i18n.t(legend),
                    source: None,
                    key: None,
                });
                set(next);
            }
        };
        view! {
            <fieldset class="sheet-section">
                <legend>{move || i18n.t(legend)}</legend>
                <div class="filters">{rows}</div>
                <div><button type="button" on:click=add>{move || i18n.t(add_key)}</button></div>
            </fieldset>
        }
    };
    view! {
        {section("sheet-feats", "sheet-add-feat", Kind::Feat, ch.feats.clone(), CharacterChange::Feats)}
        {section("sheet-features", "sheet-add-feature", Kind::Feature, ch.features.clone(), CharacterChange::Features)}
    }
}

pub fn summary_line(
    i18n: I18n,
    name: &str,
    classes: &[(String, u8)],
    ac: i16,
    hp: HitPoints,
) -> String {
    i18n.tr(
        "party-character",
        &[
            ("name", name.to_owned().into()),
            ("classes", class_line(classes).into()),
            ("ac", ac.into()),
            ("hp", hp.current.into()),
            ("max", hp.max.into()),
        ],
    )
}
