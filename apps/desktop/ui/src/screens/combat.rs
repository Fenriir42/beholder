use beholder_core::actions::{Details, MonsterStats};
use beholder_core::character::HitPoints;
use beholder_core::combat::{Combatant, Condition, Encounter, EncounterOp, Health, Source};
use beholder_core::compendium::{Hit, Kind, SearchQuery};
use beholder_core::dice::{DiceOp, Expr, Visibility};
use beholder_core::{CampaignId, CombatVisibility, CombatantId, CommandId, EncounterId, Role};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use serde::de::DeserializeOwned;

use crate::app::{AppCtx, error_key, use_app};
use crate::i18n::use_i18n;
use crate::screens::acting::ActingPanel;
use crate::screens::battle_map::{BattleMap, MapCtx};
use crate::screens::dice_view::{RollToast, Roller};
use crate::{icons, ipc};

#[derive(serde::Serialize)]
struct OpArgs {
    campaign_id: String,
    op: EncounterOp,
}

#[derive(serde::Serialize)]
struct CountArgs {
    count: usize,
}

#[derive(serde::Serialize)]
struct RollArgs {
    campaign_id: String,
    op: DiceOp,
}

#[derive(serde::Serialize)]
struct SearchArgs {
    query: SearchQuery,
}

#[derive(serde::Serialize)]
struct DetailsArgs {
    refs: Vec<(String, String)>,
}

const CONDITIONS: [&str; 15] = [
    "condition-blinded",
    "condition-charmed",
    "condition-deafened",
    "condition-exhaustion",
    "condition-frightened",
    "condition-grappled",
    "condition-incapacitated",
    "condition-invisible",
    "condition-paralyzed",
    "condition-petrified",
    "condition-poisoned",
    "condition-prone",
    "condition-restrained",
    "condition-stunned",
    "condition-unconscious",
];

pub(crate) fn send(campaign: CampaignId, op: EncounterOp) {
    let ctx = use_app();
    spawn_local_scoped(async move {
        let args = OpArgs {
            campaign_id: campaign.to_string(),
            op,
        };
        if let Err(e) = ipc::call::<()>("encounter_command", args).await {
            ctx.notify(error_key(e));
        }
    });
}

pub(crate) async fn fresh<T: DeserializeOwned>(count: usize) -> Vec<T> {
    ipc::call("new_ids", CountArgs { count })
        .await
        .unwrap_or_default()
}

async fn monster_stats(source: String, key: String) -> MonsterStats {
    let args = DetailsArgs {
        refs: vec![(source, key)],
    };
    ipc::call::<Vec<Option<Details>>>("sheet_details", args)
        .await
        .ok()
        .and_then(|d| d.into_iter().next().flatten())
        .and_then(|d| d.monster)
        .unwrap_or_default()
}

fn health_key(h: Health) -> &'static str {
    match h {
        Health::Healthy => "combat-health-healthy",
        Health::Bloodied => "combat-health-bloodied",
        Health::NearDeath => "combat-health-near-death",
        Health::Down => "combat-health-down",
    }
}

fn parse_num(ev: &leptos::ev::Event) -> Option<i16> {
    event_target_value(ev).trim().parse().ok()
}

#[derive(Clone, Copy)]
struct Tracker {
    campaign: CampaignId,
    encounter: EncounterId,
    is_dm: Memo<bool>,
    pending: RwSignal<Vec<(CommandId, EncounterId, CombatantId)>>,
    acting: RwSignal<Option<CombatantId>>,
    follow: RwSignal<bool>,
}

impl Tracker {
    fn send(&self, op: EncounterOp) {
        send(self.campaign, op);
    }

