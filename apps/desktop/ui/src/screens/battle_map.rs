use std::collections::HashMap;

use beholder_core::combat::{Colour, EncounterOp, MAX_MAP, Shape, ShapeKind, Source};
use beholder_core::{CampaignId, CombatantId, EncounterId, Role, ShapeId, UserId};
use beholder_protocol::Preview;
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use wasm_bindgen::JsCast;

use crate::app::use_app;
use crate::i18n::use_i18n;
use crate::ipc;
use crate::screens::combat::{fresh, send};

const CELL: i32 = 40;
const PALETTE: u8 = 8;
const PREVIEW_MS: f64 = 80.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Select,
    Wall,
    Zone,
    Token(CombatantId),
}

#[derive(Clone, Copy, PartialEq)]
struct Drag {
    shape: ShapeId,
    grab: (i16, i16),
    from: (i16, i16),
    at: (i16, i16),
    sent_at: f64,
    previewed: bool,
}

#[derive(serde::Serialize)]
struct PreviewArgs {
    preview: Preview,
}

fn colour_var(c: Colour) -> String {
    match c {
        Colour::Player(i) => format!("var(--player-{})", i % PALETTE),
        Colour::Palette(i) => format!("var(--palette-{})", i % PALETTE),
    }
}

fn initials(label: &str) -> String {
    label
        .split_whitespace()
        .take(2)
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .collect()
}

fn covers(s: &Shape, (x, y): (i16, i16)) -> bool {
    x >= s.x && y >= s.y && x < s.x + i16::from(s.w) && y < s.y + i16::from(s.h)
}

fn grid_path(w: u16, h: u16) -> String {
    let (pw, ph) = (i32::from(w) * CELL, i32::from(h) * CELL);
    let mut d = String::new();
    for i in 0..=i32::from(w) {
        d.push_str(&format!("M{} 0V{ph}", i * CELL));
    }
    for j in 0..=i32::from(h) {
        d.push_str(&format!("M0 {}H{pw}", j * CELL));
    }
    d
}

