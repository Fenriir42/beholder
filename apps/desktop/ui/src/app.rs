use std::cell::Cell;
use std::rc::Rc;

use beholder_core::{CampaignId, ValidationError};
use beholder_ipc::{Boot, CompendiumStatus, ConnState, IpcError, Rejected, ViewData, events};
use beholder_protocol::RejectReason;
use beholder_protocol::http::{AuthError, ImageError};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::i18n::{I18n, Lang, use_i18n};
use crate::screens::{AuthScreen, CampaignScreen, LibraryScreen, ServerScreen};
use crate::{icons, ipc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Loading,
    Server,
    Auth,
    Library,
    Campaign(CampaignId),
}

#[derive(Clone, Copy)]
pub struct AppCtx {
    pub screen: RwSignal<Screen>,
    pub boot: RwSignal<Boot>,
    pub view: RwSignal<ViewData>,
    pub conn: RwSignal<ConnState>,
    pub toast: RwSignal<Option<&'static str>>,
    pub compendium: RwSignal<Option<CompendiumStatus>>,
    pub available_sources: RwSignal<Option<Vec<beholder_protocol::http::SourceInfo>>>,
    refresh: StoredValue<Rc<Refresher>, LocalStorage>,
}

#[derive(Default)]
struct Refresher {
    in_flight: Cell<bool>,
    dirty: Cell<bool>,
}

impl AppCtx {
    pub fn refresh(&self) {
        let r = self.refresh.get_value();
        if r.in_flight.replace(true) {
            r.dirty.set(true);
            return;
        }
        let view = self.view;
        spawn_local(async move {
            loop {
                r.dirty.set(false);
                if let Ok(v) = ipc::call0::<ViewData>("view").await {
                    view.set(v);
                }
                if !r.dirty.get() {
                    break;
                }
            }
            r.in_flight.set(false);
        });
    }

    pub fn notify(&self, key: &'static str) {
        let toast = self.toast;
        toast.set(Some(key));
        set_timeout(
            move || {
                if toast.get_untracked() == Some(key) {
                    toast.set(None);
                }
            },
            std::time::Duration::from_secs(5),
        );
    }
}

pub fn use_app() -> AppCtx {
    expect_context::<AppCtx>()
}

pub fn error_key(e: IpcError) -> &'static str {
    match e {
        IpcError::Invalid(ValidationError::Length) => "err-length",
        IpcError::Invalid(ValidationError::Character) => "err-character",
        IpcError::BadServerUrl => "err-bad-url",
        IpcError::Auth(AuthError::UsernameTaken) => "err-username-taken",
        IpcError::Auth(AuthError::InvalidCredentials) => "err-credentials",
        IpcError::Auth(AuthError::RateLimited) | IpcError::Image(ImageError::RateLimited) => {
            "err-rate-limited"
        }
        IpcError::Auth(AuthError::UpdateRequired { .. }) => "err-update",
        IpcError::Image(ImageError::TooLarge | ImageError::TooManyPixels) => "err-image-large",
        IpcError::Image(ImageError::Unsupported) => "err-image-unsupported",
        IpcError::Image(ImageError::Unauthorized) | IpcError::SignedOut => "err-signed-out",
        IpcError::Network => "err-network",
        IpcError::Server => "err-server",
        IpcError::Storage => "err-storage",
        IpcError::BadFile => "err-bad-file",
    }
}

fn reject_key(r: RejectReason) -> &'static str {
    match r {
        RejectReason::Forbidden => "reject-forbidden",
        RejectReason::NotFound => "reject-not-found",
        RejectReason::InvalidInviteCode => "reject-invite",
        RejectReason::Invalid(_) => "reject-invalid",
        RejectReason::UnknownImage => "reject-image",
        RejectReason::IdConflict => "reject-conflict",
        RejectReason::Expired => "reject-expired",
        RejectReason::RateLimited => "err-rate-limited",
        RejectReason::Stale => "reject-stale",
    }
}

