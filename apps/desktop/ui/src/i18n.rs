use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use leptos::prelude::*;
use unic_langid::LanguageIdentifier;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Fr,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fr => "fr",
        }
    }

    pub fn from_code(code: &str) -> Self {
        if code.to_ascii_lowercase().starts_with("fr") {
            Self::Fr
        } else {
            Self::En
        }
    }

    pub fn other(self) -> Self {
        match self {
            Self::En => Self::Fr,
            Self::Fr => Self::En,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::En => 0,
            Self::Fr => 1,
        }
    }
}

thread_local! {
    static BUNDLES: [FluentBundle<FluentResource>; 2] = [
        bundle("en", include_str!("../locales/en/main.ftl")),
        bundle("fr", include_str!("../locales/fr/main.ftl")),
    ];
}

fn bundle(id: &str, source: &str) -> FluentBundle<FluentResource> {
    let lang: LanguageIdentifier = id.parse().unwrap_or_default();
    let mut b = FluentBundle::new(vec![lang]);
    b.set_use_isolating(false);
    let res = FluentResource::try_new(source.to_owned()).unwrap_or_else(|(r, _)| r);
    let _ = b.add_resource(res);
    b
}

#[derive(Clone, Copy)]
pub struct I18n(RwSignal<Lang>);

impl I18n {
    pub fn new(lang: Lang) -> Self {
        Self(RwSignal::new(lang))
    }

    pub fn lang(&self) -> Lang {
        self.0.get()
    }

    pub fn set(&self, lang: Lang) {
        self.0.set(lang);
    }

    pub fn t(&self, key: &str) -> String {
        self.tr(key, &[])
    }

    pub fn tr(&self, key: &str, args: &[(&str, FluentValue<'_>)]) -> String {
        let lang = self.0.get();
        BUNDLES.with(|b| {
            let bundle = &b[lang.index()];
            let Some(pattern) = bundle.get_message(key).and_then(|m| m.value()) else {
                return key.to_owned();
            };
            let mut fa = FluentArgs::new();
            for (k, v) in args {
                fa.set(*k, v.clone());
            }
            let mut errors = Vec::new();
            bundle
                .format_pattern(pattern, Some(&fa), &mut errors)
                .into_owned()
        })
    }
}

pub fn use_i18n() -> I18n {
    expect_context::<I18n>()
}
