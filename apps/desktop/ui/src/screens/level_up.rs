use beholder_core::actions::Details;
use beholder_core::character::{
    Character, CharacterChange, CharacterOp, ClassLevel, LevelUp, Link,
};
use beholder_core::compendium::{Facets, Hit, Kind, SearchQuery};
use beholder_core::dice::{Expr, Keep, Term};
use beholder_core::progression::{Milestone, Progression, milestone};
use beholder_core::rules::{Ability, Caster, class_defaults, modifier};
use beholder_core::{CampaignId, CommandId};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;

use crate::app::use_app;
use crate::i18n::use_i18n;
use crate::ipc;
use crate::screens::dice_view::{RollToast, Roller};
use crate::screens::sheet::{ABILITY_KEYS, send, signed};
use crate::screens::sheet_view::sources_of;

#[derive(serde::Serialize)]
struct DetailsArgs {
    refs: Vec<(String, String)>,
}

#[derive(serde::Serialize)]
struct SearchArgs {
    query: SearchQuery,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Boost {
    Two,
    OneOne,
    Feat,
}

fn link_of(h: &Hit) -> Link {
    Link {
        name: h.name.clone(),
        source: Some(h.source.clone()),
        key: Some(h.key.clone()),
    }
}

async fn progression_of(campaign: CampaignId, link: &Link, kind: Kind) -> Option<Progression> {
    let (source, key) = match (&link.source, &link.key) {
        (Some(s), Some(k)) => (s.clone(), k.clone()),
        _ => {
            let (sources, hidden) = sources_of(campaign);
            let query = SearchQuery {
                text: link.name.clone(),
                kind: Some(kind),
                sources,
                hidden,
                ..SearchQuery::default()
            };
            let hits = ipc::call::<Vec<Hit>>("compendium_search", SearchArgs { query })
                .await
                .ok()?;
            let h = hits
                .into_iter()
                .find(|h| h.name.eq_ignore_ascii_case(link.name.trim()))?;
            (h.source, h.key)
        }
    };
    ipc::call::<Vec<Option<Details>>>(
        "sheet_details",
        DetailsArgs {
            refs: vec![(source, key)],
        },
    )
    .await
    .ok()?
    .into_iter()
    .next()
    .flatten()?
    .progression
}

async fn search(campaign: CampaignId, kind: Kind, text: String, class: Option<String>) -> Vec<Hit> {
    let (sources, hidden) = sources_of(campaign);
    let query = SearchQuery {
        text,
        kind: Some(kind),
        class,
        sources,
        hidden,
        ..SearchQuery::default()
    };
    ipc::call::<Vec<Hit>>("compendium_search", SearchArgs { query })
        .await
        .unwrap_or_default()
}

#[component]
pub fn LevelUpWizard(
    campaign: CampaignId,
    ch: Character,
    roller: Roller,
    on_done: Callback<()>,
) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let id = ch.id;
    let existing: Vec<ClassLevel> = ch.classes.clone();
    let start = existing
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| c.level)
        .map(|(i, _)| i);
    let pick = RwSignal::new(start);
    let new_class = RwSignal::new(None::<Link>);
    let class_hits = RwSignal::new(Vec::<Hit>::new());
    let class_text = RwSignal::new(String::new());
    let stored = StoredValue::new(existing);
    let chosen = Memo::new(move |_| -> Option<ClassLevel> {
        match pick.get() {
            Some(i) => stored.with_value(|e| e.get(i).cloned()),
            None => new_class.get().map(|link| {
                let (hit_die, caster, spell_ability) =
                    class_defaults(&link.name).unwrap_or((8, Caster::None, None));
                ClassLevel {
                    class: link,
                    subclass: None,
                    level: 0,
                    hit_die,
                    caster,
                    spell_ability,
                }
            }),
        }
    });
    let progression = RwSignal::new(None::<Progression>);
    let class_die = Memo::new(move |_| {
        let c = chosen.get()?;
        let from_table = progression.with(|p| p.as_ref().and_then(|p| p.hit_die));
        Some(if c.level == 0 {
            from_table.unwrap_or(c.hit_die)
        } else {
            c.hit_die
        })
    });
    let sub_progression = RwSignal::new(None::<Progression>);
    let subclass = RwSignal::new(None::<Link>);
    let sub_hits = RwSignal::new(Vec::<Hit>::new());
    let unticked = RwSignal::new(Vec::<String>::new());
    let boost = RwSignal::new(Boost::Two);
    let first = RwSignal::new(Ability::Str);
    let second = RwSignal::new(Ability::Dex);
    let feat = RwSignal::new(None::<Link>);
    let feat_hits = RwSignal::new(Vec::<Hit>::new());
    let feat_text = RwSignal::new(String::new());
    let hit_roll = RwSignal::new(None::<u8>);
    let pending_roll = RwSignal::new(None::<CommandId>);

    Effect::new(move |_| {
        let Some(c) = chosen.get() else {
            progression.set(None);
            return;
        };
        progression.set(None);
        sub_progression.set(None);
        subclass.set(c.subclass.clone());
        unticked.set(Vec::new());
        hit_roll.set(None);
        let link = c.class.clone();
        let sub = c.subclass.clone();
        spawn_local_scoped(async move {
            let p = progression_of(campaign, &link, Kind::Class).await;
            progression.try_set(Some(p.unwrap_or_default()));
            if let Some(s) = sub {
                let sp = progression_of(campaign, &s, Kind::Subclass).await;
                sub_progression.try_set(sp);
            }
        });
    });
    Effect::new(move |_| {
        let Some(cmd) = pending_roll.get() else {
            return;
        };
        let total = ctx.view.with(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == campaign)?
                .campaign
                .rolls
                .iter()
                .find(|r| r.id == cmd)
                .map(|r| r.outcome.total)
        });
        if let Some(t) = total {
            hit_roll.set(u8::try_from(t.max(1)).ok());
            pending_roll.set(None);
        }
    });

    let level = Memo::new(move |_| chosen.with(|c| c.as_ref().map_or(1, |c| c.level + 1)));
    let row_features = Memo::new(move |_| {
        let n = level.get();
        let mut out: Vec<String> = progression.with(|p| {
            p.as_ref()
                .and_then(|p| p.at(n))
                .map(|r| r.features.clone())
                .unwrap_or_default()
        });
        out.extend(sub_progression.with(|p| {
            p.as_ref()
                .and_then(|p| p.at(n))
                .map(|r| r.features.clone())
                .unwrap_or_default()
        }));
        out
    });
    let milestones = Memo::new(move |_| {
        row_features.with(|f| f.iter().filter_map(|n| milestone(n)).collect::<Vec<_>>())
    });
    let needs_subclass = Memo::new(move |_| {
        milestones.with(|m| m.contains(&Milestone::Subclass))
            && chosen.with(|c| c.as_ref().is_some_and(|c| c.subclass.is_none()))
    });
    let improvement = Memo::new(move |_| milestones.with(|m| m.contains(&Milestone::Improvement)));
    let boon = Memo::new(move |_| milestones.with(|m| m.contains(&Milestone::EpicBoon)));
    let total_level: u8 = ch.level();
    let con = ch.abilities[Ability::Con.index()];
    let abilities = ch.abilities;

    let increase = move || -> [u8; 6] {
        let mut inc = [0u8; 6];
        if improvement.get() {
            match boost.get() {
                Boost::Two => inc[first.get().index()] = 2,
                Boost::OneOne => {
                    inc[first.get().index()] = 1;
                    inc[second.get().index()] += 1;
                }
                Boost::Feat => {}
            }
        }
        inc
    };
    let gain = move || {
        let die = class_die.get()?;
        let roll = if total_level == 0 {
            die
        } else {
            hit_roll.get()?
        };
        let new_con = modifier((con + increase()[Ability::Con.index()]).min(20.max(con)));
        let old = modifier(con);
        Some(
            (i16::from(roll) + i16::from(new_con)).max(1)
                + (i16::from(new_con) - i16::from(old)) * i16::from(total_level),
        )
    };

    let class_step = move || {
        let options = stored
            .with_value(|e| e.iter().map(|c| format!("{} {}", c.class.name, c.level)).collect::<Vec<_>>())
            .into_iter()
            .enumerate()
            .map(|(i, label)| view! { <option value=i.to_string() selected=pick.get_untracked() == Some(i)>{label}</option> })
            .collect_view();
        let find_class = move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let text = class_text.get_untracked();
            spawn_local_scoped(async move {
                class_hits.try_set(search(campaign, Kind::Class, text, None).await);
            });
        };
        view! {
            <fieldset class="shape-editor">
                <legend>{move || i18n.t("levelup-class")}</legend>
                <select aria-label=move || i18n.t("levelup-class")
                    on:change=move |ev| pick.set(event_target_value(&ev).parse().ok())>
                    {options}
                    <option value="new" selected=pick.get_untracked().is_none()>{move || i18n.t("levelup-new-class")}</option>
                </select>
                {move || pick.get().is_none().then(|| view! {
                    <form class="toolbar" on:submit=find_class>
                        <input type="search" class="grow" bind:value=class_text
                            placeholder=move || i18n.t("levelup-find-class") aria-label=move || i18n.t("levelup-find-class")/>
                        <button type="submit">{move || i18n.t("combat-search")}</button>
                    </form>
                    <div class="toolbar">
                        {move || class_hits.get().into_iter().map(|h| {
                            let link = link_of(&h);
                            let on = new_class.with(|n| n.as_ref().is_some_and(|n| n.name == h.name));
                            view! {
                                <button type="button" class="level-filter" class:active=on aria-pressed=on.to_string()
                                    on:click=move |_| new_class.set(Some(link.clone()))>{h.name.clone()}</button>
                            }
                        }).collect_view()}
                    </div>
                })}
            </fieldset>
        }
    };

    let features_step = move || {
        let n = level.get();
        let list = row_features.get();
        let loading = progression.with(Option::is_none) && chosen.with(Option::is_some);
        view! {
            <fieldset class="shape-editor">
                <legend>{move || i18n.tr("levelup-features", &[("level", n.into())])}</legend>
                {loading.then(|| view! { <p class="muted small">{move || i18n.t("preview-loading")}</p> })}
                {(!loading && list.is_empty()).then(|| view! { <p class="muted small">{move || i18n.t("levelup-no-features")}</p> })}
                {list.into_iter().filter(|f| milestone(f).is_none()).map(|f| {
                    let name = f.clone();
                    let on = move || !unticked.with(|u| u.contains(&name));
                    let toggle = f.clone();
                    view! {
                        <label class="choice small">
                            <input type="checkbox" prop:checked=on
                                on:change=move |ev| {
                                    let keep = event_target_checked(&ev);
                                    unticked.update(|u| {
                                        u.retain(|x| *x != toggle);
                                        if !keep { u.push(toggle.clone()); }
                                    });
                                }/>
                            {f.clone()}
                        </label>
                    }
                }).collect_view()}
                {move || {
                    let n = level.get();
                    let counts = progression.with(|p| {
                        let p = p.as_ref()?;
                        let now = p.at(n)?;
                        let before = p.at(n.saturating_sub(1));
                        Some(now.counts.iter().filter_map(|(name, v)| {
                            let old = before.and_then(|b| b.counts.iter().find(|(x, _)| x == name)).map(|x| x.1);
                            (old != Some(*v)).then(|| format!("{name}: {} → {v}", old.map(|o| o.to_string()).unwrap_or_else(|| "—".into())))
                        }).collect::<Vec<_>>())
                    }).unwrap_or_default();
                    (!counts.is_empty()).then(|| view! {
                        <p class="small">{counts.join(" · ")}</p>
                        <p class="muted small">{move || i18n.t("levelup-spells-hint")}</p>
                    })
                }}
            </fieldset>
        }
    };

    let subclass_step = move || {
        needs_subclass.get().then(|| {
            let class_name = chosen.with(|c| c.as_ref().map(|c| c.class.name.clone()));
            spawn_local_scoped(async move {
                sub_hits.try_set(search(campaign, Kind::Subclass, String::new(), class_name).await);
            });
            view! {
                <fieldset class="shape-editor">
                    <legend>{move || i18n.t("levelup-subclass")}</legend>
                    <div class="toolbar">
                        {move || sub_hits.get().into_iter().filter(|h| matches!(h.facets, Facets::Subclass { .. })).map(|h| {
                            let link = link_of(&h);
                            let on = subclass.with(|s| s.as_ref().is_some_and(|s| s.name == h.name));
                            view! {
                                <button type="button" class="level-filter" class:active=on aria-pressed=on.to_string()
                                    on:click=move |_| {
                                        let l = link.clone();
                                        subclass.set(Some(l.clone()));
                                        spawn_local_scoped(async move {
                                            let sp = progression_of(campaign, &l, Kind::Subclass).await;
                                            sub_progression.try_set(sp);
                                        });
                                    }>{if on { "✓ " } else { "" }}{h.name.clone()}</button>
                            }
                        }).collect_view()}
                    </div>
                    {move || sub_hits.with(Vec::is_empty).then(|| view! { <p class="muted small">{move || i18n.t("levelup-no-subclass")}</p> })}
                </fieldset>
            }
        })
    };

    let ability_select = move |sig: RwSignal<Ability>, label: &'static str| {
        view! {
            <select aria-label=move || i18n.t(label)
                on:change=move |ev| {
                    if let Some(a) = event_target_value(&ev).parse::<usize>().ok().and_then(|i| Ability::ALL.get(i).copied()) {
                        sig.set(a);
                    }
                }>
                {Ability::ALL.iter().enumerate().map(|(i, a)| {
                    let score = abilities[a.index()];
                    view! { <option value=i.to_string() selected=sig.get_untracked() == *a>{format!("{} ({score})", i18n.t(ABILITY_KEYS[a.index()]))}</option> }
                }).collect_view()}
            </select>
        }
    };
    let feat_picker = move |category: Option<&'static str>| {
        let find = move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let text = feat_text.get_untracked();
            spawn_local_scoped(async move {
                let mut hits = search(campaign, Kind::Feat, text, None).await;
                if let Some(cat) = category {
                    hits.retain(|h| matches!(&h.facets, Facets::Feat { category } if category.to_lowercase().contains(cat)));
                }
                feat_hits.try_set(hits.into_iter().take(30).collect());
            });
        };
        view! {
            <form class="toolbar" on:submit=find>
                <input type="search" class="grow" bind:value=feat_text
                    placeholder=move || i18n.t("levelup-find-feat") aria-label=move || i18n.t("levelup-find-feat")/>
                <button type="submit">{move || i18n.t("combat-search")}</button>
            </form>
            <div class="toolbar">
                {move || feat_hits.get().into_iter().map(|h| {
                    let link = link_of(&h);
                    let on = feat.with(|f| f.as_ref().is_some_and(|f| f.name == h.name));
                    view! {
                        <button type="button" class="level-filter" class:active=on aria-pressed=on.to_string()
                            on:click=move |_| feat.set(Some(link.clone()))>{if on { "✓ " } else { "" }}{h.name.clone()}</button>
                    }
                }).collect_view()}
            </div>
        }
    };
    let improvement_step = move || {
        if boon.get() {
            return Some(
                view! {
                    <fieldset class="shape-editor">
                        <legend>{move || i18n.t("levelup-boon")}</legend>
                        {feat_picker(Some("epic"))}
                    </fieldset>
                }
                .into_any(),
            );
        }
        improvement.get().then(|| {
            let choice = move |b: Boost, key: &'static str| view! {
                <label class="choice small">
                    <input type="radio" name="boost" prop:checked=move || boost.get() == b on:change=move |_| boost.set(b)/>
                    {move || i18n.t(key)}
                </label>
            };
            view! {
                <fieldset class="shape-editor">
                    <legend>{move || i18n.t("levelup-improvement")}</legend>
                    <div class="toolbar">
                        {choice(Boost::Two, "levelup-plus-two")}
                        {choice(Boost::OneOne, "levelup-plus-one-one")}
                        {choice(Boost::Feat, "levelup-feat")}
                    </div>
                    {move || match boost.get() {
                        Boost::Two => view! { <div class="toolbar">{ability_select(first, "levelup-ability")}</div> }.into_any(),
                        Boost::OneOne => view! { <div class="toolbar">{ability_select(first, "levelup-ability")}{ability_select(second, "levelup-ability-second")}</div> }.into_any(),
                        Boost::Feat => feat_picker(None).into_any(),
                    }}
                    <p class="muted small">{move || i18n.t("levelup-cap")}</p>
                </fieldset>
            }
            .into_any()
        })
    };

    let hp_step = move || {
        let die = class_die.get()?;
        if total_level == 0 {
            return Some(view! { <p class="small">{move || i18n.tr("levelup-hp-first", &[("die", die.into())])}</p> }.into_any());
        }
        let average = die / 2 + 1;
        let roll = move |_| {
            let expr = Expr {
                terms: vec![Term {
                    negative: false,
                    count: 1,
                    sides: u16::from(die),
                    keep: Keep::All,
                }],
                modifier: 0,
            };
            roller.roll(
                expr,
                i18n.tr(
                    "levelup-hp-roll",
                    &[("level", level.get_untracked().into())],
                ),
                None,
            );
        };
        Some(view! {
            <fieldset class="shape-editor">
                <legend>{move || i18n.tr("levelup-hp", &[("die", die.into())])}</legend>
                <div class="toolbar">
                    <button type="button" on:click=move |_| hit_roll.set(Some(average))>
                        {move || i18n.tr("levelup-average", &[("n", average.into())])}
                    </button>
                    <button type="button" on:click=roll>{move || i18n.tr("levelup-roll", &[("die", die.into())])}</button>
                    <label class="filter"><span>{move || i18n.t("levelup-result")}</span>
                        <input type="number" class="hp-amount" min="1" max=die
                            prop:value=move || hit_roll.get().map(|r| r.to_string()).unwrap_or_default()
                            on:change=move |ev| hit_roll.set(event_target_value(&ev).trim().parse::<u8>().ok().map(|r| r.clamp(1, die)))/>
                    </label>
                </div>
                <RollToast roller=roller/>
                {move || gain().map(|g| view! { <p class="small">{i18n.tr("levelup-gain", &[("n", signed(g).into())])}</p> })}
            </fieldset>
        }
        .into_any())
    };
    let earlier = roller.last.get_untracked().map(|(cmd, _)| cmd);
    Effect::new(move |_| {
        let Some((cmd, _)) = roller.last.get() else {
            return;
        };
        if Some(&cmd) == earlier.as_ref() {
            return;
        }
        if let Ok(u) = cmd.parse() {
            pending_roll.set(Some(CommandId(u)));
        }
    });

    let ready = move || {
        chosen.with(Option::is_some)
            && (total_level == 0 || hit_roll.get().is_some())
            && (!needs_subclass.get() || subclass.with(Option::is_some))
            && (!(boon.get() || (improvement.get() && boost.get() == Boost::Feat))
                || feat.with(Option::is_some))
            && (!improvement.get() || boost.get() != Boost::OneOne || first.get() != second.get())
    };
    let confirm = move |_| {
        let Some(mut class) = chosen.get_untracked() else {
            return;
        };
        if let Some(die) = class_die.get_untracked() {
            class.hit_die = die;
        }
        let features = row_features
            .get_untracked()
            .into_iter()
            .filter(|f| milestone(f).is_none() && !unticked.with_untracked(|u| u.contains(f)))
            .map(|name| Link {
                name,
                ..Link::default()
            })
            .collect();
        let wants_feat = boon.get_untracked()
            || (improvement.get_untracked() && boost.get_untracked() == Boost::Feat);
        let up = LevelUp {
            class,
            subclass: if needs_subclass.get_untracked() {
                subclass.get_untracked()
            } else {
                None
            },
            hit_roll: hit_roll.get_untracked().unwrap_or(1),
            features,
            increase: increase(),
            feat: if wants_feat {
                feat.get_untracked()
            } else {
                None
            },
        };
        send(
            campaign,
            CharacterOp::Change {
                id,
                change: CharacterChange::LevelUp(Box::new(up)),
            },
        );
        on_done.run(());
    };

    view! {
        <section class="home sheet level-up">
            <div class="toolbar">
                <h1 class="grow">{move || i18n.tr("levelup-title", &[("level", (total_level + 1).into())])}</h1>
                <button type="button" class="ghost" on:click=move |_| on_done.run(())>{move || i18n.t("cancel")}</button>
            </div>
            {class_step}
            {features_step}
            {subclass_step}
            {improvement_step}
            {hp_step}
            <div class="toolbar">
                <button type="button" class="primary" disabled=move || !ready() on:click=confirm>
                    {move || i18n.t("levelup-confirm")}
                </button>
            </div>
        </section>
    }
}
