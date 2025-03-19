use include_dir::{include_dir, Dir};
use accept_language::parse;
use fluent::{FluentBundle, FluentResource};
use fluent_syntax::ast;
use salvo::prelude::*;
use std::collections::{HashMap, HashSet};
use unic_langid::LanguageIdentifier;

const LOCALES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/locales");

pub fn get_lang(req: &salvo::Request) -> String {
    let lang = req
        .headers()
        .get("Accept-Language")
        .and_then(|header| header.to_str().ok())
        .and_then(|accept_language| {
            let langs = parse(accept_language);
            langs.into_iter().next().map(|lang| lang.to_string())
        })
        .unwrap_or_else(|| "en-US".to_string());
    lang
}

pub struct I18n {
    bundles: HashMap<String, (FluentBundle<FluentResource>, HashSet<String>)>,
}

impl I18n {

    pub fn new(locales: &[&str]) -> Self {
        let mut bundles = HashMap::new();
        let mut locales_to_load = vec!["en-US"];
        locales_to_load.extend(locales.iter().filter(|&&l| l != "en-US"));

        for &locale in locales_to_load.iter() {
            let lang_id: LanguageIdentifier = match locale.parse() {
                Ok(id) => id,
                Err(_) => {
                    if locale == "en-US" {
                        panic!("Invalid en-US locale identifier");
                    }
                    continue;
                }
            };

            let ftl_file = match LOCALES_DIR.get_file(format!("{}.ftl", locale)) {
                Some(file) => file,
                None => {
                    if locale == "en-US" {
                        panic!("en-US.ftl not found in embedded locales");
                    }
                    continue;
                }
            };

            let ftl_content = ftl_file
                .contents_utf8()
                .expect("Invalid UTF-8 FTL content");

            let resource = match FluentResource::try_new(ftl_content.to_string()) {
                Ok(res) => res,
                Err(_) => {
                    if locale == "en-US" {
                        panic!("Invalid en-US Fluent file");
                    }
                    continue;
                }
            };

            let mut message_keys = HashSet::new();
            for entry in resource.entries() {
                if let ast::Entry::Message(msg) = entry {
                    message_keys.insert(msg.id.name.to_string());
                }
            }

            let mut bundle = FluentBundle::new(vec![lang_id]);
            if let Err(e) = bundle.add_resource(resource) {
                if locale == "en-US" {
                    panic!("Failed to add en-US resource: {:?}", e);
                }
                continue;
            }

            bundles.insert(locale.to_string(), (bundle, message_keys));
        }

        Self { bundles }
    }

    pub fn get_all_translations(&self, lang: &str) -> HashMap<String, String> {
        let (bundle, message_keys) = self
            .bundles
            .get(lang)
            .unwrap_or_else(|| self.bundles.get("en-US").unwrap());

        let mut translations = HashMap::new();

        for key in message_keys {
            let mut errors = vec![];
            if let Some(message) = bundle.get_message(key) {
                if let Some(pattern) = message.value() {
                    let translated = bundle.format_pattern(pattern, None, &mut errors);
                    translations.insert(key.to_string(), translated.to_string());
                }
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
    let translate = {
        let lang = get_lang(req);
        let i18n = I18n::new(&[&lang]);
        i18n.get_all_translations(&lang)
    };
    depot.insert("translate".to_string(), translate);
    ctrl.call_next(req, depot, res).await;
}