    fn own(&self, ctx: AppCtx, c: &Combatant) -> bool {
        let Source::Character(ch) = c.source else {
            return false;
        };
        ctx.view.with_untracked(|v| {
            let Some(me) = v.user.as_ref().map(|u| u.id) else {
                return false;
            };
            v.campaigns
                .iter()
                .find(|x| x.campaign.id == self.campaign)
                .is_some_and(|x| {
                    x.campaign
                        .characters
                        .iter()
                        .any(|k| k.id == ch && k.owner == me)
                        || x.campaign.party.iter().any(|k| k.id == ch && k.owner == me)
                })
        })
    }

    fn roll_initiative(&self, c: Combatant, visibility: Visibility) {
        let ctx = use_app();
        let i18n = use_i18n();
        let t = *self;
        let character_bonus = match c.source {
            Source::Character(ch) => ctx.view.with_untracked(|v| {
                v.campaigns
                    .iter()
                    .find(|x| x.campaign.id == t.campaign)
                    .and_then(|x| x.campaign.characters.iter().find(|k| k.id == ch))
                    .map(|k| i32::from(k.initiative()))
            }),
            _ => None,
        };
        spawn_local_scoped(async move {
            let bonus = match (&c.source, character_bonus) {
                (_, Some(b)) => b,
                (Source::Monster { source, key }, None) => {
                    monster_stats(source.clone(), key.clone())
                        .await
                        .initiative
                        .map_or(0, i32::from)
                }
                _ => 0,
            };
            let op = DiceOp::Roll {
                expr: Expr::d20(bonus),
                context: i18n.tr("combat-initiative-of", &[("name", c.name.clone().into())]),
                character: Some(c.name.clone()),
                visibility,
            };
            let args = RollArgs {
                campaign_id: t.campaign.to_string(),
                op,
            };
            match ipc::call::<CommandId>("roll_dice", args).await {
                Ok(cmd) => t.pending.update(|p| p.push((cmd, t.encounter, c.id))),
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    }
}

#[component]
pub fn Combat(id: CampaignId) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let is_dm = Memo::new(move |_| {
        ctx.view.with(|v| {
            let me = v.user.as_ref()?.id;
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == id)?
                .campaign
                .role_of(me)
        }) == Some(Role::Dm)
    });
    let encounters = Memo::new(move |_| {
        ctx.view.with(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == id)
                .map(|c| {
                    c.campaign
                        .encounters
                        .iter()
                        .map(|e| (e.id, e.name.clone(), e.active))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    let me = Memo::new(move |_| ctx.view.with(|v| v.user.as_ref().map(|u| u.id)));
    let roller = Roller::new(id);
    Effect::new(move |_| {
        if is_dm.get() {
            roller.visibility.set(Visibility::ToDm);
        }
    });
    let acting = RwSignal::new(None::<CombatantId>);
    let follow = RwSignal::new(false);
    let selected = RwSignal::new(None::<EncounterId>);
    let current = Memo::new(move |_| {
        encounters.with(|list| {
            selected
                .get()
                .filter(|s| list.iter().any(|e| e.0 == *s))
                .or_else(|| list.iter().find(|e| e.2).map(|e| e.0))
                .or_else(|| list.first().map(|e| e.0))
        })
    });
    let pending = RwSignal::new(Vec::<(CommandId, EncounterId, CombatantId)>::new());
    Effect::new(move |_| {
        if pending.with(Vec::is_empty) {
            return;
        }
        let waiting = pending.get();
        let done: Vec<(CommandId, EncounterId, CombatantId, i32)> = ctx.view.with(|v| {
            let Some(c) = v.campaigns.iter().find(|c| c.campaign.id == id) else {
                return Vec::new();
            };
            waiting
                .iter()
                .filter_map(|(cmd, e, k)| {
                    let r = c.campaign.rolls.iter().find(|r| r.id == *cmd)?;
                    Some((*cmd, *e, *k, r.outcome.total))
                })
                .collect()
        });
        if done.is_empty() {
            return;
        }
        pending.update(|p| p.retain(|(cmd, ..)| !done.iter().any(|d| d.0 == *cmd)));
        for (_, e, k, total) in done {
            send(
                id,
                EncounterOp::SetInitiative {
                    id: e,
                    combatant: k,
                    initiative: Some(total.clamp(-999, 999) as i16),
                },
            );
        }
    });

    let new_name = RwSignal::new(String::new());
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let name = new_name.get_untracked();
        spawn_local_scoped(async move {
            if let Some(eid) = fresh::<EncounterId>(1).await.into_iter().next() {
                send(id, EncounterOp::Create { id: eid, name });
                new_name.set(String::new());
                selected.set(Some(eid));
            }
        });
    };
    let picker = move || {
        let options = encounters
            .get()
            .into_iter()
            .map(|(eid, name, active)| {
                let label = if active {
                    format!("{name} · {}", i18n.t("combat-running"))
                } else {
                    name
                };
                view! { <option value=eid.to_string() selected=current.get() == Some(eid)>{label}</option> }
            })
            .collect_view();
        view! {
            <label class="filter"><span>{move || i18n.t("combat-encounter")}</span>
                <select on:change=move |ev| selected.set(event_target_value(&ev).parse().ok().map(EncounterId))>
                    {options}
                </select>
            </label>
        }
    };

    view! {
        <section class="home combat">
            <h1>{move || i18n.t("nav-combat")}</h1>
            {move || is_dm.get().then(|| view! {
                <div class="toolbar">
                    {move || (!encounters.with(Vec::is_empty)).then(picker)}
                    <form class="toolbar" on:submit=create>
                        <input type="text" required maxlength="80" bind:value=new_name
                            placeholder=move || i18n.t("combat-new-name") aria-label=move || i18n.t("combat-new-name")/>
                        <button type="submit">{icons::plus()}{move || i18n.t("combat-new")}</button>
                    </form>
                </div>
            })}
            {move || match current.get() {
                Some(eid) => {
                    let t = Tracker { campaign: id, encounter: eid, is_dm, pending, acting, follow };
                    let encounter = Memo::new(move |_| ctx.view.with(|v| {
                        v.campaigns
                            .iter()
                            .find(|c| c.campaign.id == id)
                            .and_then(|c| c.campaign.encounters.iter().find(|e| e.id == eid))
                            .cloned()
                    }));
                    let m = MapCtx {
                        campaign: id,
                        encounter: encounter.into(),
                        is_dm: is_dm.into(),
                        me: me.into(),
                        interactive: true,
                        roller,
                        acting: Some(acting),
                    };
                    let actor = Memo::new(move |_| {
                        let cid = acting.get()?;
                        encounter.with(|e| e.as_ref()?.combatants.iter().find(|c| c.id == cid).cloned())
                    });
                    let close = Callback::new(move |()| { acting.set(None); follow.set(false); });
                    view! {
                        <RollToast roller=roller/>
                        <div class="combat-layout">
                            <div class="tracker-col"><TrackerView t=t/></div>
                            <BattleMap m=m/>
                        </div>
                        {move || actor.with(Option::is_some).then(|| view! { <ActingPanel campaign=id combatant=actor on_close=close/> })}
                    }.into_any()
                }
                None => view! {
                    <p class="empty">{move || if is_dm.get() { i18n.t("combat-none-dm") } else { i18n.t("combat-none") }}</p>
                }.into_any(),
            }}
        </section>
    }
}

#[component]
fn TrackerView(t: Tracker) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let encounter = Memo::new(move |_| {
        ctx.view.with(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == t.campaign)
                .and_then(|c| c.campaign.encounters.iter().find(|e| e.id == t.encounter))
                .map(|e| Encounter {
                    shapes: Vec::new(),
                    ..e.clone()
                })
        })
    });
    let visibility = Memo::new(move |_| {
        ctx.view.with(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == t.campaign)
                .map(|c| c.campaign.settings.combat_visibility)
        })
    });
    let combatants = Memo::new(move |_| {
        encounter.with(|e| e.as_ref().map(|e| e.combatants.clone()).unwrap_or_default())
    });
    let turn = Memo::new(move |_| encounter.with(|e| e.as_ref().and_then(|e| e.turn)));
    Effect::new(move |_| {
        if let Some(cid) = turn.get()
            && t.follow.get()
        {
            t.acting.set(Some(cid));
        }
    });
    let round = Memo::new(move |_| encounter.with(|e| e.as_ref().map_or(0, |e| e.round)));
    let eid = t.encounter;

    let header = move || {
        let e = encounter.get()?;
        let active = e.active;
        let confirm = RwSignal::new(false);
        let rename = move |ev: leptos::ev::Event| {
            let name = event_target_value(&ev);
            if !name.trim().is_empty() {
                t.send(EncounterOp::Rename { id: eid, name });
            }
        };
        Some(view! {
            <div class="toolbar">
                <input type="text" class="grow" maxlength="80" prop:value=e.name.clone() on:change=rename
                    aria-label=move || i18n.t("combat-rename")/>
                {active.then(|| view! { <span class="badge">{move || i18n.t("combat-running")}</span> })}
                <button type="button" class:primary=!active
                    on:click=move |_| t.send(EncounterOp::Activate { id: eid, active: !active })>
                    {move || if active { i18n.t("combat-stop") } else { i18n.t("combat-run") }}
                </button>
                {move || if confirm.get() {
                    view! {
                        <button type="button" class="danger" on:click=move |_| t.send(EncounterOp::Delete { id: eid })>
                            {icons::trash()}{move || i18n.t("combat-delete-confirm")}
                        </button>
                        <button type="button" class="ghost" on:click=move |_| confirm.set(false)>{move || i18n.t("cancel")}</button>
                    }.into_any()
                } else {
                    view! {
                        <button type="button" class="ghost" on:click=move |_| confirm.set(true)>
                            {icons::trash()}{move || i18n.t("combat-delete")}
                        </button>
                    }.into_any()
                }}
            </div>
        })
    };

    let roll_others = move |_| {
        for c in combatants.get_untracked() {
            if c.initiative.is_none() && !matches!(c.source, Source::Character(_)) {
                t.roll_initiative(c, Visibility::ToDm);
            }
        }
    };
    let round_bar = move || {
        let r = round.get();
        let dm = t.is_dm.get();
        view! {
            <div class="toolbar round-bar">
                <strong class="grow">
                    {move || if r == 0 { i18n.t("combat-not-started") } else { i18n.tr("combat-round", &[("n", r.into())]) }}
                </strong>
                {dm.then(|| view! {
                    <label class="choice small">
                        <input type="checkbox" prop:checked=move || t.follow.get()
                            on:change=move |ev| t.follow.set(event_target_checked(&ev))/>
                        {move || i18n.t("acting-follow")}
                    </label>
                    <button type="button" class="ghost" on:click=roll_others>{icons::dice()}{move || i18n.t("combat-roll-others")}</button>
                    <button type="button" class="ghost" on:click=move |_| t.send(EncounterOp::ResetCombat { id: eid })>{move || i18n.t("combat-reset")}</button>
                    <button type="button" disabled=r == 0 on:click=move |_| t.send(EncounterOp::PreviousTurn { id: eid })>{move || i18n.t("combat-previous")}</button>
                    <button type="button" class="primary" on:click=move |_| t.send(EncounterOp::NextTurn { id: eid })>
                        {move || if r == 0 { i18n.t("combat-start") } else { i18n.t("combat-next") }}
                    </button>
                })}
            </div>
        }
    };

    let hidden_tracker = move || {
        !t.is_dm.get()
            && visibility.get() == Some(CombatVisibility::Hidden)
            && combatants.with(Vec::is_empty)
    };

    view! {
        {move || t.is_dm.get().then(header)}
        {move || if hidden_tracker() {
            view! { <p class="muted">{move || i18n.t("combat-hidden-tracker")}</p> }.into_any()
        } else {
            view! {
                {round_bar}
                <ul class="list plain tracker" aria-label=move || i18n.t("combat-order")>
                    <For each=move || combatants.get() key=|c| c.id let(c)>
                        <CombatantRow t=t cid=c.id combatants=combatants turn=turn/>
                    </For>
                </ul>
            }.into_any()
        }}
        {move || t.is_dm.get().then(|| view! { <AddCombatants t=t combatants=combatants/> })}
    }
}

