use std::collections::HashMap;

use beholder_core::combat::{
    Area, Colour, Door, Encounter, EncounterOp, Object, Shape, ShapeKind, Template, Terrain,
};
use beholder_core::{CampaignId, CombatantId, Role, ShapeId, UserId};
use beholder_protocol::Preview;
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use wasm_bindgen::JsCast;

use crate::app::use_app;
use crate::i18n::{I18n, use_i18n};
use crate::ipc;
use crate::screens::combat::{fresh, send};
use crate::screens::dice_view::Roller;
use crate::screens::map_edit::{MapTools, ShapePanel};

pub(crate) const CELL: i32 = 40;
pub(crate) const PALETTE: u8 = 8;
const PREVIEW_MS: f64 = 80.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tool {
    Select,
    Place(ShapeKind),
    Token(CombatantId),
}

#[derive(Clone, Copy)]
pub struct MapCtx {
    pub campaign: CampaignId,
    pub encounter: Signal<Option<Encounter>>,
    pub is_dm: Signal<bool>,
    pub me: Signal<Option<UserId>>,
    pub interactive: bool,
    pub roller: Roller,
    pub acting: Option<RwSignal<Option<CombatantId>>>,
}

impl MapCtx {
    pub(crate) fn send(&self, op: EncounterOp) {
        if self.interactive {
            send(self.campaign, op);
        }
    }

    pub(crate) fn editable(&self, s: &Shape) -> bool {
        self.interactive
            && (self.is_dm.get_untracked()
                || (matches!(s.kind, ShapeKind::Template(_))
                    && s.owner.is_some()
                    && s.owner == self.me.get_untracked()))
    }
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

pub(crate) fn colour_var(c: Colour) -> String {
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

pub(crate) fn terrain_key(t: Terrain) -> &'static str {
    match t {
        Terrain::Difficult => "terrain-difficult",
        Terrain::Water => "terrain-water",
        Terrain::HalfCover => "terrain-half-cover",
        Terrain::ThreeQuartersCover => "terrain-three-quarters-cover",
        Terrain::TotalCover => "terrain-total-cover",
        Terrain::LightlyObscured => "terrain-lightly-obscured",
        Terrain::HeavilyObscured => "terrain-heavily-obscured",
    }
}

fn terrain_pattern(t: Terrain) -> &'static str {
    match t {
        Terrain::Difficult => "url(#pat-difficult)",
        Terrain::Water => "url(#pat-water)",
        Terrain::HalfCover | Terrain::ThreeQuartersCover => "url(#pat-cover)",
        Terrain::TotalCover => "url(#pat-cover-total)",
        Terrain::LightlyObscured => "url(#pat-light)",
        Terrain::HeavilyObscured => "url(#pat-heavy)",
    }
}

pub(crate) fn object_key(o: Object) -> &'static str {
    match o {
        Object::Door(Door::Open) => "object-door-open",
        Object::Door(Door::Closed) => "object-door-closed",
        Object::Door(Door::Locked) => "object-door-locked",
        Object::Chest => "object-chest",
        Object::Lever => "object-lever",
        Object::Statue => "object-statue",
        Object::Light => "object-light",
    }
}

pub(crate) fn area_key(a: Area) -> &'static str {
    match a {
        Area::Circle => "area-circle",
        Area::Cone => "area-cone",
        Area::Line => "area-line",
        Area::Cube => "area-cube",
    }
}

pub(crate) fn kind_name(i18n: I18n, k: ShapeKind) -> String {
    match k {
        ShapeKind::Token => i18n.t("map-token"),
        ShapeKind::Wall => i18n.t("map-wall"),
        ShapeKind::Zone => i18n.t("map-zone"),
        ShapeKind::Terrain(t) => i18n.t(terrain_key(t)),
        ShapeKind::Hazard => i18n.t("map-hazard"),
        ShapeKind::Object(o) => i18n.t(object_key(o)),
        ShapeKind::Template(t) => i18n.tr(
            "map-template-size",
            &[
                ("feet", t.feet.into()),
                ("area", i18n.t(area_key(t.area)).into()),
            ],
        ),
    }
}

fn rank(k: ShapeKind) -> u8 {
    match k {
        ShapeKind::Token => 0,
        ShapeKind::Object(_) => 1,
        ShapeKind::Template(_) => 2,
        ShapeKind::Hazard => 3,
        ShapeKind::Terrain(_) => 4,
        ShapeKind::Wall => 5,
        ShapeKind::Zone => 6,
    }
}

