//! The search engines a profile searches what is typed in the Command Bar with.

use serde::{Deserialize, Serialize};

/// Arc's list of engines, Google first, its default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchEngine {
    #[default]
    Google,
    DuckDuckGo,
    Bing,
    Ecosia,
    Qwant,
    Brave,
    Perplexity,
    Yahoo,
}

impl SearchEngine {
    pub const ALL: [Self; 8] = [
        Self::Google,
        Self::DuckDuckGo,
        Self::Bing,
        Self::Ecosia,
        Self::Qwant,
        Self::Brave,
        Self::Perplexity,
        Self::Yahoo,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::DuckDuckGo => "DuckDuckGo",
            Self::Bing => "Bing",
            Self::Ecosia => "Ecosia",
            Self::Qwant => "Qwant",
            Self::Brave => "Brave",
            Self::Perplexity => "Perplexity",
            Self::Yahoo => "Yahoo",
        }
    }

    /// The address of a search, before its percent-encoded query.
    pub fn url(self) -> &'static str {
        match self {
            Self::Google => "https://www.google.com/search?q=",
            Self::DuckDuckGo => "https://duckduckgo.com/?q=",
            Self::Bing => "https://www.bing.com/search?q=",
            Self::Ecosia => "https://www.ecosia.org/search?q=",
            Self::Qwant => "https://www.qwant.com/?q=",
            Self::Brave => "https://search.brave.com/search?q=",
            Self::Perplexity => "https://www.perplexity.ai/search?q=",
            Self::Yahoo => "https://search.yahoo.com/search?p=",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_each_engine_in_lowercase_for_the_ui() {
        for engine in SearchEngine::ALL {
            let id = serde_json::to_string(&engine).unwrap();
            assert_eq!(id, format!("\"{}\"", engine.name().to_lowercase()));
            assert!(engine.url().starts_with("https://"));
        }
        assert_eq!(SearchEngine::default(), SearchEngine::Google);
    }
}