#[component]
fn CombatantRow(
    t: Tracker,
    cid: CombatantId,
    combatants: Memo<Vec<Combatant>>,
    turn: Memo<Option<CombatantId>>,
) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let eid = t.encounter;
    let combatant =
        Memo::new(move |_| combatants.with(|l| l.iter().find(|c| c.id == cid).cloned()));
    let current = move || turn.get() == Some(cid);
    let amount = RwSignal::new(String::new());
    let put = move |f: &dyn Fn(&mut Combatant)| {
        if let Some(mut c) = combatant.get_untracked() {
            f(&mut c);
            t.send(EncounterOp::PutCombatant {
                id: eid,
                combatant: c,
            });
        }
    };
    let hit = move |sign: i16| {
        if let Ok(n) = amount.get_untracked().trim().parse::<i16>() {
            amount.set(String::new());
            t.send(EncounterOp::Damage {
                id: eid,
                combatant: cid,
                amount: n.abs() * sign,
            });
        }
    };

    let initiative = move || {
        let c = combatant.get()?;
        let editable = t.is_dm.get() || t.own(ctx, &c);
        let value = c.initiative.map(|i| i.to_string()).unwrap_or_default();
        Some(if editable {
            let visibility = if t.is_dm.get() && !matches!(c.source, Source::Character(_)) {
                Visibility::ToDm
            } else {
                Visibility::Public
            };
            let stored = StoredValue::new(c);
            view! {
                <span class="init-cell">
                    <input type="number" class="init" min="-99" max="99" prop:value=value
                        aria-label=move || i18n.t("combat-initiative")
                        on:change=move |ev| t.send(EncounterOp::SetInitiative { id: eid, combatant: cid, initiative: parse_num(&ev) })/>
                    <button type="button" class="ghost icon-button" title=move || i18n.t("combat-roll-initiative")
                        aria-label=move || i18n.t("combat-roll-initiative")
                        on:click=move |_| t.roll_initiative(stored.get_value(), visibility)>
                        {icons::dice()}
                    </button>
                </span>
            }
            .into_any()
        } else {
            view! { <strong class="init-cell">{if value.is_empty() { "—".to_owned() } else { value }}</strong> }.into_any()
        })
    };

    let conditions = move || {
        let c = combatant.get()?;
        let dm = t.is_dm.get();
        let chips = c
            .conditions
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let label = match k.rounds {
                    Some(r) => format!("{} ({r})", k.name),
                    None => k.name.clone(),
                };
                if dm {
                    let name = k.name.clone();
                    view! {
                        <button type="button" class="chip private"
                            aria-label=move || i18n.tr("combat-remove-condition", &[("name", name.clone().into())])
                            on:click=move |_| put(&|c: &mut Combatant| { c.conditions.remove(i); })>
                            {label}" ✕"
                        </button>
                    }
                    .into_any()
                } else {
                    view! { <span class="chip private">{label}</span> }.into_any()
                }
            })
            .collect_view();
        Some(view! {
            <span class="chips">
                {c.hidden.then(|| view! { <span class="chip dm">{icons::eye()}{move || i18n.t("combat-hidden-badge")}</span> })}
                {c.concentration.then(|| view! { <span class="chip shared">{move || i18n.t("combat-concentration")}</span> })}
                {chips}
            </span>
        })
    };

    let hp = move || {
        let c = combatant.get()?;
        if let Some(h) = c.health {
            return Some(
                view! { <span class="health">{move || i18n.t(health_key(h))}</span> }.into_any(),
            );
        }
        let HitPoints { current, max, temp } = c.hp;
        let down = current <= 0;
        let text = if temp > 0 {
            format!("{current}/{max} (+{temp})")
        } else {
            format!("{current}/{max}")
        };
        Some(view! {
            <span class="hp-cell">
                <span class="muted small">{move || i18n.t("combat-hp")}</span>" "
                <strong>{text}</strong>
                {down.then(|| view! { <span class="small">" · "{move || i18n.t("combat-health-down")}</span> })}
            </span>
        }
        .into_any())
    };

    let dm_controls = move || {
        t.is_dm.get().then(|| {
            let condition = RwSignal::new(String::new());
            let rounds = RwSignal::new(String::new());
            let add_condition = move |ev: leptos::ev::SubmitEvent| {
                ev.prevent_default();
                let name = condition.get_untracked().trim().to_owned();
                if name.is_empty() {
                    return;
                }
                let r = rounds.get_untracked().trim().parse::<u8>().ok().filter(|r| *r > 0);
                condition.set(String::new());
                rounds.set(String::new());
                put(&|c: &mut Combatant| c.conditions.push(Condition { name: name.clone(), rounds: r }));
            };
            let list_id = format!("conditions-{cid}");
            let options = CONDITIONS
                .iter()
                .map(|k| view! { <option value=move || i18n.t(k)></option> })
                .collect_view();
            let field = move |f: fn(&Combatant) -> String| {
                move || combatant.with(|c| c.as_ref().map(f).unwrap_or_default())
            };
            let flag = move |f: fn(&Combatant) -> bool| {
                move || combatant.with(|c| c.as_ref().is_some_and(f))
            };
            view! {
                <span class="hp-controls">
                    <input type="number" class="hp-amount" min="0" bind:value=amount
                        aria-label=move || i18n.t("combat-amount")/>
                    <button type="button" on:click=move |_| hit(1)>{move || i18n.t("combat-damage")}</button>
                    <button type="button" on:click=move |_| hit(-1)>{move || i18n.t("combat-heal")}</button>
                </span>
                <details class="combatant-edit">
                    <summary>{icons::pencil()}{move || i18n.t("combat-edit")}</summary>
                    <form class="toolbar" on:submit=add_condition>
                        <input type="text" maxlength="80" list=list_id.clone() bind:value=condition
                            placeholder=move || i18n.t("combat-condition") aria-label=move || i18n.t("combat-condition")/>
                        <datalist id=list_id>{options}</datalist>
                        <input type="number" class="hp-amount" min="1" max="255" bind:value=rounds
                            placeholder=move || i18n.t("combat-rounds") aria-label=move || i18n.t("combat-rounds")/>
                        <button type="submit">{move || i18n.t("combat-add-condition")}</button>
                    </form>
                    <div class="toolbar">
                        <label class="choice small">
                            <input type="checkbox" prop:checked=flag(|c| c.concentration)
                                on:change=move |ev| { let v = event_target_checked(&ev); put(&|c: &mut Combatant| c.concentration = v) }/>
                            {move || i18n.t("combat-concentration")}
                        </label>
                        <label class="choice small">
                            <input type="checkbox" prop:checked=flag(|c| c.hidden)
                                on:change=move |ev| { let v = event_target_checked(&ev); put(&|c: &mut Combatant| c.hidden = v) }/>
                            {move || i18n.t("combat-hide")}
                        </label>
                        <label class="filter"><span>{move || i18n.t("combat-max-hp")}</span>
                            <input type="number" class="hp-amount" min="1" prop:value=field(|c| c.hp.max.to_string())
                                on:change=move |ev| if let Some(v) = parse_num(&ev) { put(&|c: &mut Combatant| c.hp.max = v.max(1)) }/>
                        </label>
                        <label class="filter"><span>{move || i18n.t("combat-ac")}</span>
                            <input type="number" class="hp-amount" min="0" prop:value=field(|c| c.ac.map(|a| a.to_string()).unwrap_or_default())
                                on:change=move |ev| { let v = parse_num(&ev); put(&|c: &mut Combatant| c.ac = v) }/>
                        </label>
                        <button type="button" class="ghost"
                            on:click=move |_| t.send(EncounterOp::RemoveCombatant { id: eid, combatant: cid })>
                            {icons::trash()}{move || i18n.t("combat-remove")}
                        </button>
                    </div>
                </details>
            }
        })
    };

    view! {
        <li class="row combatant" class:current=current aria-current=move || current().then_some("true")>
            <span class="turn-mark" aria-hidden="true">{move || if current() { "▶" } else { "" }}</span>
            {initiative}
            <span class="grow combatant-name">
                {move || {
                    let c = combatant.get()?;
                    let name = c.name.clone();
                    Some(if t.is_dm.get() || t.own(ctx, &c) {
                        view! {
                            <button type="button" class="link-row" aria-pressed=move || (t.acting.get() == Some(cid)).to_string()
                                title=move || i18n.t("acting-open") on:click=move |_| t.acting.set(Some(cid))>
                                <strong>{name.clone()}</strong>
                            </button>
                        }.into_any()
                    } else {
                        view! { <strong>{name}</strong> }.into_any()
                    })
                }}
                {move || current().then(|| view! { <span class="muted small">" · "{move || i18n.t("combat-turn")}</span> })}
                {conditions}
            </span>
            {move || combatant.get().and_then(|c| c.ac).map(|ac| view! {
                <span class="stat inline"><span class="muted small">{move || i18n.t("combat-ac")}</span><strong>{ac}</strong></span>
            })}
            {hp}
            {dm_controls}
        </li>
    }
}

