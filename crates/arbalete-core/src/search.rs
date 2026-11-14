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

    /// The color of its chip in the Command Bar, searching it as a site.
    pub fn color(self) -> &'static str {
        match self {
            Self::Google => "#4285F4",
            Self::DuckDuckGo => "#DE5833",
            Self::Bing => "#0C8484",
            Self::Ecosia => "#008009",
            Self::Qwant => "#5C97FF",
            Self::Brave => "#FB542B",
            Self::Perplexity => "#20808D",
            Self::Yahoo => "#6001D2",
        }
    }

    /// The address of its suggestions for typed text, before the percent-encoded text, if it
    /// offers any. Each answers in the OpenSearch format (suggestions).
    pub fn suggest_url(self) -> Option<&'static str> {
        match self {
            Self::Google => {
                Some("https://suggestqueries.google.com/complete/search?client=firefox&q=")
            }
            Self::DuckDuckGo => Some("https://duckduckgo.com/ac/?type=list&q="),
            Self::Bing => Some("https://api.bing.com/osjson.aspx?query="),
            Self::Ecosia => Some("https://ac.ecosia.org/autocomplete?type=list&q="),
            Self::Qwant => Some("https://api.qwant.com/api/suggest/?client=opensearch&q="),
            Self::Brave => Some("https://search.brave.com/api/suggest?q="),
            Self::Perplexity => None,
            Self::Yahoo => Some("https://ff.search.yahoo.com/gossip?output=fxjson&command="),
        }
    }
}

/// A site the Command Bar searches once Tab follows its name or shortcut, like Arc's site search:
/// its name, its shortcut (usually its host), the address of a search with `%s` where the
/// percent-encoded query goes, and the color of its chip.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Site {
    pub name: String,
    pub shortcut: String,
    pub url: String,
    pub color: String,
}

impl Site {
    fn new(name: &str, shortcut: &str, url: &str, color: &str) -> Self {
        Self {
            name: name.to_string(),
            shortcut: shortcut.to_string(),
            url: url.to_string(),
            color: color.to_string(),
        }
    }

    /// The address searching `query` on it; without `%s`, the query goes at the end.
    pub fn search_url(&self, query: &str) -> String {
        let query = encode(query);
        if self.url.contains("%s") {
            self.url.replace("%s", &query)
        } else {
            format!("{}{query}", self.url)
        }
    }
}

/// The sites searched until the settings change them, popular ones with a public search.
pub fn default_sites() -> Vec<Site> {
    [
        (
            "YouTube",
            "youtube.com",
            "https://www.youtube.com/results?search_query=%s",
            "#FF0033",
        ),
        (
            "Wikipedia",
            "wikipedia.org",
            "https://en.wikipedia.org/w/index.php?search=%s",
            "#E8A33D",
        ),
        (
            "Google Drive",
            "drive.google.com",
            "https://drive.google.com/drive/search?q=%s",
            "#1FA463",
        ),
        (
            "Gmail",
            "mail.google.com",
            "https://mail.google.com/mail/u/0/#search/%s",
            "#EA4335",
        ),
        (
            "Google Maps",
            "maps.google.com",
            "https://www.google.com/maps/search/%s",
            "#34A853",
        ),
        (
            "Google Docs",
            "docs.google.com",
            "https://docs.google.com/document/?q=%s",
            "#4285F4",
        ),
        (
            "Amazon",
            "amazon.com",
            "https://www.amazon.com/s?k=%s",
            "#FF9900",
        ),
        (
            "GitHub",
            "github.com",
            "https://github.com/search?q=%s",
            "#6E40C9",
        ),
        (
            "Reddit",
            "reddit.com",
            "https://www.reddit.com/search/?q=%s",
            "#FF4500",
        ),
        ("X", "x.com", "https://x.com/search?q=%s", "#000000"),
        (
            "LinkedIn",
            "linkedin.com",
            "https://www.linkedin.com/search/results/all/?keywords=%s",
            "#0A66C2",
        ),
        (
            "Spotify",
            "open.spotify.com",
            "https://open.spotify.com/search/%s",
            "#1DB954",
        ),
        (
            "Stack Overflow",
            "stackoverflow.com",
            "https://stackoverflow.com/search?q=%s",
            "#F48024",
        ),
        (
            "Figma",
            "figma.com",
            "https://www.figma.com/files/search?q=%s",
            "#A259FF",
        ),
        (
            "Dropbox",
            "dropbox.com",
            "https://www.dropbox.com/search/personal?query=%s",
            "#0061FF",
        ),
    ]
    .into_iter()
    .map(|(name, shortcut, url, color)| Site::new(name, shortcut, url, color))
    .collect()
}

