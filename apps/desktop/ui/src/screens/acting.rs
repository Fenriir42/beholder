use beholder_core::actions::{Damage, Details, MonsterAction};
use beholder_core::combat::{Combatant, Source};
use beholder_core::dice::Expr;
use beholder_core::{CampaignId, CharacterId, CombatantId};
use beholder_ipc::EntryView;
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;

use crate::app::use_app;
use crate::i18n::use_i18n;
use crate::screens::combat::{Economy, TargetPicker, mark_combatant};
use crate::screens::dice_view::{RollOptions, RollToast, Roller};
use crate::screens::sheet::{ABILITY_KEYS, signed};
use crate::screens::sheet_view::Sheet;
use crate::{ipc, markdown};

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

fn damage_expr(parts: &[Damage]) -> Option<Expr> {
    let mut out: Option<Expr> = None;
    for d in parts {
        let e = Expr::from(d.roll);
        out = Some(match out {
            None => e,
            Some(mut acc) => {
                acc.terms.extend(e.terms);
                acc.modifier += e.modifier;
                acc
            }
        });
    }
    out
}

fn damage_label(parts: &[Damage]) -> String {
    parts
        .iter()
        .map(|d| match &d.kind {
            Some(k) => format!("{} {k}", d.roll),
            None => d.roll.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

#[component]
pub fn ActingPanel(
    campaign: CampaignId,
    combatant: Memo<Option<Combatant>>,
    on_close: Callback<()>,
) -> impl IntoView {
    let i18n = use_i18n();
    let source =
        Memo::new(move |_| combatant.with(|c| c.as_ref().map(|c| (c.id, c.source.clone()))));
    let body = move || {
        let (_, source) = source.get()?;
        Some(match source {
            Source::Character(ch) => view! { <CharacterActor campaign=campaign ch=ch/> }.into_any(),
            Source::Monster { source, key } => {
                view! { <MonsterActor campaign=campaign combatant=combatant source=source key=key/> }.into_any()
            }
            Source::Custom => view! { <CustomActor campaign=campaign combatant=combatant/> }.into_any(),
        })
    };
    view! {
        <aside class="acting-panel" aria-label=move || i18n.t("acting-title")>
            <div class="toolbar">
                <strong class="grow">{move || combatant.with(|c| c.as_ref().map(|c| c.name.clone()).unwrap_or_default())}</strong>
                <button type="button" class="ghost" on:click=move |_| on_close.run(())>{move || i18n.t("sheet-close")}</button>
            </div>
            {body}
        </aside>
    }
}

#[component]
fn CharacterActor(campaign: CampaignId, ch: CharacterId) -> impl IntoView {
    let selected = RwSignal::new(Some(ch));
    view! { <div class="acting-sheet"><Sheet id=campaign selected=selected/></div> }
}

fn vitals(combatant: Memo<Option<Combatant>>) -> impl IntoView {
    let i18n = use_i18n();
    move || {
        combatant.get().map(|c| {
            let ac = c.ac.map(|a| a.to_string()).unwrap_or_else(|| "—".into());
            view! {
                <div class="toolbar">
                    <span class="stat inline"><span class="muted small">{move || i18n.t("combat-hp")}</span>
                        <strong>{format!("{}/{}", c.hp.current, c.hp.max)}</strong></span>
                    <span class="stat inline"><span class="muted small">{move || i18n.t("combat-ac")}</span>
                        <strong>{ac}</strong></span>
                </div>
            }
        })
    }
}

#[component]
fn MonsterActor(
    campaign: CampaignId,
    combatant: Memo<Option<Combatant>>,
    source: String,
    key: String,
) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let roller = Roller::new(campaign);
    roller.visibility.set(beholder_core::dice::Visibility::ToDm);
    Effect::new(move |_| {
        roller
            .character
            .set(combatant.with(|c| c.as_ref().map(|c| c.name.clone())));
    });
    let actions = RwSignal::new(None::<Vec<MonsterAction>>);
    let targets = RwSignal::new(Vec::<CombatantId>::new());
    let entry = RwSignal::new(None::<Option<EntryView>>);
    let sources = ctx.view.with_untracked(|v| {
        v.campaigns
            .iter()
            .find(|c| c.campaign.id == campaign)
            .map(|c| c.campaign.sources.clone())
            .unwrap_or_default()
    });
    spawn_local_scoped(async move {
        let refs = vec![(source.clone(), key.clone())];
        let details = ipc::call::<Vec<Option<Details>>>("sheet_details", DetailsArgs { refs })
            .await
            .ok()
            .and_then(|d| d.into_iter().next().flatten())
            .map(|d| d.actions)
            .unwrap_or_default();
        actions.try_set(Some(details));
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
    let image_base = ctx.boot.with_untracked(|b| b.image_base.clone());
    let rows = move || {
        let list = actions.get()?;
        if list.is_empty() {
            return Some(
                view! { <p class="muted small">{move || i18n.t("acting-no-actions")}</p> }
                    .into_any(),
            );
        }
        Some(
            list.into_iter()
                .map(|a| {
                    let damage = damage_expr(&a.damage);
                    let label = damage_label(&a.damage);
                    let name = a.name.clone();
                    let to_hit = a.to_hit.map(|bonus| {
                        let crit = damage.clone().map(|e| {
                            (e, i18n.tr("dice-ctx-damage", &[("what", name.clone().into())]))
                        });
                        let context = i18n.tr("dice-ctx-attack", &[("what", name.clone().into())]);
                        let attack = move |_| {
                            let chosen = targets.get_untracked();
                            let each: Vec<Vec<CombatantId>> = if chosen.is_empty() {
                                vec![Vec::new()]
                            } else {
                                chosen.into_iter().map(|t| vec![t]).collect()
                            };
                            for t in each {
                                let follow = crit.clone().map(|(e, c)| (e, c, t.clone()));
                                roller.test_at(i32::from(bonus), context.clone(), follow, t);
                            }
                            if let Some(cid) = combatant.with_untracked(|c| c.as_ref().map(|c| c.id)) {
                                mark_combatant(campaign, cid, Economy::Action);
                            }
                        };
                        view! {
                            <button type="button" class="primary" on:click=attack>
                                {format!("{} {}", signed(bonus), i18n.t("dice-to-hit"))}
                            </button>
                        }
                    });
                    let save = a.save.map(|(ability, dc)| {
                        let text = i18n.tr(
                            "map-save-dc",
                            &[("dc", dc.into()), ("ability", i18n.t(ABILITY_KEYS[ability.index()]).into())],
                        );
                        let half = a.half_on_save.then(|| i18n.t("acting-half-on-save"));
                        view! { <span class="chip private">{text}{half.map(|h| format!(" · {h}"))}</span> }
                    });
                    let roll_damage = damage.map(|e| {
                        let context = i18n.tr("dice-ctx-damage", &[("what", name.clone().into())]);
                        view! {
                            <button type="button" on:click=move |_| roller.roll_at(e.clone(), context.clone(), None, targets.get_untracked())>
                                {label.clone()}
                            </button>
                        }
                    });
                    view! {
                        <li class="row acting-action">
                            <strong class="grow">{a.name.clone()}</strong>
                            {save}
                            {to_hit}
                            {roll_damage}
                        </li>
                    }
                })
                .collect_view()
                .into_any(),
        )
    };
    let block = move || match entry.get() {
        None => None,
        Some(None) => Some(
            view! { <p class="muted">{move || i18n.t("sheet-not-in-compendium")}</p> }.into_any(),
        ),
        Some(Some(v)) => {
            let html = markdown::render(&v.entry.text, &image_base);
            Some(
                view! {
                    <details class="stat-block">
                        <summary>{move || i18n.t("acting-stat-block")}</summary>
                        <div class="prose" inner_html=html></div>
                    </details>
                }
                .into_any(),
            )
        }
    };
    view! {
        {vitals(combatant)}
        <RollOptions roller=roller/>
        <RollToast roller=roller/>
        <TargetPicker campaign=campaign targets=targets/>
        <ul class="list plain">{rows}</ul>
        {block}
    }
}

#[component]
fn CustomActor(campaign: CampaignId, combatant: Memo<Option<Combatant>>) -> impl IntoView {
    let i18n = use_i18n();
    let roller = Roller::new(campaign);
    roller.visibility.set(beholder_core::dice::Visibility::ToDm);
    Effect::new(move |_| {
        roller
            .character
            .set(combatant.with(|c| c.as_ref().map(|c| c.name.clone())));
    });
    let expression = RwSignal::new(String::new());
    let error = RwSignal::new(false);
    let roll = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        match Expr::parse(&expression.get_untracked()) {
            Ok(e) => {
                error.set(false);
                roller.roll(e, String::new(), None);
            }
            Err(_) => error.set(true),
        }
    };
    view! {
        {vitals(combatant)}
        <RollOptions roller=roller/>
        <RollToast roller=roller/>
        <p class="muted small">{move || i18n.t("acting-custom")}</p>
        <form class="toolbar" on:submit=roll>
            <input type="text" class="grow" required bind:value=expression placeholder="1d20+4, 2d6+2"
                aria-label=move || i18n.t("dice-expression")/>
            <button type="submit" class="primary">{move || i18n.t("dice-roll")}</button>
        </form>
        {move || error.get().then(|| view! { <p class="error" role="alert">{move || i18n.t("dice-bad-expression")}</p> })}
    }
}