#[component]
fn AddCombatants(t: Tracker, combatants: Memo<Vec<Combatant>>) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let eid = t.encounter;
    let add = move |make: Vec<Box<dyn FnOnce(CombatantId) -> Combatant>>| {
        spawn_local_scoped(async move {
            let ids = fresh::<CombatantId>(make.len()).await;
            for (id, f) in ids.into_iter().zip(make) {
                t.send(EncounterOp::PutCombatant {
                    id: eid,
                    combatant: f(id),
                });
            }
        });
    };
    let blank =
        |id: CombatantId, name: String, source: Source, hp: HitPoints, ac: Option<i16>| Combatant {
            id,
            name,
            source,
            initiative: None,
            hp,
            ac,
            conditions: Vec::new(),
            concentration: false,
            hidden: false,
            health: None,
        };

    let add_party = move |_| {
        let present: Vec<_> = combatants.with_untracked(|l| {
            l.iter()
                .filter_map(|c| match c.source {
                    Source::Character(ch) => Some(ch),
                    _ => None,
                })
                .collect()
        });
        let party: Vec<Box<dyn FnOnce(CombatantId) -> Combatant>> = ctx.view.with_untracked(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == t.campaign)
                .map(|c| {
                    c.campaign
                        .characters
                        .iter()
                        .filter(|k| k.active && !present.contains(&k.id))
                        .map(|k| {
                            let (name, ch, hp, ac) = (k.name.clone(), k.id, k.hp, k.armor_class());
                            Box::new(move |id| blank(id, name, Source::Character(ch), hp, Some(ac)))
                                as Box<dyn FnOnce(CombatantId) -> Combatant>
                        })
                        .collect()
                })
                .unwrap_or_default()
        });
        add(party);
    };

    let text = RwSignal::new(String::new());
    let hits = RwSignal::new(Vec::<Hit>::new());
    let search = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let (sources, hidden) = ctx.view.with_untracked(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == t.campaign)
                .map(|c| {
                    (
                        c.campaign.sources.clone(),
                        c.campaign.hidden.iter().map(|h| h.to_string()).collect(),
                    )
                })
                .unwrap_or_default()
        });
        let query = SearchQuery {
            text: text.get_untracked(),
            kind: Some(Kind::Monster),
            sources,
            hidden,
            ..SearchQuery::default()
        };
        spawn_local_scoped(async move {
            match ipc::call::<Vec<Hit>>("compendium_search", SearchArgs { query }).await {
                Ok(h) => {
                    hits.try_set(h.into_iter().take(20).collect());
                }
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    };
    let add_monster = move |h: Hit| {
        let same = combatants.with_untracked(|l| {
            l.iter()
                .filter(|c| matches!(&c.source, Source::Monster { key, .. } if *key == h.key))
                .count()
        });
        let name = if same == 0 {
            h.name.clone()
        } else {
            format!("{} {}", h.name, same + 1)
        };
        spawn_local_scoped(async move {
            let stats = monster_stats(h.source.clone(), h.key.clone()).await;
            let max = stats.hp.unwrap_or(1).max(1);
            let source = Source::Monster {
                source: h.source,
                key: h.key,
            };
            let hp = HitPoints {
                current: max,
                max,
                temp: 0,
            };
            add(vec![Box::new(move |id| {
                blank(id, name, source, hp, stats.ac)
            })]);
        });
    };
    let results = move || {
        hits.get()
            .into_iter()
            .map(|h| {
                let label = h.name.clone();
                let stored = StoredValue::new(h);
                view! {
                    <li class="row">
                        <span class="grow">{label}</span>
                        <button type="button" on:click=move |_| add_monster(stored.get_value())>
                            {icons::plus()}{move || i18n.t("combat-add-monster")}
                        </button>
                    </li>
                }
            })
            .collect_view()
    };

    let quick_name = RwSignal::new(String::new());
    let quick_hp = RwSignal::new(String::new());
    let quick_ac = RwSignal::new(String::new());
    let quick = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let name = quick_name.get_untracked().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let max = quick_hp
            .get_untracked()
            .trim()
            .parse::<i16>()
            .unwrap_or(1)
            .max(1);
        let ac = quick_ac.get_untracked().trim().parse::<i16>().ok();
        quick_name.set(String::new());
        let hp = HitPoints {
            current: max,
            max,
            temp: 0,
        };
        add(vec![Box::new(move |id| {
            blank(id, name, Source::Custom, hp, ac)
        })]);
    };

    view! {
        <details class="add-combatants" open=move || combatants.with(Vec::is_empty)>
            <summary><h2>{move || i18n.t("combat-add")}</h2></summary>
            <div class="toolbar">
                <button type="button" on:click=add_party>{icons::users()}{move || i18n.t("combat-add-party")}</button>
            </div>
            <form class="toolbar" on:submit=search>
                <input type="search" class="grow" bind:value=text
                    placeholder=move || i18n.t("combat-search-monsters") aria-label=move || i18n.t("combat-search-monsters")/>
                <button type="submit">{move || i18n.t("combat-search")}</button>
            </form>
            <ul class="list plain">{results}</ul>
            <form class="toolbar" on:submit=quick>
                <input type="text" class="grow" required maxlength="80" bind:value=quick_name
                    placeholder=move || i18n.t("combat-quick-name") aria-label=move || i18n.t("combat-quick-name")/>
                <input type="number" class="hp-amount" min="1" bind:value=quick_hp
                    placeholder=move || i18n.t("combat-hp") aria-label=move || i18n.t("combat-max-hp")/>
                <input type="number" class="hp-amount" min="0" bind:value=quick_ac
                    placeholder=move || i18n.t("combat-ac") aria-label=move || i18n.t("combat-ac")/>
                <button type="submit">{icons::plus()}{move || i18n.t("combat-quick-add")}</button>
            </form>
        </details>
    }
}
