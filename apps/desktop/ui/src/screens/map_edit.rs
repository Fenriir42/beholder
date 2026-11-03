use beholder_core::actions::parse_note;
use beholder_core::combat::{
    Area, Colour, Door, EncounterOp, MAX_MAP, MAX_NOTE, MAX_TEMPLATE_FEET, Object, Shape,
    ShapeKind, Template, Terrain,
};
use beholder_core::dice::Expr;
use beholder_core::{CombatantId, ShapeId};
use leptos::prelude::*;

use crate::i18n::use_i18n;
use crate::screens::battle_map::{
    MapCtx, PALETTE, Tool, area_key, colour_var, kind_name, object_key, template_preset,
    terrain_key,
};
use crate::screens::sheet::ABILITY_KEYS;

const TERRAINS: [Terrain; 7] = [
    Terrain::Difficult,
    Terrain::Water,
    Terrain::HalfCover,
    Terrain::ThreeQuartersCover,
    Terrain::TotalCover,
    Terrain::LightlyObscured,
    Terrain::HeavilyObscured,
];

const OBJECTS: [Object; 7] = [
    Object::Door(Door::Closed),
    Object::Door(Door::Open),
    Object::Door(Door::Locked),
    Object::Chest,
    Object::Lever,
    Object::Statue,
    Object::Light,
];

const AREAS: [Area; 4] = [Area::Circle, Area::Cone, Area::Line, Area::Cube];

#[component]
pub fn MapTools(m: MapCtx, tool: RwSignal<Tool>) -> impl IntoView {
    let i18n = use_i18n();
    let flags = Memo::new(move |_| {
        m.encounter.with(|e| {
            e.as_ref()
                .map(|e| (e.id, e.width, e.height, e.active, e.locked, e.live_drag))
        })
    });
    let untokened = Memo::new(move |_| {
        m.encounter.with(|e| {
            e.as_ref()
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
    let button = move |kind: Option<ShapeKind>, key: &'static str| {
        let t = kind.map_or(Tool::Select, Tool::Place);
        view! {
            <button type="button" class="level-filter" class:active=move || tool.get() == t
                aria-pressed=move || (tool.get() == t).to_string()
                on:click=move |_| tool.set(t)>
                {move || i18n.t(key)}
            </button>
        }
    };
    let picker = move |label: &'static str, items: Vec<(ShapeKind, String)>| {
        let kinds: Vec<ShapeKind> = items.iter().map(|(k, _)| *k).collect();
        let kinds = StoredValue::new(kinds);
        let options = items
            .into_iter()
            .enumerate()
            .map(|(i, (_, name))| view! { <option value=i.to_string()>{name}</option> })
            .collect_view();
        view! {
            <select aria-label=move || i18n.t(label) class:active=move || kinds.with_value(|k| matches!(tool.get(), Tool::Place(p) if k.contains(&p)))
                prop:value=move || match tool.get() {
                    Tool::Place(p) => kinds.with_value(|k| k.iter().position(|x| *x == p)).map(|i| i.to_string()).unwrap_or_default(),
                    _ => String::new(),
                }
                on:change=move |ev| {
                    if let Some(k) = event_target_value(&ev).parse::<usize>().ok().and_then(|i| kinds.with_value(|k| k.get(i).copied())) {
                        tool.set(Tool::Place(k));
                    }
                }>
                <option value="">{move || i18n.t(label)}</option>
                {options}
            </select>
        }
    };
    let templates = move || {
        picker(
            "map-template",
            AREAS
                .iter()
                .map(|a| (template_preset(*a), i18n.t(area_key(*a))))
                .collect(),
        )
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
                    prop:value=move || match tool.get() { Tool::Token(c) => c.to_string(), _ => String::new() }
                    on:change=move |ev| {
                        if let Ok(id) = event_target_value(&ev).parse() {
                            tool.set(Tool::Token(CombatantId(id)));
                        }
                    }>
                    <option value="">{move || i18n.t("map-place-token")}</option>
                    {options}
                </select>
            }
        })
    };
    let settings = move || {
        let (id, w, h, _, locked, live) = flags.get()?;
        let resize = move |width: Option<u16>, height: Option<u16>| {
            let clamp = |v: u16| v.clamp(1, MAX_MAP);
            m.send(EncounterOp::Resize {
                id,
                width: clamp(width.unwrap_or(w)),
                height: clamp(height.unwrap_or(h)),
            });
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
                        on:change=move |ev| m.send(EncounterOp::Settings { id, locked: event_target_checked(&ev), live_drag: live })/>
                    {move || i18n.t("map-lock")}
                </label>
                <label class="choice small">
                    <input type="checkbox" prop:checked=live
                        on:change=move |ev| m.send(EncounterOp::Settings { id, locked, live_drag: event_target_checked(&ev) })/>
                    {move || i18n.t("map-live-drag")}
                </label>
            </div>
        })
    };
    move || {
        if !m.interactive {
            return None;
        }
        let (_, _, _, active, locked, _) = flags.get()?;
        if m.is_dm.get() {
            let terrain = picker(
                "map-terrain",
                TERRAINS
                    .iter()
                    .map(|t| (ShapeKind::Terrain(*t), i18n.t(terrain_key(*t))))
                    .collect(),
            );
            let objects = picker(
                "map-object",
                OBJECTS
                    .iter()
                    .map(|o| (ShapeKind::Object(*o), i18n.t(object_key(*o))))
                    .collect(),
            );
            Some(
                view! {
                    <div class="toolbar map-tools" role="group" aria-label=move || i18n.t("map-tools")>
                        {button(None, "map-select")}
                        {button(Some(ShapeKind::Wall), "map-wall")}
                        {button(Some(ShapeKind::Zone), "map-zone")}
                        {button(Some(ShapeKind::Hazard), "map-hazard")}
                        {terrain}
                        {objects}
                        {templates}
                        {token_picker}
                    </div>
                    {settings}
                }
                .into_any(),
            )
        } else {
            (active && !locked).then(|| {
                view! {
                    <div class="toolbar map-tools" role="group" aria-label=move || i18n.t("map-tools")>
                        {button(None, "map-select")}
                        {templates}
                    </div>
                }
                .into_any()
            })
        }
    }
}