/// The search engines as sites, searched like the others.
pub fn engine_sites() -> Vec<Site> {
    SearchEngine::ALL
        .iter()
        .map(|engine| {
            let host = engine.url().trim_start_matches("https://");
            let host = host
                .trim_start_matches("www.")
                .split('/')
                .next()
                .unwrap_or("");
            Site::new(engine.name(), host, engine.url(), engine.color())
        })
        .collect()
}

/// The site, among `sites`, whose name or shortcut starts with what is typed, from two letters
/// on, or whose name it is, like "x".
pub fn find<'a>(sites: &'a [Site], typed: &str) -> Option<&'a Site> {
    let typed = typed.trim().to_lowercase();
    if typed.is_empty() || typed.contains(char::is_whitespace) {
        return None;
    }
    let long = typed.chars().count() >= 2;
    sites.iter().find(|site| {
        let name = site.name.to_lowercase().replace(' ', "");
        let shortcut = site.shortcut.to_lowercase();
        let shortcut = shortcut.trim_start_matches("www.");
        !site.url.is_empty()
            && (name == typed || long && (name.starts_with(&typed) || shortcut.starts_with(&typed)))
    })
}

/// Text percent-encoded for a query, as `encodeURIComponent` does in the UI: what is not a letter,
/// a digit or one of `-_.!~*'()` goes as its UTF-8 bytes.
pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => (byte as char).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// The suggestions of an engine's OpenSearch answer, `["typed text", ["suggestion", …], …]`:
/// none when it is not one.
pub fn suggestions(answer: &str) -> Vec<String> {
    let answer: serde_json::Value = serde_json::from_str(answer).unwrap_or_default();
    let list = answer.get(1).and_then(serde_json::Value::as_array);
    list.into_iter()
        .flatten()
        .filter_map(|suggestion| suggestion.as_str().map(str::to_string))
        .collect()
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

    #[test]
    fn finds_the_site_named_or_shortcut_by_what_is_typed() {
        let sites: Vec<_> = engine_sites().into_iter().chain(default_sites()).collect();
        let name = |typed| find(&sites, typed).map(|site| site.name.as_str());
        assert_eq!(name("you"), Some("YouTube"));
        assert_eq!(name("YouTube.com"), Some("YouTube"));
        assert_eq!(name("stackov"), Some("Stack Overflow"));
        assert_eq!(name("go"), Some("Google"));
        assert_eq!(name("gi"), Some("GitHub"));
        assert_eq!(name("search.brave"), Some("Brave"));
        assert_eq!(name("y"), None);
        assert_eq!(name("x"), Some("X"));
        assert_eq!(name("you tube"), None);
        assert_eq!(name("youtube.com/watch"), None);
        assert_eq!(name("zzz"), None);
    }

    #[test]
    fn puts_the_query_where_the_site_says() {
        let site = Site::new("Maps", "maps", "https://example.com/s/%s?x=1", "#000000");
        assert_eq!(site.search_url("a b"), "https://example.com/s/a%20b?x=1");
        let site = Site::new("Old", "old", "https://example.com/?q=", "#000000");
        assert_eq!(site.search_url("a"), "https://example.com/?q=a");
    }

    #[test]
    fn encodes_queries_like_the_ui() {
        assert_eq!(
            encode("rust lang & café?"),
            "rust%20lang%20%26%20caf%C3%A9%3F"
        );
        assert_eq!(encode("a-b_c.d!~*'()"), "a-b_c.d!~*'()");
    }

    #[test]
    fn reads_the_suggestions_of_an_opensearch_answer() {
        let answer = r#"["rust",["rustdesk","rust language"],[],{"google:suggesttype":[]}]"#;
        assert_eq!(suggestions(answer), ["rustdesk", "rust language"]);
        assert!(suggestions("not json").is_empty());
        assert!(suggestions(r#"{"rust":1}"#).is_empty());
        assert!(
            SearchEngine::ALL
                .iter()
                .filter_map(|e| e.suggest_url())
                .all(|u| u.starts_with("https://"))
        );
    }
}
