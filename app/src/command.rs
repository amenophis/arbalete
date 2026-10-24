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
    /// Pins or unpins a tab, `pin/<tab id>`, or the selected one, `pin`.
    Pin(Option<NodeId>),
    /// Creates a folder at the end of the pinned tabs, for the UI to rename: `folder/new`.
    NewFolder,
    /// Collapses or expands a folder: `toggle/<folder id>`.
    ToggleFolder(NodeId),
    /// Renames a folder: `rename/<folder id>/<percent-encoded title>`.
    RenameFolder(NodeId, String),
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
            "pin" => Some(Self::Pin(None)),
            "folder/new" => Some(Self::NewFolder),
            _ => {
                if let Some(id) = path.strip_prefix("select/") {
                    return serde_json::from_str(id).ok().map(Self::Select);
                }
                if let Some(id) = path.strip_prefix("close/") {
                    return serde_json::from_str(id)
                        .ok()
                        .map(|id| Self::Close(Some(id)));
                }
                if let Some(id) = path.strip_prefix("pin/") {
                    return serde_json::from_str(id).ok().map(|id| Self::Pin(Some(id)));
                }
                if let Some(id) = path.strip_prefix("toggle/") {
                    return serde_json::from_str(id).ok().map(Self::ToggleFolder);
                }
                if let Some((id, title)) =
                    path.strip_prefix("rename/").and_then(|r| r.split_once('/'))
                {
                    let id = serde_json::from_str(id).ok()?;
                    return percent_decode(title).map(|title| Self::RenameFolder(id, title));
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
        assert_eq!(Command::parse("pin"), Some(Command::Pin(None)));
        assert!(matches!(
            Command::parse("pin/3"),
            Some(Command::Pin(Some(_)))
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
    fn parses_folder_commands() {
        assert_eq!(Command::parse("folder/new"), Some(Command::NewFolder));
        assert!(matches!(
            Command::parse("toggle/6"),
            Some(Command::ToggleFolder(_))
        ));
        let Some(Command::RenameFolder(id, title)) = Command::parse("rename/6/Caf%C3%A9%2F2")
        else {
            panic!("not a rename")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "6");
        assert_eq!(title, "Café/2");
        assert_eq!(Command::parse("rename/6"), None);
        assert_eq!(Command::parse("rename/x/a"), None);
        assert_eq!(Command::parse("toggle/x"), None);
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
