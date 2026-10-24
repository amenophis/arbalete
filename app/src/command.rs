//! Commands the UI sends to the app, as `https://arbalette.ui/command/<name>` requests.

use arbalette_core::sidebar::NodeId;

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// The UI is listening: send it the current state.
    Ready,
    Back,
    Forward,
    Reload,
    /// Shows a tab in the content view: `select/<tab id>`.
    Select(NodeId),
    ShowOverlay,
    HideOverlay,
    /// Opens a tab: `open/<percent-encoded url>`.
    Open(String),
    /// Closes a tab, `close/<tab id>`, or the selected one, `close`.
    Close(Option<NodeId>),
    /// Reopens the last closed tab.
    Reopen,
}

impl Command {
    pub fn parse(path: &str) -> Option<Self> {
        match path {
            "ready" => Some(Self::Ready),
            "back" => Some(Self::Back),
            "forward" => Some(Self::Forward),
            "reload" => Some(Self::Reload),
            "overlay/show" => Some(Self::ShowOverlay),
            "overlay/hide" => Some(Self::HideOverlay),
            "close" => Some(Self::Close(None)),
            "reopen" => Some(Self::Reopen),
            _ => {
                if let Some(id) = path.strip_prefix("select/") {
                    return serde_json::from_str(id).ok().map(Self::Select);
                }
                if let Some(id) = path.strip_prefix("close/") {
                    return serde_json::from_str(id)
                        .ok()
                        .map(|id| Self::Close(Some(id)));
                }
                percent_decode(path.strip_prefix("open/")?).map(Self::Open)
            }
        }
    }
}

/// Decodes what `encodeURIComponent` encodes.
fn percent_decode(text: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut rest = text.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        if byte == b'%' {
            let hex = std::str::from_utf8(tail.get(..2)?).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            rest = &tail[2..];
        } else {
            bytes.push(byte);
            rest = tail;
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::Command;

    #[test]
    fn parses_known_commands() {
        assert_eq!(Command::parse("ready"), Some(Command::Ready));
        assert_eq!(Command::parse("back"), Some(Command::Back));
        assert_eq!(Command::parse("forward"), Some(Command::Forward));
        assert_eq!(Command::parse("reload"), Some(Command::Reload));
        assert_eq!(Command::parse("overlay/show"), Some(Command::ShowOverlay));
        assert_eq!(Command::parse("overlay/hide"), Some(Command::HideOverlay));
        assert_eq!(Command::parse("close"), Some(Command::Close(None)));
        assert_eq!(Command::parse("reopen"), Some(Command::Reopen));
        assert!(matches!(
            Command::parse("close/3"),
            Some(Command::Close(Some(_)))
        ));
    }

    #[test]
    fn parses_an_encoded_url() {
        assert_eq!(
            Command::parse("open/https%3A%2F%2Fexample.com%2F%C3%A9t%C3%A9"),
            Some(Command::Open("https://example.com/été".to_string()))
        );
        assert_eq!(Command::parse("open/%zz"), None);
        assert_eq!(Command::parse("open/%C3"), None);
    }

    #[test]
    fn parses_a_tab_selection() {
        let Some(Command::Select(id)) = Command::parse("select/12") else {
            panic!("not a selection")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "12");
    }

    #[test]
    fn ignores_unknown_commands() {
        assert_eq!(Command::parse("quit"), None);
        assert_eq!(Command::parse("select/tab"), None);
    }
}