#[component]
pub fn ShapePanel(m: MapCtx, selected: RwSignal<Option<ShapeId>>) -> impl IntoView {
    let i18n = use_i18n();
    let shape = Memo::new(move |_| {
        let id = selected.get()?;
        m.encounter.with(|e| {
            let e = e.as_ref()?;
            let s = e.shapes.iter().find(|s| s.id == id)?.clone();
            Some((e.id, e.width, e.height, s))
        })
    });
    let covered = Memo::new(move |_| {
        let (_, w, h, s) = shape.get()?;
        if !matches!(s.kind, ShapeKind::Template(_)) {
            return None;
        }
        let cells = s.cells(w, h);
        m.encounter.with(|e| {
            let e = e.as_ref()?;
            Some(
                e.shapes
                    .iter()
                    .filter(|t| {
                        t.kind == ShapeKind::Token
                            && t.cells(w, h).iter().any(|c| cells.contains(c))
                    })
                    .map(|t| t.label.clone())
                    .collect::<Vec<_>>(),
            )
        })
    });
    move || {
        let (encounter, _, _, s) = shape.get()?;
        let editable = m.editable(&s);
        let dm = m.is_dm.get_untracked();
        let stored = StoredValue::new(s.clone());
        let put = move |f: &dyn Fn(&mut Shape)| {
            let mut s = stored.get_value();
            f(&mut s);
            m.send(EncounterOp::PutShape {
                id: encounter,
                shape: s,
            });
        };
        let rolls = parse_note(&s.note);
        let context = if s.label.is_empty() {
            kind_name(i18n, s.kind)
        } else {
            s.label.clone()
        };
        let save = rolls.save.map(|(a, dc)| {
            let ability = i18n.t(ABILITY_KEYS[a.index()]);
            view! { <span class="chip private">{i18n.tr("map-save-dc", &[("dc", dc.into()), ("ability", ability.into())])}</span> }
        });
        let damage = rolls
            .damage
            .into_iter()
            .map(|d| {
                let label = match &d.kind {
                    Some(k) => format!("{} {k}", d.roll),
                    None => d.roll.to_string(),
                };
                let context = context.clone();
                view! {
                    <button type="button" on:click=move |_| m.roller.roll(Expr::from(d.roll), context.clone(), None)>
                        {label.clone()}
                    </button>
                }
            })
            .collect_view();
        let covers = covered.get().map(|names| {
            let text = if names.is_empty() {
                i18n.t("map-covers-none")
            } else {
                i18n.tr("map-covers", &[("names", names.join(", ").into())])
            };
            view! { <p class="small">{text}</p> }
        });
        let note_view =
            (!editable && !s.note.is_empty()).then(|| view! { <p>{s.note.clone()}</p> });
        let editor = editable.then(|| editor(s.clone(), dm, put));
        Some(view! {
            <fieldset class="shape-editor">
                <legend>{kind_name(i18n, s.kind)}</legend>
                {(!editable && !s.label.is_empty()).then(|| view! { <strong>{s.label.clone()}</strong> })}
                {note_view}
                <div class="toolbar">{save}{damage}</div>
                {covers}
                {editor}
                <div class="toolbar">
                    {editable.then(|| view! {
                        <button type="button" class="ghost danger"
                            on:click=move |_| { selected.set(None); m.send(EncounterOp::RemoveShape { id: encounter, shape: stored.with_value(|s| s.id) }) }>
                            {move || i18n.t("map-remove")}
                        </button>
                    })}
                    <button type="button" class="ghost" on:click=move |_| selected.set(None)>{move || i18n.t("sheet-close")}</button>
                </div>
            </fieldset>
        })
    }
}

