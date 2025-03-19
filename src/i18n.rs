use std::collections::HashMap;
use fluent::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;
use accept_language::parse;


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
    bundles: HashMap<String, FluentBundle<FluentResource>>,
}

impl I18n {
    pub fn new(locales: &[&str]) -> Self {
        let mut bundles = HashMap::new();
        for &locale in locales {
            let lang_id: LanguageIdentifier = locale.parse().expect("Invalid locale identifier");
            let ftl_path = format!("locales/{}.ftl", locale);
            let ftl_content = std::fs::read_to_string(&ftl_path).expect("Failed to read FTL file");

            let resource = FluentResource::try_new(ftl_content).expect("Failed to parse Fluent file");
            let mut bundle = FluentBundle::new(vec![lang_id]);
            bundle.add_resource(resource).expect("Failed to add resource");
            bundles.insert(locale.to_string(), bundle);
        }
        Self { bundles }
    }

    pub fn get_all_translations(&self, lang: &str, keys: &[&str]) -> HashMap<String, String> {
        let bundle = self.bundles.get(lang).unwrap_or_else(|| self.bundles.get("en-US").unwrap());
        let mut translations = HashMap::new();

        for &key in keys {
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