#[component]
pub fn BattleMap(campaign: CampaignId, encounter: EncounterId, is_dm: Memo<bool>) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let found = Memo::new(move |_| {
        ctx.view.with(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == campaign)
                .and_then(|c| c.campaign.encounters.iter().find(|e| e.id == encounter))
                .map(|e| (e.width, e.height, e.active, e.locked, e.live_drag))
        })
    });
    let shapes = Memo::new(move |_| {
        ctx.view.with(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == campaign)
                .and_then(|c| c.campaign.encounters.iter().find(|e| e.id == encounter))
                .map(|e| e.shapes.clone())
                .unwrap_or_default()
        })
    });
    let me = Memo::new(move |_| ctx.view.with(|v| v.user.as_ref().map(|u| u.id)));
    let previews = RwSignal::new(HashMap::<ShapeId, ((i16, i16), (i16, i16))>::new());
    Effect::new(move |_| {
        let Some(p) = ctx.preview.get() else {
            return;
        };
        if p.campaign_id != campaign || p.encounter != encounter {
            return;
        }
        let base =
            shapes.with_untracked(|l| l.iter().find(|s| s.id == p.shape).map(|s| (s.x, s.y)));
        if let Some(base) = base {
            previews.update(|m| {
                m.insert(p.shape, ((p.x, p.y), base));
            });
        }
    });
    let drag = RwSignal::new(None::<Drag>);
    let selected = RwSignal::new(None::<ShapeId>);
    let tool = RwSignal::new(Tool::Select);
    let zoom = RwSignal::new(32_u16);

    let can_move = move |s: &Shape| {
        if is_dm.get_untracked() {
            return true;
        }
        let Some((_, _, active, locked, _)) = found.get_untracked() else {
            return false;
        };
        active && !locked && s.owner.is_some() && s.owner == me.get_untracked()
    };
    let position = move |s: &Shape| -> (i16, i16) {
        if let Some(d) = drag.get().filter(|d| d.shape == s.id) {
            return d.at;
        }
        previews.with(|m| match m.get(&s.id) {
            Some((at, base)) if *base == (s.x, s.y) => *at,
            _ => (s.x, s.y),
        })
    };
    let cell_of = move |ev: &leptos::ev::PointerEvent| -> Option<(i16, i16)> {
        let (w, h, ..) = found.get_untracked()?;
        let el = ev.current_target()?.dyn_into::<web_sys::Element>().ok()?;
        let r = el.get_bounding_client_rect();
        if r.width() <= 0.0 || r.height() <= 0.0 {
            return None;
        }
        let x = ((f64::from(ev.client_x()) - r.left()) / r.width() * f64::from(w)).floor();
        let y = ((f64::from(ev.client_y()) - r.top()) / r.height() * f64::from(h)).floor();
        Some((
            x.clamp(0.0, f64::from(w) - 1.0) as i16,
            y.clamp(0.0, f64::from(h) - 1.0) as i16,
        ))
    };
    let send_preview = move |shape: ShapeId, (x, y): (i16, i16)| {
        let preview = Preview {
            campaign_id: campaign,
            encounter,
            shape,
            x,
            y,
        };
        spawn_local_scoped(async move {
            let _ = ipc::call::<()>("token_preview", PreviewArgs { preview }).await;
        });
    };

    let place = move |cell: (i16, i16), t: Tool| {
        let Some((w, h, ..)) = found.get_untracked() else {
            return;
        };
        let fit = |n: u8, room: i16| n.min(u8::try_from(room.max(1)).unwrap_or(u8::MAX));
        let room_x = i16::try_from(w).unwrap_or(i16::MAX) - cell.0;
        let room_y = i16::try_from(h).unwrap_or(i16::MAX) - cell.1;
        let base = |kind: ShapeKind, size: u8, colour: Colour| Shape {
            id: ShapeId::from_bytes([0; 16]),
            kind,
            x: cell.0,
            y: cell.1,
            w: fit(size, room_x),
            h: fit(size, room_y),
            label: String::new(),
            colour,
            hidden: false,
            combatant: None,
            owner: None,
        };
        let shape = match t {
            Tool::Select => return,
            Tool::Wall => base(ShapeKind::Wall, 1, Colour::Palette(0)),
            Tool::Zone => base(ShapeKind::Zone, 2, Colour::Palette(3)),
            Tool::Token(cid) => {
                let token = ctx.view.with_untracked(|v| {
                    let c = v.campaigns.iter().find(|c| c.campaign.id == campaign)?;
                    let e = c.campaign.encounters.iter().find(|e| e.id == encounter)?;
                    let k = e.combatants.iter().find(|k| k.id == cid)?;
                    let owner: Option<UserId> = match k.source {
                        Source::Character(ch) => c
                            .campaign
                            .characters
                            .iter()
                            .find(|x| x.id == ch)
                            .map(|x| x.owner),
                        _ => None,
                    };
                    let colour = owner
                        .and_then(|o| {
                            c.campaign
                                .members
                                .iter()
                                .filter(|m| m.role != Role::Dm)
                                .position(|m| m.user_id == o)
                        })
                        .map_or(Colour::Palette(1), |i| {
                            Colour::Player(u8::try_from(i % 8).unwrap_or(0))
                        });
                    Some(Shape {
                        label: k.name.clone(),
                        hidden: k.hidden,
                        combatant: Some(cid),
                        owner,
                        ..base(ShapeKind::Token, 1, colour)
                    })
                });
                tool.set(Tool::Select);
                match token {
                    Some(s) => s,
                    None => return,
                }
            }
        };
        spawn_local_scoped(async move {
            if let Some(id) = fresh::<ShapeId>(1).await.into_iter().next() {
                send(
                    campaign,
                    EncounterOp::PutShape {
                        id: encounter,
                        shape: Shape { id, ..shape },
                    },
                );
                selected.set(Some(id));
            }
        });
    };

    let down = move |ev: leptos::ev::PointerEvent| {
        let Some(cell) = cell_of(&ev) else {
            return;
        };
        let dm = is_dm.get_untracked();
        let t = tool.get_untracked();
        if dm && t != Tool::Select {
            place(cell, t);
            return;
        }
        let hit = shapes.with_untracked(|l| {
            let order = [ShapeKind::Token, ShapeKind::Wall, ShapeKind::Zone];
            order.iter().find_map(|k| {
                l.iter()
                    .rev()
                    .find(|s| s.kind == *k && covers(s, cell) && (dm || s.kind == ShapeKind::Token))
                    .cloned()
            })
        });
        if dm {
            selected.set(hit.as_ref().map(|s| s.id));
        }
        let Some(s) = hit.filter(|s| can_move(s)) else {
            return;
        };
        if let Some(el) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        {
            let _ = el.set_pointer_capture(ev.pointer_id());
        }
        drag.set(Some(Drag {
            shape: s.id,
            grab: (cell.0 - s.x, cell.1 - s.y),
            from: (s.x, s.y),
            at: (s.x, s.y),
            sent_at: 0.0,
            previewed: false,
        }));
    };
    let moved = move |ev: leptos::ev::PointerEvent| {
        let Some(d) = drag.get_untracked() else {
            return;
        };
        let Some(cell) = cell_of(&ev) else {
            return;
        };
        let at = ((cell.0 - d.grab.0).max(0), (cell.1 - d.grab.1).max(0));
        if at == d.at {
            return;
        }
        let live = found.get_untracked().is_some_and(|f| f.4);
        let now = js_sys::Date::now();
        let mut next = Drag { at, ..d };
        if live && now - d.sent_at >= PREVIEW_MS {
            send_preview(d.shape, at);
            next.sent_at = now;
            next.previewed = true;
        }
        drag.set(Some(next));
    };
    let up = move |_| {
        let Some(d) = drag.get_untracked() else {
            return;
        };
        drag.set(None);
        if d.previewed {
            send_preview(d.shape, d.at);
        }
        if d.at != d.from {
            send(
                campaign,
                EncounterOp::MoveShape {
                    id: encounter,
                    shape: d.shape,
                    x: d.at.0,
                    y: d.at.1,
                },
            );
        }
    };

    let drawn = move || {
        let mut list = shapes.get();
        list.sort_by_key(|s| match s.kind {
            ShapeKind::Zone => 0,
            ShapeKind::Wall => 1,
            ShapeKind::Token => 2,
        });
        let mine = me.get();
        let sel = selected.get();
        list.into_iter()
            .map(|s| {
                let (x, y) = position(&s);
                let (px, py) = (i32::from(x) * CELL, i32::from(y) * CELL);
                let (pw, ph) = (i32::from(s.w) * CELL, i32::from(s.h) * CELL);
                let fill = format!("fill: {}", colour_var(s.colour));
                let mut class = match s.kind {
                    ShapeKind::Token => "token".to_owned(),
                    ShapeKind::Wall => "wall".to_owned(),
                    ShapeKind::Zone => "zone".to_owned(),
                };
                if s.hidden {
                    class.push_str(" hidden-shape");
                }
                if Some(s.id) == sel {
                    class.push_str(" selected");
                }
                if s.owner.is_some() && s.owner == mine {
                    class.push_str(" mine");
                }
                let label = if s.hidden {
                    format!("{} ({})", s.label, i18n.t("combat-hidden-badge"))
                } else {
                    s.label.clone()
                };
                match s.kind {
                    ShapeKind::Token => {
                        let r = pw.min(ph) / 2 - 3;
                        let text = initials(&s.label);
                        view! {
                            <g class=class role="img" aria-label=label>
                                <circle cx=px + pw / 2 cy=py + ph / 2 r=r style=fill></circle>
                                <text x=px + pw / 2 y=py + ph / 2 class="token-label">{text}</text>
                            </g>
                        }
                        .into_any()
                    }
                    _ => {
                        let text = (!s.label.is_empty()).then(|| {
                            view! { <text x=px + 4 y=py + 14 class="zone-label">{s.label.clone()}</text> }
                        });
                        view! {
                            <g class=class role="img" aria-label=label>
                                <rect x=px y=py width=pw height=ph style=fill></rect>
                                {text}
                            </g>
                        }
                        .into_any()
                    }
                }
            })
            .collect_view()
    };

    let board = move || {
        let (w, h, ..) = found.get()?;
        let (pw, ph) = (i32::from(w) * CELL, i32::from(h) * CELL);
        let z = u32::from(zoom.get());
        Some(view! {
            <div class="map-scroll">
                <svg class="battle-map" viewBox=format!("0 0 {pw} {ph}")
                    width=u32::from(w) * z height=u32::from(h) * z
                    role="application" aria-label=move || i18n.t("map-label")
                    class:placing=move || tool.get() != Tool::Select
                    on:pointerdown=down on:pointermove=moved on:pointerup=up on:pointercancel=up>
                    <rect class="map-bg" width=pw height=ph></rect>
                    <path class="map-grid" d=grid_path(w, h)></path>
                    {drawn}
                </svg>
            </div>
        })
    };

    let zoom_bar = move || {
        view! {
            <div class="toolbar map-tools" role="group" aria-label=move || i18n.t("map-zoom")>
                <button type="button" aria-label=move || i18n.t("map-zoom-out")
                    on:click=move |_| zoom.update(|z| *z = (*z - 8).max(16))>"−"</button>
                <button type="button" aria-label=move || i18n.t("map-zoom-in")
                    on:click=move |_| zoom.update(|z| *z = (*z + 8).min(72))>"+"</button>
                {move || {
                    let (_, _, active, locked, live) = found.get()?;
                    let hint = if is_dm.get() {
                        None
                    } else if !active {
                        Some("map-inactive")
                    } else if locked {
                        Some("map-locked-hint")
                    } else if shapes.with(|l| l.iter().any(|s| s.owner.is_some() && s.owner == me.get())) {
                        Some(if live { "map-drag-live" } else { "map-drag-hint" })
                    } else {
                        None
                    };
                    hint.map(|k| view! { <span class="muted small">{move || i18n.t(k)}</span> })
                }}
            </div>
        }
    };

    view! {
        <div class="map-pane">
            {zoom_bar}
            {move || is_dm.get().then(|| view! {
                <MapTools campaign=campaign encounter=encounter tool=tool selected=selected shapes=shapes/>
            })}
            {board}
        </div>
    }
}

