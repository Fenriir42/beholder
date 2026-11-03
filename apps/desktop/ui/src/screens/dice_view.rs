use beholder_core::dice::{Advantage, DiceOp, Expr, Keep, RollRecord, Visibility};
use beholder_core::{CampaignId, CharacterId, CombatantId, CommandId, UserId};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::{error_key, use_app};
use crate::i18n::{I18n, use_i18n};
use crate::ipc;
use crate::screens::combat::{ApplyRoll, Economy, mark_character, target_names};

#[derive(serde::Serialize)]
struct RollArgs {
    campaign_id: String,
    op: DiceOp,
}

pub type FollowUp = (Expr, String, Vec<CombatantId>);

#[derive(Clone, Copy)]
pub struct Roller {
    pub campaign: CampaignId,
    pub character: RwSignal<Option<String>>,
    pub character_id: RwSignal<Option<CharacterId>>,
    pub advantage: RwSignal<Advantage>,
    pub visibility: RwSignal<Visibility>,
    pub last: RwSignal<Option<(String, Option<FollowUp>)>>,
}

impl Roller {
    pub fn new(campaign: CampaignId) -> Self {
        Self {
            campaign,
            character: RwSignal::new(None),
            character_id: RwSignal::new(None),
            advantage: RwSignal::new(Advantage::Normal),
            visibility: RwSignal::new(Visibility::Public),
            last: RwSignal::new(None),
        }
    }

    pub fn roll(&self, expr: Expr, context: String, crit: Option<FollowUp>) {
        self.roll_at(expr, context, crit, Vec::new());
    }

    pub fn roll_at(
        &self,
        expr: Expr,
        context: String,
        crit: Option<FollowUp>,
        targets: Vec<CombatantId>,
    ) {
        let ctx = use_app();
        let this = *self;
        let names = target_names(self.campaign, &targets);
        let context = if names.is_empty() {
            context
        } else {
            format!("{context} → {names}").chars().take(120).collect()
        };
        let op = DiceOp::Roll {
            expr,
            context,
            character: self.character.get_untracked(),
            visibility: self.visibility.get_untracked(),
            targets,
        };
        spawn_local(async move {
            let args = RollArgs {
                campaign_id: this.campaign.to_string(),
                op,
            };
            match ipc::call::<String>("roll_dice", args).await {
                Ok(id) => this.last.set(Some((id, crit))),
                Err(e) => ctx.notify(error_key(e)),
            }
        });
    }

    pub fn test(&self, modifier: i32, context: String, crit: Option<FollowUp>) {
        self.test_at(modifier, context, crit, Vec::new());
    }

    pub fn test_at(
        &self,
        modifier: i32,
        context: String,
        crit: Option<FollowUp>,
        targets: Vec<CombatantId>,
    ) {
        let advantage = self.advantage.get_untracked();
        self.roll_at(
            Expr::d20(modifier).with_advantage(advantage),
            context,
            crit,
            targets,
        );
    }

    pub fn mark(&self, which: Economy) {
        if let Some(ch) = self.character_id.get_untracked() {
            mark_character(self.campaign, ch, which);
        }
    }
}

pub fn dice_detail(r: &RollRecord) -> String {
    let mut parts = Vec::new();
    for t in &r.outcome.terms {
        let dice: Vec<String> = t
            .rolls
            .iter()
            .zip(&t.kept)
            .map(|(v, k)| if *k { v.to_string() } else { format!("({v})") })
            .collect();
        let sign = if t.term.negative { "−" } else { "" };
        parts.push(format!("{sign}[{}]", dice.join(" ")));
    }
    match r.outcome.modifier {
        0 => {}
        m if m > 0 => parts.push(format!("+{m}")),
        m => parts.push(m.to_string()),
    }
    parts.join(" ")
}

fn natural_key(r: &RollRecord) -> Option<&'static str> {
    let first = r.expr.terms.first()?;
    let single =
        first.sides == 20 && matches!(first.keep, Keep::All | Keep::Highest(1) | Keep::Lowest(1));
    match r.outcome.natural() {
        Some(20) if single => Some("dice-natural-20"),
        Some(1) if single => Some("dice-natural-1"),
        _ => None,
    }
}

