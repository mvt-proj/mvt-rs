use accept_language::parse;
use fluent_bundle::FluentResource;
use fluent_bundle::bundle::FluentBundle;
use fluent_syntax::ast;
use include_dir::{Dir, include_dir};
use intl_memoizer::concurrent::IntlLangMemoizer;
use salvo::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::error;
use unic_langid::LanguageIdentifier;

const LOCALES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/locales");
const DEFAULT_LANG: &str = "en-US";

type SafeBundle = FluentBundle<FluentResource, IntlLangMemoizer>;

#[derive(Clone)]
pub struct I18n {
    bundles: Arc<HashMap<String, (SafeBundle, HashSet<String>)>>,
    fallback_lang: String,
}

impl I18n {
    pub fn new() -> Self {
        let mut bundles = HashMap::new();

        for file in LOCALES_DIR.files() {
            let filename = file
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if !filename.ends_with(".ftl") {
                continue;
            }

            let locale_str = filename.replace(".ftl", "");

            let lang_id: LanguageIdentifier = match locale_str.parse() {
                Ok(id) => id,
                Err(e) => {
                    error!("I18n Error: Invalid locale '{locale_str}': {e}");
                    continue;
                }
            };

            let ftl_content = match file.contents_utf8() {
                Some(c) => c,
                None => {
                    error!("I18n Error: File {filename} is not valid UTF-8.");
                    continue;
                }
            };

            let resource = match FluentResource::try_new(ftl_content.to_string()) {
                Ok(res) => res,
                Err((res, _)) => {
                    error!("I18n Warning: Syntax errors found in {filename}");
                    res
                }
            };

            let mut message_keys = HashSet::new();
            for entry in resource.entries() {
                if let ast::Entry::Message(msg) = entry {
                    message_keys.insert(msg.id.name.to_string());
                }
            }

            let mut bundle = FluentBundle::new_concurrent(vec![lang_id]);
            bundle.set_use_isolating(false);

            if let Err(e) = bundle.add_resource(resource) {
                error!("I18n Error: Failed to add resource {locale_str}: {e:?}");
                continue;
            }

            bundles.insert(locale_str, (bundle, message_keys));
        }

        if !bundles.contains_key(DEFAULT_LANG) {
            error!(
                "I18n CRITICAL: Fallback language '{}' was not loaded.",
                DEFAULT_LANG
            );
        }

        Self {
            bundles: Arc::new(bundles),
            fallback_lang: DEFAULT_LANG.to_string(),
        }
    }

    pub fn resolve_lang(&self, req: &salvo::Request) -> String {
        let requested_langs = req
            .headers()
            .get("Accept-Language")
            .and_then(|h| h.to_str().ok())
            .map(parse)
            .unwrap_or_default();

        for pref in &requested_langs {
            if self.bundles.contains_key(pref) {
                return pref.clone();
            }

            let req_base = pref.split('-').next().unwrap_or(pref);
            for available_key in self.bundles.keys() {
                let available_base = available_key.split('-').next().unwrap_or(available_key);
                if req_base.eq_ignore_ascii_case(available_base) {
                    return available_key.clone();
                }
            }
        }
        self.fallback_lang.clone()
    }

    pub fn get_all_translations(&self, lang: &str) -> HashMap<String, String> {
        let target_lang = if self.bundles.contains_key(lang) {
            lang
        } else {
            &self.fallback_lang
        };

        let Some((bundle, message_keys)) = self.bundles.get(target_lang) else {
            return HashMap::new();
        };

        let mut translations = HashMap::new();
        let mut errors = vec![];

        for key in message_keys {
            if let Some(message) = bundle.get_message(key) {
                if let Some(pattern) = message.value() {
                    let translated = bundle.format_pattern(pattern, None, &mut errors);
                    translations.insert(key.to_string(), translated.to_string());
                }
                errors.clear();
            }
        }

        translations
    }
}

#[handler]
pub async fn i18n_middleware(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
    if let Ok(i18n) = depot.obtain::<Arc<I18n>>() {
        let lang = i18n.resolve_lang(req);
        let translations = i18n.get_all_translations(&lang);

        depot.insert("translate", translations);
        depot.insert("lang", lang);
    } else {
        error!("I18n Middleware: I18n instance not found in Depot.");
    }

    ctrl.call_next(req, depot, res).await;
}
