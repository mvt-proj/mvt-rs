// ISO 19115 codelist tables consumed by the metadata admin UI and API
// (Phase 2 / Work Unit 2). Authored fresh from the public ISO 19115
// standard and its public codelist registers — NOT read, copied, or
// transcribed from any proprietary implementation (see design decision
// #13 and the IP-provenance constraint in the tasks artifact).
//
// Only the two codelists actually referenced by `MetadataRecord` are
// covered: `MD_TopicCategoryCode` (`topic_category`) and `MD_ProgressCode`
// (`status`).
//
// Consumers (`api::metadata`, `html::admin::metadata`) are Phase 3/4, not
// yet wired into the module tree — mirrors the `#[allow(dead_code)]`
// convention in `config/metadata.rs`.
#![allow(dead_code)]

use crate::error::{AppError, AppResult};
use crate::models::metadata::MetadataContact;

/// One entry of an ISO 19115 codelist: the machine `code` (used verbatim
/// as the stored/exchanged value). Display labels are resolved at render
/// time via Fluent i18n — see `topic_category_translate_key` /
/// `progress_code_translate_key` below, which build the `.ftl` message
/// key for a given code (design decision #13 amendment, Phase 2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodelistEntry {
    pub code: &'static str,
}

/// `MD_TopicCategoryCode` — the 19 standard ISO 19115 topic categories.
pub const TOPIC_CATEGORY_CODES: &[CodelistEntry] = &[
    CodelistEntry { code: "farming" },
    CodelistEntry { code: "biota" },
    CodelistEntry { code: "boundaries" },
    CodelistEntry { code: "climatologyMeteorologyAtmosphere" },
    CodelistEntry { code: "economy" },
    CodelistEntry { code: "elevation" },
    CodelistEntry { code: "environment" },
    CodelistEntry { code: "geoscientificInformation" },
    CodelistEntry { code: "health" },
    CodelistEntry { code: "imageryBaseMapsEarthCover" },
    CodelistEntry { code: "intelligenceMilitary" },
    CodelistEntry { code: "inlandWaters" },
    CodelistEntry { code: "location" },
    CodelistEntry { code: "oceans" },
    CodelistEntry { code: "planningCadastre" },
    CodelistEntry { code: "society" },
    CodelistEntry { code: "structure" },
    CodelistEntry { code: "transportation" },
    CodelistEntry { code: "utilitiesCommunication" },
];

/// `MD_ProgressCode` — the core ISO 19115 progress codes.
pub const PROGRESS_CODES: &[CodelistEntry] = &[
    CodelistEntry { code: "completed" },
    CodelistEntry { code: "historicalArchive" },
    CodelistEntry { code: "obsolete" },
    CodelistEntry { code: "onGoing" },
    CodelistEntry { code: "planned" },
    CodelistEntry { code: "required" },
    CodelistEntry { code: "underDevelopment" },
];

fn translate_key_for(table: &[CodelistEntry], prefix: &str, code: &str) -> Option<String> {
    table
        .iter()
        .find(|entry| entry.code == code)
        .map(|_| format!("{prefix}-{code}"))
}

fn is_valid_in(table: &[CodelistEntry], code: &str) -> bool {
    table.iter().any(|entry| entry.code == code)
}

/// Fluent translate key (`topic-category-{code}`) for a `MD_TopicCategoryCode`
/// value, or `None` if unknown. The key must exist in every locale's `.ftl`
/// bundle (see the completeness check in this module's tests / Phase 2.5.4).
pub fn topic_category_translate_key(code: &str) -> Option<String> {
    translate_key_for(TOPIC_CATEGORY_CODES, "topic-category", code)
}

/// Whether `code` is one of the 19 standard `MD_TopicCategoryCode` values.
pub fn is_valid_topic_category(code: &str) -> bool {
    is_valid_in(TOPIC_CATEGORY_CODES, code)
}

/// Fluent translate key (`progress-code-{code}`) for a `MD_ProgressCode`
/// value, or `None` if unknown.
pub fn progress_code_translate_key(code: &str) -> Option<String> {
    translate_key_for(PROGRESS_CODES, "progress-code", code)
}

/// Whether `code` is one of the standard `MD_ProgressCode` values.
pub fn is_valid_progress_code(code: &str) -> bool {
    is_valid_in(PROGRESS_CODES, code)
}

/// `CI_RoleCode` (closed subset) — the six responsible-party roles a
/// `MetadataContact` may declare (spec "Closed role vocabulary
/// enforcement", Work Unit 2).
pub const ROLE_CODES: &[CodelistEntry] = &[
    CodelistEntry { code: "originator" },
    CodelistEntry { code: "pointOfContact" },
    CodelistEntry { code: "metadataAuthor" },
    CodelistEntry { code: "custodian" },
    CodelistEntry { code: "publisher" },
    CodelistEntry { code: "processor" },
];

/// Fluent translate key (`role-code-{code}`) for a `CI_RoleCode` value, or
/// `None` if unknown.
pub fn role_code_translate_key(code: &str) -> Option<String> {
    translate_key_for(ROLE_CODES, "role-code", code)
}

/// Whether `code` is one of the six closed `ROLE_CODES` values.
pub fn is_valid_role_code(code: &str) -> bool {
    is_valid_in(ROLE_CODES, code)
}

