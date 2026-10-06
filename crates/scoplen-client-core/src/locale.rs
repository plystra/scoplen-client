// SPDX-License-Identifier: Apache-2.0

//! Interface languages and their selection from the system's preferences.

use serde::{Deserialize, Serialize};
use specta::Type;

/// A language the interface is written in (`11-client-architecture.md` §4).
///
/// English is the source language; every other language is written from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub enum Locale {
    /// English, the source language.
    #[serde(rename = "en")]
    En,
    /// Simplified Chinese.
    #[serde(rename = "zh-Hans")]
    ZhHans,
}

impl Locale {
    /// Every supported language, source language first.
    pub const ALL: [Locale; 2] = [Locale::En, Locale::ZhHans];

    /// The BCP 47 tag used for this language in messages and markup.
    pub fn tag(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::ZhHans => "zh-Hans",
        }
    }

    /// Chooses the interface language from the system's preferred languages,
    /// most preferred first.
    ///
    /// The first preference that names a supported language wins. Chinese
    /// written in Simplified script is recognized whether the system states the
    /// script (`zh-Hans`) or only a region that uses it (`zh-CN`, `zh-SG`);
    /// Traditional Chinese (`zh-Hant`, `zh-TW`, `zh-HK`, `zh-MO`) is not
    /// supported and is skipped. With no supported preference, English is used.
    pub fn negotiate<S: AsRef<str>>(preferred: &[S]) -> Locale {
        preferred.iter().find_map(|tag| Self::match_tag(tag.as_ref())).unwrap_or(Locale::En)
    }

    fn match_tag(tag: &str) -> Option<Locale> {
        // POSIX locales use `_` and may carry an encoding or modifier.
        let tag = tag.split(['.', '@']).next().unwrap_or_default();
        let mut subtags = tag.split(['-', '_']).map(str::to_ascii_lowercase);
        let language = subtags.next()?;
        let rest: Vec<String> = subtags.collect();
        match language.as_str() {
            "en" => Some(Locale::En),
            "zh" => {
                let script = rest.iter().find(|s| s.len() == 4);
                let region = rest.iter().find(|s| s.len() == 2 || s.len() == 3);
                match (script.map(String::as_str), region.map(String::as_str)) {
                    (Some("hans"), _) => Some(Locale::ZhHans),
                    (Some(_), _) => None,
                    (None, Some("tw" | "hk" | "mo")) => None,
                    (None, _) => Some(Locale::ZhHans),
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Locale;

    #[test]
    fn first_supported_preference_wins() {
        assert_eq!(Locale::negotiate(&["fr-FR", "zh-CN", "en-US"]), Locale::ZhHans);
        assert_eq!(Locale::negotiate(&["en-GB", "zh-Hans-CN"]), Locale::En);
    }

    #[test]
    fn simplified_chinese_is_recognized_in_every_form() {
        for tag in ["zh", "zh-Hans", "zh-Hans-CN", "zh-CN", "zh_CN.UTF-8", "zh-SG", "ZH-hans"] {
            assert_eq!(Locale::negotiate(&[tag]), Locale::ZhHans, "{tag}");
        }
    }

    #[test]
    fn traditional_chinese_falls_through() {
        for tag in ["zh-Hant", "zh-Hant-TW", "zh-TW", "zh_HK", "zh-MO"] {
            assert_eq!(Locale::negotiate(&[tag, "en"]), Locale::En, "{tag}");
            assert_eq!(Locale::negotiate(&[tag]), Locale::En, "{tag}");
        }
        assert_eq!(Locale::negotiate(&["zh-Hant", "zh-CN"]), Locale::ZhHans);
    }

    #[test]
    fn unsupported_or_empty_preferences_use_english() {
        assert_eq!(Locale::negotiate::<&str>(&[]), Locale::En);
        assert_eq!(Locale::negotiate(&["", "ja-JP", "C", "POSIX"]), Locale::En);
    }

    #[test]
    fn tags_match_serialized_names() {
        for locale in Locale::ALL {
            let json = serde_json::to_string(&locale).unwrap();
            assert_eq!(json, format!("\"{}\"", locale.tag()));
            assert_eq!(serde_json::from_str::<Locale>(&json).unwrap(), locale);
        }
    }
}