fn hit(s: &Shape, (x, y): (i16, i16)) -> bool {
    match s.kind {
        ShapeKind::Template(_) => (s.x, s.y) == (x, y),
        _ => x >= s.x && y >= s.y && x < s.x + i16::from(s.w) && y < s.y + i16::from(s.h),
    }
}

pub(crate) fn player_colour(campaign: CampaignId, user: UserId) -> Option<Colour> {
    use_app().view.with_untracked(|v| {
        let c = v.campaigns.iter().find(|c| c.campaign.id == campaign)?;
        c.campaign
            .members
            .iter()
            .filter(|m| m.role != Role::Dm)
            .position(|m| m.user_id == user)
            .map(|i| Colour::Player(u8::try_from(i % 8).unwrap_or(0)))
    })
}

pub(crate) fn template_preset(area: Area) -> ShapeKind {
    let feet = match area {
        Area::Circle => 20,
        Area::Cone => 15,
        Area::Line => 30,
        Area::Cube => 10,
    };
    ShapeKind::Template(Template {
        area,
        feet,
        angle: 0,
    })
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

fn patterns() -> impl IntoView {
    view! {
        <defs>
            <pattern id="pat-difficult" width="12" height="12" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
                <rect width="12" height="12" class="pat-bg terrain-difficult"></rect>
                <line x1="0" y1="0" x2="0" y2="12" class="pat-line"></line>
            </pattern>
            <pattern id="pat-water" width="20" height="10" patternUnits="userSpaceOnUse">
                <rect width="20" height="10" class="pat-bg terrain-water"></rect>
                <path d="M0 5 Q5 0 10 5 T20 5" class="pat-line"></path>
            </pattern>
            <pattern id="pat-cover" width="14" height="14" patternUnits="userSpaceOnUse">
                <rect width="14" height="14" class="pat-bg terrain-cover"></rect>
                <path d="M0 0L14 14M14 0L0 14" class="pat-line"></path>
            </pattern>
            <pattern id="pat-cover-total" width="8" height="8" patternUnits="userSpaceOnUse">
                <rect width="8" height="8" class="pat-bg terrain-cover"></rect>
                <path d="M0 0L8 8M8 0L0 8" class="pat-line"></path>
            </pattern>
            <pattern id="pat-light" width="14" height="14" patternUnits="userSpaceOnUse">
                <rect width="14" height="14" class="pat-bg terrain-obscured"></rect>
                <circle cx="7" cy="7" r="1.5" class="pat-dot"></circle>
            </pattern>
            <pattern id="pat-heavy" width="7" height="7" patternUnits="userSpaceOnUse">
                <rect width="7" height="7" class="pat-bg terrain-obscured-heavy"></rect>
                <circle cx="3.5" cy="3.5" r="2" class="pat-dot"></circle>
            </pattern>
            <pattern id="pat-hazard" width="12" height="12" patternUnits="userSpaceOnUse" patternTransform="rotate(-45)">
                <rect width="12" height="12" class="pat-bg hazard-bg"></rect>
                <rect width="5" height="12" class="hazard-stripe"></rect>
            </pattern>
        </defs>
    }
}

fn object_glyph(o: Object, px: i32, py: i32) -> AnyView {
    let (cx, cy) = (px + CELL / 2, py + CELL / 2);
    match o {
        Object::Door(state) => {
            let class = match state {
                Door::Open => "door open",
                Door::Closed => "door",
                Door::Locked => "door locked",
            };
            view! {
                <rect x=px + 2 y=cy - 5 width=CELL - 4 height="10" class=class></rect>
                {(state == Door::Locked).then(|| view! { <circle cx=cx cy=cy r="4" class="keyhole"></circle> })}
            }
            .into_any()
        }
        Object::Chest => view! {
            <rect x=px + 8 y=py + 12 width=CELL - 16 height=CELL - 20 class="object-body"></rect>
            <line x1=px + 8 y1=py + 19 x2=px + CELL - 8 y2=py + 19 class="object-line"></line>
        }
        .into_any(),
        Object::Lever => view! {
            <line x1=cx y1=py + CELL - 8 x2=cx + 9 y2=py + 10 class="object-line"></line>
            <circle cx=cx + 9 cy=py + 10 r="4" class="object-body"></circle>
            <rect x=cx - 8 y=py + CELL - 10 width="16" height="4" class="object-body"></rect>
        }
        .into_any(),
        Object::Statue => view! {
            <circle cx=cx cy=py + 14 r="7" class="object-body"></circle>
            <rect x=cx - 10 y=py + 24 width="20" height="8" class="object-body"></rect>
        }
        .into_any(),
        Object::Light => view! {
            <circle cx=cx cy=cy r=CELL * 2 class="light-glow"></circle>
            <circle cx=cx cy=cy r="6" class="light-core"></circle>
        }
        .into_any(),
    }
}

fn draw_shape(
    i18n: I18n,
    s: &Shape,
    (x, y): (i16, i16),
    (w, h): (u16, u16),
    class: String,
) -> AnyView {
    let (px, py) = (i32::from(x) * CELL, i32::from(y) * CELL);
    let (pw, ph) = (i32::from(s.w) * CELL, i32::from(s.h) * CELL);
    let fill = format!("fill: {}", colour_var(s.colour));
    let name = kind_name(i18n, s.kind);
    let mut label = if s.label.is_empty() {
        name.clone()
    } else {
        format!("{}: {name}", s.label)
    };
    if s.hidden {
        label = format!("{label} ({})", i18n.t("combat-hidden-badge"));
    }
    let tag = |text: String| view! { <text x=px + 4 y=py + 14 class="zone-label">{text}</text> };
    match s.kind {
        ShapeKind::Token => {
            let r = pw.min(ph) / 2 - 3;
            view! {
                <g class=class role="img" aria-label=label>
                    <circle cx=px + pw / 2 cy=py + ph / 2 r=r style=fill></circle>
                    <text x=px + pw / 2 y=py + ph / 2 class="token-label">{initials(&s.label)}</text>
                </g>
            }
            .into_any()
        }
        ShapeKind::Terrain(t) => {
            let text = if s.label.is_empty() { name } else { s.label.clone() };
            view! {
                <g class=class role="img" aria-label=label>
                    <rect x=px y=py width=pw height=ph fill=terrain_pattern(t) class="terrain-rect"></rect>
                    {tag(text)}
                </g>
            }
            .into_any()
        }
        ShapeKind::Hazard => view! {
            <g class=class role="img" aria-label=label>
                <rect x=px y=py width=pw height=ph fill="url(#pat-hazard)" class="hazard-rect"></rect>
                <path d=format!("M{} {}l9 -16l9 16z", px + 4, py + 22) class="hazard-sign"></path>
                <text x=px + 13 y=py + 18 class="hazard-mark">"!"</text>
                {(!s.label.is_empty()).then(|| view! { <text x=px + 26 y=py + 18 class="zone-label">{s.label.clone()}</text> })}
            </g>
        }
        .into_any(),
        ShapeKind::Object(o) => view! {
            <g class=class role="img" aria-label=label>
                <rect x=px y=py width=CELL height=CELL class="object-hit"></rect>
                {object_glyph(o, px, py)}
            </g>
        }
        .into_any(),
        ShapeKind::Template(t) => {
            let moved = Shape {
                x,
                y,
                ..s.clone()
            };
            let cells = moved
                .cells(w, h)
                .into_iter()
                .map(|(cx, cy)| {
                    view! { <rect x=i32::from(cx) * CELL y=i32::from(cy) * CELL width=CELL height=CELL class="template-cell" style=fill.clone()></rect> }
                })
                .collect_view();
            let (ox, oy) = (px + CELL / 2, py + CELL / 2);
            let pointer = matches!(t.area, Area::Cone | Area::Line).then(|| {
                let a = f64::from(t.angle).to_radians();
                let len = f64::from(i32::from(t.feet / 5) * CELL);
                view! { <line x1=ox y1=oy x2=ox + (a.cos() * len) as i32 y2=oy + (a.sin() * len) as i32 class="template-axis"></line> }
            });
            let text = format!("{} ft", t.feet);
            view! {
                <g class=class role="img" aria-label=label>
                    {cells}
                    {pointer}
                    <circle cx=ox cy=oy r="7" class="template-origin" style=fill></circle>
                    <text x=ox + 10 y=oy - 8 class="zone-label">{text}</text>
                </g>
            }
            .into_any()
        }
        ShapeKind::Wall | ShapeKind::Zone => view! {
            <g class=class role="img" aria-label=label>
                <rect x=px y=py width=pw height=ph style=fill></rect>
                {(!s.label.is_empty()).then(|| tag(s.label.clone()))}
            </g>
        }
        .into_any(),
    }
}

#[component]
pub fn BattleMap(m: MapCtx) -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let found = Memo::new(move |_| {
        m.encounter.with(|e| {
            e.as_ref()
                .map(|e| (e.id, e.width, e.height, e.active, e.locked, e.live_drag))
        })
    });
    let shapes = Memo::new(move |_| {
        m.encounter
            .with(|e| e.as_ref().map(|e| e.shapes.clone()).unwrap_or_default())
    });
    let previews = RwSignal::new(HashMap::<ShapeId, ((i16, i16), (i16, i16))>::new());
    Effect::new(move |_| {
        let Some(p) = ctx.preview.get() else {
            return;
        };
        let Some((id, ..)) = found.get_untracked() else {
            return;
        };
        if p.campaign_id != m.campaign || p.encounter != id {
            return;
        }
        let base =
            shapes.with_untracked(|l| l.iter().find(|s| s.id == p.shape).map(|s| (s.x, s.y)));
        if let Some(base) = base {
            previews.update(|map| {
                map.insert(p.shape, ((p.x, p.y), base));
            });
        }
    });
    let drag = RwSignal::new(None::<Drag>);
    let selected = RwSignal::new(None::<ShapeId>);
    let tool = RwSignal::new(Tool::Select);
    let zoom = RwSignal::new(32_u16);

    let can_move = move |s: &Shape| {
        if !m.interactive {
            return false;
        }
        if m.is_dm.get_untracked() {
            return true;
        }
        let Some((_, _, _, active, locked, _)) = found.get_untracked() else {
            return false;
        };
        active && !locked && s.owner.is_some() && s.owner == m.me.get_untracked()
    };
    let position = move |s: &Shape| -> (i16, i16) {
        if let Some(d) = drag.get().filter(|d| d.shape == s.id) {
            return d.at;
        }
        previews.with(|map| match map.get(&s.id) {
            Some((at, base)) if *base == (s.x, s.y) => *at,
            _ => (s.x, s.y),
        })
    };
    let cell_of = move |ev: &leptos::ev::PointerEvent| -> Option<(i16, i16)> {
        let (_, w, h, ..) = found.get_untracked()?;
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
        let Some((encounter, ..)) = found.get_untracked() else {
            return;
        };
        let preview = Preview {
            campaign_id: m.campaign,
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
        let Some((encounter, w, h, ..)) = found.get_untracked() else {
            return;
        };
        let fit = |n: u8, room: i16| n.min(u8::try_from(room.max(1)).unwrap_or(u8::MAX));
        let room_x = i16::try_from(w).unwrap_or(i16::MAX) - cell.0;
        let room_y = i16::try_from(h).unwrap_or(i16::MAX) - cell.1;
        let dm = m.is_dm.get_untracked();
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
            note: String::new(),
        };
        let shape = match t {
            Tool::Select => return,
            Tool::Place(kind @ ShapeKind::Template(_)) => {
                let owner = if dm { None } else { m.me.get_untracked() };
                let colour = owner
                    .and_then(|o| player_colour(m.campaign, o))
                    .unwrap_or(Colour::Palette(5));
                tool.set(Tool::Select);
                Shape {
                    owner,
                    w: 1,
                    h: 1,
                    ..base(kind, 1, colour)
                }
            }
            Tool::Place(_) if !dm => return,
            Tool::Place(kind) => match kind {
                ShapeKind::Wall => base(kind, 1, Colour::Palette(0)),
                ShapeKind::Object(_) => base(kind, 1, Colour::Palette(6)),
                ShapeKind::Hazard => base(kind, 2, Colour::Palette(1)),
                _ => base(kind, 2, Colour::Palette(3)),
            },
            Tool::Token(cid) => {
                let token = ctx.view.with_untracked(|v| {
                    let c = v.campaigns.iter().find(|c| c.campaign.id == m.campaign)?;
                    let e = c.campaign.encounters.iter().find(|e| e.id == encounter)?;
                    let k = e.combatants.iter().find(|k| k.id == cid)?;
                    let owner: Option<UserId> = match k.source {
                        beholder_core::combat::Source::Character(ch) => c
                            .campaign
                            .characters
                            .iter()
                            .find(|x| x.id == ch)
                            .map(|x| x.owner),
                        _ => None,
                    };
                    Some((k.name.clone(), k.hidden, owner))
                });
                tool.set(Tool::Select);
                let Some((name, hidden, owner)) = token else {
                    return;
                };
                let colour = owner
                    .and_then(|o| player_colour(m.campaign, o))
                    .unwrap_or(Colour::Palette(1));
                Shape {
                    label: name,
                    hidden,
                    combatant: Some(cid),
                    owner,
                    ..base(ShapeKind::Token, 1, colour)
                }
            }
        };
        spawn_local_scoped(async move {
            if let Some(id) = fresh::<ShapeId>(1).await.into_iter().next() {
                m.send(EncounterOp::PutShape {
                    id: encounter,
                    shape: Shape { id, ..shape },
                });
                selected.set(Some(id));
            }
        });
    };

    let down = move |ev: leptos::ev::PointerEvent| {
        let Some(cell) = cell_of(&ev) else {
            return;
        };
        let t = tool.get_untracked();
        if m.interactive && t != Tool::Select {
            place(cell, t);
            return;
        }
        let found_shape = shapes.with_untracked(|l| {
            l.iter()
                .filter(|s| hit(s, cell))
                .min_by_key(|s| rank(s.kind))
                .cloned()
        });
        selected.set(found_shape.as_ref().map(|s| s.id));
        if let Some(acting) = m.acting
            && acting.get_untracked().is_some()
            && let Some(cid) = found_shape.as_ref().and_then(|s| s.combatant)
        {
            acting.set(Some(cid));
        }
        let Some(s) = found_shape.filter(|s| can_move(s)) else {
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
        let live = found.get_untracked().is_some_and(|f| f.5);
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
        if d.at != d.from
            && let Some((encounter, ..)) = found.get_untracked()
        {
            m.send(EncounterOp::MoveShape {
                id: encounter,
                shape: d.shape,
                x: d.at.0,
                y: d.at.1,
            });
        }
    };

    let drawn = move || {
        let Some((_, w, h, ..)) = found.get() else {
            return Vec::new();
        };
        let mut list = shapes.get();
        list.sort_by_key(|s| std::cmp::Reverse(rank(s.kind)));
        let mine = m.me.get();
        let sel = selected.get();
        list.into_iter()
            .map(|s| {
                let mut class = match s.kind {
                    ShapeKind::Token => "token",
                    ShapeKind::Wall => "wall",
                    ShapeKind::Zone => "zone",
                    ShapeKind::Terrain(_) => "terrain",
                    ShapeKind::Hazard => "hazard",
                    ShapeKind::Object(_) => "object",
                    ShapeKind::Template(_) => "template",
                }
                .to_owned();
                if s.hidden {
                    class.push_str(" hidden-shape");
                }
                if Some(s.id) == sel {
                    class.push_str(" selected");
                }
                if s.owner.is_some() && s.owner == mine {
                    class.push_str(" mine");
                }
                draw_shape(i18n, &s, position(&s), (w, h), class)
            })
            .collect::<Vec<_>>()
    };

    let board = move || {
        let (_, w, h, ..) = found.get()?;
        let (pw, ph) = (i32::from(w) * CELL, i32::from(h) * CELL);
        let z = u32::from(zoom.get());
        Some(view! {
            <div class="map-scroll">
                <svg class="battle-map" viewBox=format!("0 0 {pw} {ph}")
                    width=u32::from(w) * z height=u32::from(h) * z
                    role="application" aria-label=move || i18n.t("map-label")
                    class:placing=move || tool.get() != Tool::Select
                    on:pointerdown=down on:pointermove=moved on:pointerup=up on:pointercancel=up>
                    {patterns()}
                    <rect class="map-bg" width=pw height=ph></rect>
                    <path class="map-grid" d=grid_path(w, h)></path>
                    {drawn}
                </svg>
            </div>
        })
    };

    let hint = move || {
        let (_, _, _, active, locked, live) = found.get()?;
        let key = if m.is_dm.get() {
            None
        } else if !active {
            Some("map-inactive")
        } else if locked {
            Some("map-locked-hint")
        } else if shapes.with(|l| l.iter().any(|s| s.owner.is_some() && s.owner == m.me.get())) {
            Some(if live {
                "map-drag-live"
            } else {
                "map-drag-hint"
            })
        } else {
            None
        };
        key.map(|k| view! { <span class="muted small">{move || i18n.t(k)}</span> })
    };

    view! {
        <div class="map-pane">
            <div class="toolbar map-tools" role="group" aria-label=move || i18n.t("map-zoom")>
                <button type="button" aria-label=move || i18n.t("map-zoom-out")
                    on:click=move |_| zoom.update(|z| *z = (*z - 8).max(16))>"−"</button>
                <button type="button" aria-label=move || i18n.t("map-zoom-in")
                    on:click=move |_| zoom.update(|z| *z = (*z + 8).min(72))>"+"</button>
                {hint}
            </div>
            <MapTools m=m tool=tool/>
            {board}
            <ShapePanel m=m selected=selected/>
        </div>
    }
}