fn member_name(campaign: CampaignId, user: UserId) -> String {
    let ctx = use_app();
    ctx.view.with_untracked(|v| {
        v.campaigns
            .iter()
            .find(|c| c.campaign.id == campaign)
            .and_then(|c| c.campaign.members.iter().find(|m| m.user_id == user))
            .map(|m| m.display_name.as_str().to_owned())
            .unwrap_or_default()
    })
}

fn who(r: &RollRecord, campaign: CampaignId) -> String {
    let player = member_name(campaign, r.roller);
    match &r.character {
        Some(c) if !c.is_empty() => format!("{c} ({player})"),
        _ => player,
    }
}

fn marks(i18n: I18n, r: &RollRecord) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(k) = natural_key(r) {
        out.push(i18n.t(k));
    }
    if r.visibility == Visibility::ToDm {
        out.push(i18n.t("dice-to-dm"));
    }
    if r.physical {
        out.push(i18n.t("dice-physical"));
    }
    out
}

#[component]
pub fn RollToast(roller: Roller) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let state = Memo::new(move |_| {
        let (id, crit) = roller.last.get()?;
        let parsed: CommandId = CommandId(id.parse().ok()?);
        ctx.view.with(|v| {
            let c = v
                .campaigns
                .iter()
                .find(|c| c.campaign.id == roller.campaign)?;
            let found = c.campaign.rolls.iter().find(|r| r.id == parsed).cloned();
            Some((found, crit))
        })
    });
    move || {
        let (found, crit) = state.get()?;
        let close = move |_| roller.last.set(None);
        Some(match found {
            None => view! {
                <div class="roll-toast" role="status">
                    <span>{move || i18n.t("dice-rolling")}</span>
                </div>
            }
            .into_any(),
            Some(r) => {
                let natural = r.outcome.natural();
                let crit_button = crit.filter(|_| natural != Some(1)).map(|(expr, context, targets)| {
                    let critical = natural == Some(20);
                    let expr = if critical { expr.critical() } else { expr };
                    view! {
                        <button type="button" class="primary"
                            on:click=move |_| roller.roll_at(expr.clone(), context.clone(), None, targets.clone())>
                            {move || if critical { i18n.t("dice-roll-critical") } else { i18n.t("dice-roll-damage") }}
                        </button>
                    }
                });
                let extra = marks(i18n, &r).join(" · ");
                view! {
                    <div class="roll-toast" role="status">
                        <span class="grow">
                            <span class="muted small">{r.context.clone()}" · "{r.expr.to_string()}</span>
                            <br/>
                            <strong class="roll-total">{r.outcome.total}</strong>
                            " "
                            <span class="small">{dice_detail(&r)}</span>
                            {(!extra.is_empty()).then(|| view! { <span class="small">" · "{extra.clone()}</span> })}
                        </span>
                        {crit_button}
                        <ApplyRoll campaign=roller.campaign roll=r.clone()/>
                        <button type="button" class="ghost" on:click=close>{move || i18n.t("sheet-close")}</button>
                    </div>
                }
                .into_any()
            }
        })
    }
}

#[component]
pub fn RollOptions(roller: Roller) -> impl IntoView {
    let i18n = use_i18n();
    let adv = move |a: Advantage, key: &'static str| {
        view! {
            <button type="button" class="level-filter" class:active=move || roller.advantage.get() == a
                aria-pressed=move || (roller.advantage.get() == a).to_string()
                on:click=move |_| roller.advantage.set(a)>
                {move || i18n.t(key)}
            </button>
        }
    };
    view! {
        <div class="field-row roll-options" role="group" aria-label=move || i18n.t("dice-options")>
            {adv(Advantage::Disadvantage, "dice-disadvantage")}
            {adv(Advantage::Normal, "dice-normal")}
            {adv(Advantage::Advantage, "dice-advantage")}
            <label class="choice small">
                <input type="checkbox" prop:checked=move || roller.visibility.get() == Visibility::ToDm
                    on:change=move |ev| roller.visibility.set(if event_target_checked(&ev) { Visibility::ToDm } else { Visibility::Public })/>
                {move || i18n.t("dice-to-dm-toggle")}
            </label>
        </div>
    }
}