fn editor(s: Shape, dm: bool, put: impl Fn(&dyn Fn(&mut Shape)) + Copy + 'static) -> impl IntoView {
    let i18n = use_i18n();
    let size = |ev: &leptos::ev::Event| {
        event_target_value(ev)
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|v| (1..=50).contains(v))
    };
    let sized = matches!(
        s.kind,
        ShapeKind::Token
            | ShapeKind::Wall
            | ShapeKind::Zone
            | ShapeKind::Terrain(_)
            | ShapeKind::Hazard
    );
    let coloured = matches!(
        s.kind,
        ShapeKind::Token | ShapeKind::Wall | ShapeKind::Zone | ShapeKind::Template(_)
    );
    let swatches = (dm && coloured).then(|| {
        (0..PALETTE)
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
            .collect_view()
    });
    let kind_select = match s.kind {
        ShapeKind::Terrain(current) => Some(
            view! {
                <select aria-label=move || i18n.t("map-terrain")
                    on:change=move |ev| {
                        let picked = event_target_value(&ev).parse::<usize>().ok().and_then(|i| TERRAINS.get(i).copied());
                        if let Some(t) = picked { put(&|s: &mut Shape| s.kind = ShapeKind::Terrain(t)) }
                    }>
                    {TERRAINS.iter().enumerate().map(|(i, t)| view! {
                        <option value=i.to_string() selected=*t == current>{i18n.t(terrain_key(*t))}</option>
                    }).collect_view()}
                </select>
            }
            .into_any(),
        ),
        ShapeKind::Object(current) => Some(
            view! {
                <select aria-label=move || i18n.t("map-object")
                    on:change=move |ev| {
                        let picked = event_target_value(&ev).parse::<usize>().ok().and_then(|i| OBJECTS.get(i).copied());
                        if let Some(o) = picked { put(&|s: &mut Shape| s.kind = ShapeKind::Object(o)) }
                    }>
                    {OBJECTS.iter().enumerate().map(|(i, o)| view! {
                        <option value=i.to_string() selected=*o == current>{i18n.t(object_key(*o))}</option>
                    }).collect_view()}
                </select>
            }
            .into_any(),
        ),
        ShapeKind::Template(t) => {
            let set = move |next: Template| put(&|s: &mut Shape| s.kind = ShapeKind::Template(next));
            let rotate = move |by: i16| {
                let angle = (i32::from(t.angle) + i32::from(by)).rem_euclid(360);
                set(Template {
                    angle: u16::try_from(angle).unwrap_or(0),
                    ..t
                });
            };
            Some(
                view! {
                    <select aria-label=move || i18n.t("map-template")
                        on:change=move |ev| {
                            let picked = event_target_value(&ev).parse::<usize>().ok().and_then(|i| AREAS.get(i).copied());
                            if let Some(a) = picked { set(Template { area: a, ..t }) }
                        }>
                        {AREAS.iter().enumerate().map(|(i, a)| view! {
                            <option value=i.to_string() selected=*a == t.area>{i18n.t(area_key(*a))}</option>
                        }).collect_view()}
                    </select>
                    <label class="filter"><span>{move || i18n.t("map-feet")}</span>
                        <input type="number" class="hp-amount" min="5" max=MAX_TEMPLATE_FEET step="5" prop:value=t.feet.to_string()
                            on:change=move |ev| {
                                if let Ok(f) = event_target_value(&ev).trim().parse::<u16>() {
                                    set(Template { feet: (f / 5 * 5).clamp(5, MAX_TEMPLATE_FEET), ..t })
                                }
                            }/></label>
                    {matches!(t.area, Area::Cone | Area::Line).then(|| view! {
                        <button type="button" aria-label=move || i18n.t("map-rotate-left") on:click=move |_| rotate(-15)>"⟲"</button>
                        <span class="small">{format!("{}°", t.angle)}</span>
                        <button type="button" aria-label=move || i18n.t("map-rotate-right") on:click=move |_| rotate(15)>"⟳"</button>
                    })}
                }
                .into_any(),
            )
        }
        _ => None,
    };
    let door_toggle = match s.kind {
        ShapeKind::Object(Object::Door(state)) => {
            let (next, key) = match state {
                Door::Open => (Door::Closed, "map-door-close"),
                Door::Closed | Door::Locked => (Door::Open, "map-door-open"),
            };
            Some(view! {
                <button type="button" class="primary" on:click=move |_| put(&|s: &mut Shape| s.kind = ShapeKind::Object(Object::Door(next)))>
                    {move || i18n.t(key)}
                </button>
            })
        }
        _ => None,
    };
    view! {
        <div class="toolbar">
            <input type="text" class="grow" maxlength="80" prop:value=s.label.clone()
                aria-label=move || i18n.t("map-shape-label") placeholder=move || i18n.t("map-shape-label")
                on:change=move |ev| { let v = event_target_value(&ev); put(&|s: &mut Shape| s.label = v.clone()) }/>
            {kind_select}
            {door_toggle}
        </div>
        <textarea maxlength=MAX_NOTE rows="2" prop:value=s.note.clone()
            aria-label=move || i18n.t("map-note") placeholder=move || i18n.t("map-note-hint")
            on:change=move |ev| { let v = event_target_value(&ev); put(&|s: &mut Shape| s.note = v.clone()) }></textarea>
        <div class="toolbar">
            {sized.then(|| view! {
                <label class="filter"><span>{move || i18n.t("map-w")}</span>
                    <input type="number" class="hp-amount" min="1" max="50" prop:value=s.w.to_string()
                        on:change=move |ev| if let Some(v) = size(&ev) { put(&|s: &mut Shape| s.w = v) }/></label>
                <label class="filter"><span>{move || i18n.t("map-h")}</span>
                    <input type="number" class="hp-amount" min="1" max="50" prop:value=s.h.to_string()
                        on:change=move |ev| if let Some(v) = size(&ev) { put(&|s: &mut Shape| s.h = v) }/></label>
            })}
            {dm.then(|| view! {
                <label class="choice small">
                    <input type="checkbox" prop:checked=s.hidden
                        on:change=move |ev| { let v = event_target_checked(&ev); put(&|s: &mut Shape| s.hidden = v) }/>
                    {move || i18n.t("combat-hide")}
                </label>
            })}
        </div>
        {swatches.map(|s| view! { <div class="toolbar">{s}</div> })}
    }
}
