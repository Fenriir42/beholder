//! Inline SVG icons: 24×24, 1.8 stroke, round caps (SPEC §6). Decorative only;
//! every icon sits next to a text label.

use leptos::prelude::*;

fn svg(paths: &'static str) -> impl IntoView {
    view! { <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" inner_html=paths></svg> }
}

/// The app mark: an eye.
pub fn logo() -> impl IntoView {
    svg(
        r#"<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>"#,
    )
}
/// Add.
pub fn plus() -> impl IntoView {
    svg(r#"<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>"#)
}
/// Invite code.
pub fn key() -> impl IntoView {
    svg(
        r#"<circle cx="7.5" cy="15.5" r="4.5"/><path d="M10.7 12.3 20 3"/><path d="m16 7 3 3"/><path d="m14 9 2 2"/>"#,
    )
}
/// Back.
pub fn back() -> impl IntoView {
    svg(r#"<line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/>"#)
}
/// Retry.
pub fn refresh() -> impl IntoView {
    svg(r#"<path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><polyline points="21 3 21 8 16 8"/>"#)
}
/// Sign out.
pub fn logout() -> impl IntoView {
    svg(
        r#"<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/>"#,
    )
}
/// Warning / offline.
pub fn alert() -> impl IntoView {
    svg(
        r#"<circle cx="12" cy="12" r="9"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>"#,
    )
}
/// Home.
pub fn home() -> impl IntoView {
    svg(r#"<path d="M3 11 12 3l9 8"/><path d="M5 10v10h14V10"/>"#)
}
/// Character sheet.
pub fn sheet() -> impl IntoView {
    svg(
        r#"<rect x="5" y="3" width="14" height="18" rx="2"/><line x1="9" y1="8" x2="15" y2="8"/><line x1="9" y1="12" x2="15" y2="12"/><line x1="9" y1="16" x2="13" y2="16"/>"#,
    )
}
/// Notes.
pub fn notes() -> impl IntoView {
    svg(r#"<path d="M4 20h4L19 9l-4-4L4 16z"/><line x1="13" y1="7" x2="17" y2="11"/>"#)
}
/// Compendium.
pub fn book() -> impl IntoView {
    svg(
        r#"<path d="M4 4h6a2 2 0 0 1 2 2v14a2 2 0 0 0-2-2H4z"/><path d="M20 4h-6a2 2 0 0 0-2 2v14a2 2 0 0 1 2-2h6z"/>"#,
    )
}
/// Combat.
pub fn swords() -> impl IntoView {
    svg(r#"<path d="M14 4h6v6L9 21l-6-6z"/><line x1="6" y1="12" x2="12" y2="18"/>"#)
}
/// Dice.
pub fn dice() -> impl IntoView {
    svg(
        r#"<rect x="4" y="4" width="16" height="16" rx="3"/><circle cx="9" cy="9" r="1"/><circle cx="15" cy="15" r="1"/><circle cx="15" cy="9" r="1"/><circle cx="9" cy="15" r="1"/>"#,
    )
}
/// Image.
pub fn image() -> impl IntoView {
    svg(
        r#"<rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m21 17-5-5-9 7"/>"#,
    )
}
