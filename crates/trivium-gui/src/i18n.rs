//! Lightweight runtime localization (English / Spanish) with OS locale detection.

use trivium_core::config::UiLanguage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
}

pub fn detect_os_lang() -> Lang {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(val) = std::env::var(var) {
            let trimmed = val.trim().to_ascii_lowercase();
            if !trimmed.is_empty() && trimmed != "c" && trimmed != "posix" {
                return if trimmed.starts_with("es") {
                    Lang::Es
                } else {
                    Lang::En
                };
            }
        }
    }
    Lang::En
}

pub fn resolve_lang(pref: UiLanguage) -> Lang {
    match pref {
        UiLanguage::Auto => detect_os_lang(),
        UiLanguage::En => Lang::En,
        UiLanguage::Es => Lang::Es,
    }
}

#[inline]
pub fn t<'a>(lang: Lang, es: &'a str, en: &'a str) -> &'a str {
    match lang {
        Lang::Es => es,
        Lang::En => en,
    }
}