#[component]
pub fn App() -> impl IntoView {
    let browser_lang = window().navigator().language().unwrap_or_default();
    let i18n = I18n::new(Lang::from_code(&browser_lang));
    provide_context(i18n);
    let ctx = AppCtx {
        screen: RwSignal::new(Screen::Loading),
        boot: RwSignal::new(Boot::default()),
        view: RwSignal::new(ViewData::default()),
        conn: RwSignal::new(ConnState::Connecting),
        toast: RwSignal::new(None),
        compendium: RwSignal::new(None),
        available_sources: RwSignal::new(None),
        refresh: StoredValue::new_local(Rc::new(Refresher::default())),
    };
    provide_context(ctx);

    ipc::listen(events::STATE, move |s: ConnState| ctx.conn.set(s));
    ipc::listen(events::VIEW_CHANGED, move |(): ()| ctx.refresh());
    ipc::listen(events::REJECTED, move |r: Rejected| {
        ctx.notify(reject_key(r.reason))
    });
    ipc::listen(events::SIGNED_OUT, move |(): ()| {
        ctx.view.set(ViewData::default());
        ctx.screen.set(Screen::Auth);
        ctx.notify("err-signed-out");
    });
    ipc::listen(events::COMPENDIUM, move |s: CompendiumStatus| {
        ctx.compendium.set(Some(s))
    });
    ipc::listen(events::UPDATE_REQUIRED, move |_: u16| {
        ctx.notify("err-update")
    });

    spawn_local(async move {
        let boot = ipc::call0::<Boot>("boot").await.unwrap_or_default();
        if let Some(code) = &boot.locale {
            i18n.set(Lang::from_code(code));
        }
        let next = match (&boot.server_url, boot.signed_in) {
            (None, _) => Screen::Server,
            (Some(_), false) => Screen::Auth,
            (Some(_), true) => Screen::Library,
        };
        ctx.boot.set(boot);
        if next == Screen::Library {
            ctx.refresh();
            if let Ok(s) = ipc::call0::<ConnState>("connection_state").await {
                ctx.conn.set(s);
            }
        }
        ctx.screen.set(next);
    });

    view! {
        {move || match ctx.screen.get() {
            Screen::Loading => ().into_any(),
            Screen::Server => view! { <ServerScreen/> }.into_any(),
            Screen::Auth => view! { <AuthScreen/> }.into_any(),
            Screen::Library => view! { <LibraryScreen/> }.into_any(),
            Screen::Campaign(id) => view! { <CampaignScreen id=id/> }.into_any(),
        }}
        <Toast/>
    }
}

#[component]
fn Toast() -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    move || {
        ctx.toast.get().map(|key| {
            view! {
                <div class="toast" role="status">
                    {icons::alert()}
                    <span>{move || i18n.t(key)}</span>
                </div>
            }
        })
    }
}

#[component]
pub fn Brand() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="brand">
            {icons::logo()}
            <span class="display">{move || i18n.t("app-name")}</span>
        </div>
    }
}

#[component]
pub fn ConnPill() -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let dot = move || match ctx.conn.get() {
        ConnState::Online => "dot online",
        ConnState::Connecting | ConnState::Reconnecting { .. } => "dot warning",
        ConnState::Offline { .. } => "dot error",
    };
    let label = move || match ctx.conn.get() {
        ConnState::Online => i18n.t("conn-online"),
        ConnState::Connecting => i18n.t("conn-connecting"),
        ConnState::Reconnecting { queued } => {
            i18n.tr("conn-reconnecting", &[("queued", queued.into())])
        }
        ConnState::Offline { .. } => i18n.t("conn-offline"),
    };
    view! {
        <span class="pill" role="status" aria-live="polite">
            <span class=dot></span>
            <span>{label}</span>
        </span>
    }
}

#[component]
pub fn ConnBanner() -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let text = move || match ctx.conn.get() {
        ConnState::Reconnecting { .. } => Some(i18n.t("banner-reconnecting")),
        ConnState::Offline { .. } => Some(i18n.t("banner-offline")),
        ConnState::Online | ConnState::Connecting => None,
    };
    let retry = move |_| {
        spawn_local(async {
            let _ = ipc::call0::<()>("retry_now").await;
        })
    };
    move || {
        text().map(|t| {
            view! {
                <div class="banner" role="status">
                    {icons::alert()}
                    <span class="grow">{t}</span>
                    <button class="link" on:click=retry>
                        {icons::refresh()}
                        {move || i18n.t("retry")}
                    </button>
                </div>
            }
        })
    }
}

#[component]
pub fn LangSwitch() -> impl IntoView {
    let i18n = use_i18n();
    let toggle = move |_| {
        let next = i18n.lang().other();
        i18n.set(next);
        spawn_local(async move {
            let _ = ipc::call::<()>(
                "set_locale",
                LocaleArgs {
                    locale: next.code(),
                },
            )
            .await;
        });
    };
    view! {
        <button class="ghost" on:click=toggle aria-label=move || i18n.t("language")>
            {move || i18n.lang().code().to_uppercase()}
        </button>
    }
}

#[derive(serde::Serialize)]
struct LocaleArgs {
    locale: &'static str,
}

#[component]
pub fn TopBar() -> impl IntoView {
    let ctx = use_app();
    let i18n = use_i18n();
    let sign_out = move |_| {
        spawn_local(async move {
            let _ = ipc::call0::<()>("logout").await;
            ctx.view.set(ViewData::default());
            ctx.screen.set(Screen::Auth);
        })
    };
    let name = move || {
        ctx.view
            .get()
            .user
            .map(|u| u.display_name.as_str().to_owned())
            .unwrap_or_default()
    };
    view! {
        <header class="topbar">
            <Brand/>
            <span class="spacer"></span>
            <ConnPill/>
            <LangSwitch/>
            <span class="muted" aria-label=move || i18n.t("account")>{name}</span>
            <button class="ghost" on:click=sign_out>
                {icons::logout()}
                {move || i18n.t("sign-out")}
            </button>
        </header>
        <ConnBanner/>
    }
}

pub fn image_url(ctx: &AppCtx, hash: &beholder_core::ImageHash, variant: &str) -> String {
    format!(
        "{}{variant}/{}",
        ctx.boot.get_untracked().image_base,
        hash.to_hex()
    )
}