#[component]
fn MapTools(
    campaign: CampaignId,
    encounter: EncounterId,
    tool: RwSignal<Tool>,
    selected: RwSignal<Option<ShapeId>>,
    shapes: Memo<Vec<Shape>>,
) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let settings = Memo::new(move |_| {
        ctx.view.with(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == campaign)
                .and_then(|c| c.campaign.encounters.iter().find(|e| e.id == encounter))
                .map(|e| (e.width, e.height, e.locked, e.live_drag))
        })
    });
    let untokened = Memo::new(move |_| {
        ctx.view.with(|v| {
            v.campaigns
                .iter()
                .find(|c| c.campaign.id == campaign)
                .and_then(|c| c.campaign.encounters.iter().find(|e| e.id == encounter))
                .map(|e| {
                    e.combatants
                        .iter()
                        .filter(|k| !e.shapes.iter().any(|s| s.combatant == Some(k.id)))
                        .map(|k| (k.id, k.name.clone()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    let tool_button = move |t: Tool, key: &'static str| {
        view! {
            <button type="button" class="level-filter" class:active=move || tool.get() == t
                aria-pressed=move || (tool.get() == t).to_string()
                on:click=move |_| tool.set(t)>
                {move || i18n.t(key)}
            </button>
        }
    };
    let token_picker = move || {
        let list = untokened.get();
        (!list.is_empty()).then(|| {
            let options = list
                .into_iter()
                .map(|(id, name)| view! { <option value=id.to_string()>{name}</option> })
                .collect_view();
            view! {
                <select aria-label=move || i18n.t("map-place-token")
                    on:change=move |ev| {
                        if let Ok(id) = event_target_value(&ev).parse() {
                            tool.set(Tool::Token(CombatantId(id)));
                        }
                    }>
                    <option value="" selected=true>{move || i18n.t("map-place-token")}</option>
                    {options}
                </select>
            }
        })
    };
    let map_settings = move || {
        let (w, h, locked, live) = settings.get()?;
        let resize = move |width: Option<u16>, height: Option<u16>| {
            let clamp = |v: u16| v.clamp(1, MAX_MAP);
            send(
                campaign,
                EncounterOp::Resize {
                    id: encounter,
                    width: clamp(width.unwrap_or(w)),
                    height: clamp(height.unwrap_or(h)),
                },
            );
        };
        let num = |ev: &leptos::ev::Event| event_target_value(ev).trim().parse::<u16>().ok();
        Some(view! {
            <div class="toolbar">
                <label class="filter"><span>{move || i18n.t("map-width")}</span>
                    <input type="number" class="hp-amount" min="1" max=MAX_MAP prop:value=w.to_string()
                        on:change=move |ev| if let Some(v) = num(&ev) { resize(Some(v), None) }/></label>
                <label class="filter"><span>{move || i18n.t("map-height")}</span>
                    <input type="number" class="hp-amount" min="1" max=MAX_MAP prop:value=h.to_string()
                        on:change=move |ev| if let Some(v) = num(&ev) { resize(None, Some(v)) }/></label>
                <label class="choice small">
                    <input type="checkbox" prop:checked=locked
                        on:change=move |ev| send(campaign, EncounterOp::Settings { id: encounter, locked: event_target_checked(&ev), live_drag: live })/>
                    {move || i18n.t("map-lock")}
                </label>
                <label class="choice small">
                    <input type="checkbox" prop:checked=live
                        on:change=move |ev| send(campaign, EncounterOp::Settings { id: encounter, locked, live_drag: event_target_checked(&ev) })/>
                    {move || i18n.t("map-live-drag")}
                </label>
            </div>
        })
    };
    let editor = move || {
        let id = selected.get()?;
        let s = shapes.with(|l| l.iter().find(|s| s.id == id).cloned())?;
        let stored = StoredValue::new(s.clone());
        let put = move |f: &dyn Fn(&mut Shape)| {
            let mut s = stored.get_value();
            f(&mut s);
            send(
                campaign,
                EncounterOp::PutShape {
                    id: encounter,
                    shape: s,
                },
            );
        };
        let size = |ev: &leptos::ev::Event| {
            event_target_value(ev)
                .trim()
                .parse::<u8>()
                .ok()
                .filter(|v| (1..=50).contains(v))
        };
        let swatches = (0..PALETTE)
            .map(|i| {
                let c = Colour::Palette(i);
                let on = s.colour == c;
                view! {
                    <button type="button" class="swatch" style=format!("background: {}", colour_var(c))
                        aria-pressed=on.to_string()
                        aria-label=move || i18n.tr("map-colour", &[("n", (i + 1).into())])
                        on:click=move |_| put(&|s: &mut Shape| s.colour = c)>
                        {if on { "✓" } else { "" }}
                    </button>
                }
            })
            .collect_view();
        let kind = match s.kind {
            ShapeKind::Token => "map-token",
            ShapeKind::Wall => "map-wall",
            ShapeKind::Zone => "map-zone",
        };
        Some(view! {
            <fieldset class="shape-editor">
                <legend>{move || i18n.t(kind)}</legend>
                <div class="toolbar">
                    <input type="text" class="grow" maxlength="80" prop:value=s.label.clone()
                        aria-label=move || i18n.t("map-shape-label") placeholder=move || i18n.t("map-shape-label")
                        on:change=move |ev| { let v = event_target_value(&ev); put(&|s: &mut Shape| s.label = v.clone()) }/>
                    <label class="filter"><span>{move || i18n.t("map-w")}</span>
                        <input type="number" class="hp-amount" min="1" max="50" prop:value=s.w.to_string()
                            on:change=move |ev| if let Some(v) = size(&ev) { put(&|s: &mut Shape| s.w = v) }/></label>
                    <label class="filter"><span>{move || i18n.t("map-h")}</span>
                        <input type="number" class="hp-amount" min="1" max="50" prop:value=s.h.to_string()
                            on:change=move |ev| if let Some(v) = size(&ev) { put(&|s: &mut Shape| s.h = v) }/></label>
                    <label class="choice small">
                        <input type="checkbox" prop:checked=s.hidden
                            on:change=move |ev| { let v = event_target_checked(&ev); put(&|s: &mut Shape| s.hidden = v) }/>
                        {move || i18n.t("combat-hide")}
                    </label>
                </div>
                <div class="toolbar">
                    {swatches}
                    <button type="button" class="ghost danger"
                        on:click=move |_| { selected.set(None); send(campaign, EncounterOp::RemoveShape { id: encounter, shape: id }) }>
                        {move || i18n.t("map-remove")}
                    </button>
                </div>
            </fieldset>
        })
    };
    view! {
        <div class="toolbar map-tools" role="group" aria-label=move || i18n.t("map-tools")>
            {tool_button(Tool::Select, "map-select")}
            {tool_button(Tool::Wall, "map-wall")}
            {tool_button(Tool::Zone, "map-zone")}
            {token_picker}
        </div>
        {map_settings}
        {editor}
    }
}