#[component]
pub fn DiceLog(id: CampaignId) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let roller = Roller::new(id);
    let expression = RwSignal::new(String::new());
    let context = RwSignal::new(String::new());
    let results = RwSignal::new(String::new());
    let error = RwSignal::new(None::<&'static str>);
    let parse = move || Expr::parse(&expression.get_untracked()).ok();
    let roll = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        match parse() {
            Some(expr) => {
                error.set(None);
                roller.roll(expr, context.get_untracked(), None);
            }
            None => error.set(Some("dice-bad-expression")),
        }
    };
    let physical = move |_| {
        let Some(expr) = parse() else {
            error.set(Some("dice-bad-expression"));
            return;
        };
        let values: Vec<u16> = results
            .get_untracked()
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        let op = DiceOp::Physical {
            expr,
            results: values,
            context: context.get_untracked(),
            character: None,
            visibility: roller.visibility.get_untracked(),
            targets: Vec::new(),
        };
        if op.check().is_err() {
            error.set(Some("dice-bad-results"));
            return;
        }
        error.set(None);
        results.set(String::new());
        spawn_local(async move {
            let args = RollArgs {
                campaign_id: id.to_string(),
                op,
            };
            if let Err(e) = ipc::call::<String>("roll_dice", args).await {
                ctx.notify(error_key(e));
            }
        });
    };
    let log = move || {
        let (rolls, pending) = ctx.view.with(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == id)
                .map(|c| (c.campaign.rolls.clone(), c.pending_rolls.len()))
                .unwrap_or_default()
        });
        let rows = rolls
            .into_iter()
            .rev()
            .map(|r| {
                let extra = marks(i18n, &r).join(" · ");
                view! {
                    <li class="row roll-row">
                        <span class="grow">
                            <strong>{who(&r, id)}</strong>
                            <span class="muted small">" · "{r.context.clone()}" · "{r.expr.to_string()}</span>
                            <br/>
                            <span class="small">{dice_detail(&r)}</span>
                            {(!extra.is_empty()).then(|| view! { <span class="small">" · "{extra.clone()}</span> })}
                        </span>
                        <strong class="roll-total">{r.outcome.total}</strong>
                        <ApplyRoll campaign=id roll=r.clone()/>
                    </li>
                }
            })
            .collect_view();
        view! {
            {(pending > 0).then(|| view! { <p class="muted small" role="status">{move || i18n.t("dice-rolling")}</p> })}
            <ul class="list plain">{rows}</ul>
        }
    };
    view! {
        <section class="home">
            <h1>{move || i18n.t("nav-dice")}</h1>
            <form class="dice-box" on:submit=roll>
                <div class="filters">
                    <label class="filter"><span>{move || i18n.t("dice-expression")}</span>
                        <input type="text" required placeholder="1d20+5, 8d6, 2d20kh1+3, d20 adv" bind:value=expression/></label>
                    <label class="filter"><span>{move || i18n.t("dice-context")}</span>
                        <input type="text" maxlength="120" bind:value=context/></label>
                </div>
                <RollOptions roller=roller/>
                <div class="toolbar">
                    <button type="submit" class="primary">{move || i18n.t("dice-roll")}</button>
                    <input type="text" class="grow" bind:value=results placeholder=move || i18n.t("dice-results-hint")
                        aria-label=move || i18n.t("dice-results-hint")/>
                    <button type="button" on:click=physical>{move || i18n.t("dice-enter-physical")}</button>
                </div>
                {move || error.get().map(|k| view! { <p class="error" role="alert">{move || i18n.t(k)}</p> })}
            </form>
            <RollToast roller=roller/>
            {log}
        </section>
    }
}