/// Validates every contact's `role` against [`ROLE_CODES`] (spec "Closed
/// role vocabulary enforcement"). Rejects with a typed
/// [`AppError::InvalidInput`] on the first invalid role found — mirrors the
/// `guard_layer_published` boundary-validation pattern (design decision #4):
/// the check lives at the application layer, not in `config::metadata` or a
/// SQL CHECK constraint.
pub fn validate_contacts(contacts: &[MetadataContact]) -> AppResult<()> {
    for contact in contacts {
        if !is_valid_role_code(&contact.role) {
            let valid = ROLE_CODES.iter().map(|entry| entry.code).collect::<Vec<_>>().join(", ");
            return Err(AppError::InvalidInput(format!(
                "invalid contact role '{}': must be one of {valid}",
                contact.role
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_category_translate_key_returns_key_for_known_code() {
        assert_eq!(
            topic_category_translate_key("boundaries"),
            Some("topic-category-boundaries".to_string())
        );
    }

    #[test]
    fn topic_category_translate_key_returns_none_for_unknown_code() {
        assert_eq!(topic_category_translate_key("not_a_real_code"), None);
    }

    #[test]
    fn topic_category_codes_table_has_the_nineteen_standard_values() {
        assert_eq!(TOPIC_CATEGORY_CODES.len(), 19);
    }

    #[test]
    fn is_valid_topic_category_accepts_every_code_in_the_table() {
        for entry in TOPIC_CATEGORY_CODES {
            assert!(is_valid_topic_category(entry.code));
        }
        assert!(!is_valid_topic_category("bogus"));
    }

    #[test]
    fn progress_code_translate_key_returns_key_for_known_code() {
        assert_eq!(
            progress_code_translate_key("onGoing"),
            Some("progress-code-onGoing".to_string())
        );
    }

    #[test]
    fn progress_code_translate_key_returns_none_for_unknown_code() {
        assert_eq!(progress_code_translate_key("not_a_real_code"), None);
    }

    #[test]
    fn is_valid_progress_code_accepts_every_code_in_the_table() {
        for entry in PROGRESS_CODES {
            assert!(is_valid_progress_code(entry.code));
        }
        assert!(!is_valid_progress_code("bogus"));
    }

    #[test]
    fn role_codes_table_has_the_six_closed_values() {
        assert_eq!(ROLE_CODES.len(), 6);
        let codes: Vec<&str> = ROLE_CODES.iter().map(|e| e.code).collect();
        assert_eq!(
            codes,
            vec![
                "originator",
                "pointOfContact",
                "metadataAuthor",
                "custodian",
                "publisher",
                "processor",
            ]
        );
    }

    #[test]
    fn role_code_translate_key_returns_key_for_known_code() {
        assert_eq!(
            role_code_translate_key("pointOfContact"),
            Some("role-code-pointOfContact".to_string())
        );
    }

    #[test]
    fn role_code_translate_key_returns_none_for_unknown_code() {
        assert_eq!(role_code_translate_key("reviewer"), None);
    }

    #[test]
    fn is_valid_role_code_accepts_every_code_in_the_table_and_rejects_unknown() {
        for entry in ROLE_CODES {
            assert!(is_valid_role_code(entry.code));
        }
        assert!(!is_valid_role_code("reviewer"));
    }

    fn contact_with_role(role: &str) -> MetadataContact {
        MetadataContact {
            id: "contact-1".to_string(),
            individual_name: Some("Ana Perez".to_string()),
            organisation_name: None,
            position_name: None,
            email: None,
            phone: None,
            role: role.to_string(),
        }
    }

    #[test]
    fn validate_contacts_accepts_empty_list() {
        assert!(validate_contacts(&[]).is_ok());
    }

    #[test]
    fn validate_contacts_accepts_every_closed_role() {
        let contacts: Vec<MetadataContact> =
            ROLE_CODES.iter().map(|entry| contact_with_role(entry.code)).collect();
        assert!(validate_contacts(&contacts).is_ok());
    }

    #[test]
    fn validate_contacts_rejects_unknown_role_with_invalid_input_and_persists_nothing() {
        let contacts = vec![contact_with_role("pointOfContact"), contact_with_role("reviewer")];
        let err = validate_contacts(&contacts).expect_err("role 'reviewer' is not in ROLE_CODES");
        match err {
            AppError::InvalidInput(message) => {
                assert!(message.contains("reviewer"), "error must name the offending role: {message}");
            }
            other => panic!("expected AppError::InvalidInput, got {other:?}"),
        }
    }

    /// Every `.ftl` locale bundle this project ships must define a
    /// `topic-category-<code>` / `progress-code-<code>` message for every
    /// code in the two codelist tables above. Askama's `translate[...]`
    /// HashMap indexing panics at render time on any missing key, so a gap
    /// here is a production panic risk, not just a cosmetic omission.
    ///
    /// Parses each `.ftl` file with the exact same logic `I18n::new()` uses
    /// in production (`crate::i18n::extract_message_keys`), rather than a
    /// fragile string/regex scan, so "key exists" means the same thing here
    /// as it does when the server actually loads translations.
    const LOCALES: &[&str] = &["en-US", "es-AR", "es-ES", "fr-FR", "it-IT", "pt-BR"];

    #[test]
    fn every_topic_category_and_progress_code_has_a_translate_key_in_every_locale() {
        for locale in LOCALES {
            let path = format!("{}/locales/{locale}.ftl", env!("CARGO_MANIFEST_DIR"));
            let content =
                std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
            let keys = crate::i18n::extract_message_keys(&content);

            for entry in TOPIC_CATEGORY_CODES {
                let key = format!("topic-category-{}", entry.code);
                assert!(
                    keys.contains(&key),
                    "locale {locale} is missing Fluent key `{key}`"
                );
            }

            for entry in PROGRESS_CODES {
                let key = format!("progress-code-{}", entry.code);
                assert!(
                    keys.contains(&key),
                    "locale {locale} is missing Fluent key `{key}`"
                );
            }

            for entry in ROLE_CODES {
                let key = format!("role-code-{}", entry.code);
                assert!(
                    keys.contains(&key),
                    "locale {locale} is missing Fluent key `{key}`"
                );
            }
        }
    }
}
