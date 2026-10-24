//! Commands the UI sends to the app, as `https://arbalette.ui/command/<name>` requests.

use arbalette_core::import::Choice;
use arbalette_core::sidebar::{Color, NodeId, ProfileId, SpaceId};

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// The UI is listening: send it the current state.
    Ready,
    Back,
    Forward,
    Reload,
    /// Shows a tab in the content view: `select/<tab id>`.
    Select(NodeId),
    /// Shows the next tab in the sidebar, `next`, or the previous one, `previous`.
    NextTab,
    PreviousTab,
    /// Shows a favorite by its number, from 1 to 9: `favorite/<n>`.
    Favorite(usize),
    ShowOverlay,
    HideOverlay,
    /// Hides the sidebar, or shows it again: `sidebar/toggle`.
    ToggleSidebar,
    /// Shows the hidden sidebar above the page, `sidebar/reveal`, or hides it again,
    /// `sidebar/conceal`.
    RevealSidebar,
    ConcealSidebar,
    /// Moves the page's card right of where it rests beside the sidebar, by a number of points,
    /// as the sidebar pages towards the library: `card/offset/<points>`.
    CardOffset(u32),
    /// Widens the sidebar for the library, the card resting beside it: `library/show`. Any card
    /// offset gives the sidebar its own width back.
    ShowLibrary,
    /// Opens a tab: `open/<percent-encoded url>`.
    Open(String),
    /// Closes a tab, `close/<tab id>`, or the selected one, `close`.
    Close(Option<NodeId>),
    /// Reopens a closed tab, by its rank from the last closed one, 0: `reopen/<rank>`, or the last
    /// closed one, `reopen`.
    Reopen(usize),
    /// Pins or unpins a tab, `pin/<tab id>`, or the selected one, `pin`.
    Pin(Option<NodeId>),
    /// Creates a folder at the end of the pinned tabs, for the UI to rename: `folder/new`.
    NewFolder,
    /// Collapses or expands a folder: `toggle/<folder id>`.
    ToggleFolder(NodeId),
    /// Renames a folder: `rename/<folder id>/<percent-encoded title>`.
    RenameFolder(NodeId, String),
    /// Moves a tab or a folder to an index among a container's or a folder's children, as the
    /// app counts them once the node has left: `move/<id>/<parent id>/<index>`.
    Move(NodeId, NodeId, usize),
    /// Creates a profile, with a Space of its own, for the UI to rename: `profile/new`.
    NewProfile,
    /// Renames a profile: `profile/rename/<profile id>/<percent-encoded name>`.
    RenameProfile(ProfileId, String),
    /// Changes a profile's color: `profile/color/<profile id>/<color>`, like `blue`.
    ColorProfile(ProfileId, Color),
    /// Shows a Space in the sidebar, and its selected tab in the content view: `space/<space id>`.
    ShowSpace(SpaceId),
    /// Shows a Space by its number in sidebar order, from 1 to 9: `space/number/<n>`.
    SpaceNumber(usize),
    /// Shows the Space after the displayed one, `space/next`, or before it, `space/previous`.
    NextSpace,
    PreviousSpace,
    /// Creates a Space in the displayed Space's profile and displays it:
    /// `space/create/<color>/<percent-encoded emoji>/<percent-encoded title>`, an empty emoji for
    /// none.
    CreateSpace {
        title: String,
        emoji: String,
        color: Color,
    },
    /// Renames a Space: `space/rename/<space id>/<percent-encoded title>`.
    RenameSpace(SpaceId, String),
    /// Sets a Space's emoji, or removes it when empty: `space/emoji/<space id>/<percent-encoded
    /// emoji>`.
    SetSpaceEmoji(SpaceId, String),
    /// Changes a Space's color: `space/color/<space id>/<color>`, like `blue`.
    ColorSpace(SpaceId, Color),
    /// Opens the import assistant: `import/show`.
    ShowImport,
    /// The import assistant is listening: send it the browsers found, `import/ready`.
    ImportReady,
    /// Closes the import assistant without importing: `import/close`.
    CloseImport,
    /// Imports browser profiles: `import/run/<percent-encoded JSON array of choices>`.
    Import(Vec<Choice>),
    /// Opens the System Settings pane where the user gives Arbalette Full Disk Access:
    /// `import/full-disk-access`.
    OpenFullDiskAccess,
    /// Paints the window around the sidebar and the card, and an empty Space's card, with colors
    /// as opaque ARGB: `window/colors/<window rrggbb>/<card rrggbb>`.
    WindowColors {
        window: u32,
        card: u32,
    },
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
            "sidebar/toggle" => Some(Self::ToggleSidebar),
            "sidebar/reveal" => Some(Self::RevealSidebar),
            "sidebar/conceal" => Some(Self::ConcealSidebar),
            "library/show" => Some(Self::ShowLibrary),
            "close" => Some(Self::Close(None)),
            "reopen" => Some(Self::Reopen(0)),
            "pin" => Some(Self::Pin(None)),
            "folder/new" => Some(Self::NewFolder),
            "next" => Some(Self::NextTab),
            "previous" => Some(Self::PreviousTab),
            "profile/new" => Some(Self::NewProfile),
            "space/next" => Some(Self::NextSpace),
            "space/previous" => Some(Self::PreviousSpace),
            "import/show" => Some(Self::ShowImport),
            "import/ready" => Some(Self::ImportReady),
            "import/close" => Some(Self::CloseImport),
            "import/full-disk-access" => Some(Self::OpenFullDiskAccess),
            _ => {
                if let Some((window, card)) = path
                    .strip_prefix("window/colors/")
                    .and_then(|r| r.split_once('/'))
                {
                    return Some(Self::WindowColors {
                        window: parse_rgb(window)?,
                        card: parse_rgb(card)?,
                    });
                }
                if let Some(offset) = path.strip_prefix("card/offset/") {
                    return offset.parse().ok().map(Self::CardOffset);
                }
                if let Some(choices) = path.strip_prefix("import/run/") {
                    let choices = serde_json::from_str(&percent_decode(choices)?).ok();
                    return choices.map(Self::Import);
                }
                if let Some((id, name)) = path
                    .strip_prefix("profile/rename/")
                    .and_then(|r| r.split_once('/'))
                {
                    let id = serde_json::from_str(id).ok()?;
                    return percent_decode(name).map(|name| Self::RenameProfile(id, name));
                }
                if let Some((id, color)) = path
                    .strip_prefix("profile/color/")
                    .and_then(|r| r.split_once('/'))
                {
                    let id = serde_json::from_str(id).ok()?;
                    return parse_color(color).map(|color| Self::ColorProfile(id, color));
                }
                if let Some(args) = path.strip_prefix("space/create/") {
                    let mut args = args.splitn(3, '/');
                    let (color, emoji, title) = (args.next()?, args.next()?, args.next()?);
                    return Some(Self::CreateSpace {
                        title: percent_decode(title)?,
                        emoji: percent_decode(emoji)?,
                        color: parse_color(color)?,
                    });
                }
                if let Some((id, title)) = path
                    .strip_prefix("space/rename/")
                    .and_then(|r| r.split_once('/'))
                {
                    let id = serde_json::from_str(id).ok()?;
                    return percent_decode(title).map(|title| Self::RenameSpace(id, title));
                }
                if let Some((id, emoji)) = path
                    .strip_prefix("space/emoji/")
                    .and_then(|r| r.split_once('/'))
                {
                    let id = serde_json::from_str(id).ok()?;
                    return percent_decode(emoji).map(|emoji| Self::SetSpaceEmoji(id, emoji));
                }
                if let Some((id, color)) = path
                    .strip_prefix("space/color/")
                    .and_then(|r| r.split_once('/'))
                {
                    let id = serde_json::from_str(id).ok()?;
                    return parse_color(color).map(|color| Self::ColorSpace(id, color));
                }
                if let Some(number) = path.strip_prefix("space/number/") {
                    let number = number.parse().ok().filter(|n| (1..=9).contains(n));
                    return number.map(Self::SpaceNumber);
                }
                if let Some(number) = path.strip_prefix("favorite/") {
                    let number = number.parse().ok().filter(|n| (1..=9).contains(n));
                    return number.map(Self::Favorite);
                }
                if let Some(id) = path.strip_prefix("space/") {
                    return serde_json::from_str(id).ok().map(Self::ShowSpace);
                }
                if let Some(rank) = path.strip_prefix("reopen/") {
                    return rank.parse().ok().map(Self::Reopen);
                }
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
                if let Some(args) = path.strip_prefix("move/") {
                    let mut args = args.split('/');
                    let (id, parent, index) = (args.next()?, args.next()?, args.next()?);
                    if args.next().is_some() {
                        return None;
                    }
                    let (id, parent) = (
                        serde_json::from_str(id).ok()?,
                        serde_json::from_str(parent).ok()?,
                    );
                    return index
                        .parse()
                        .ok()
                        .map(|index| Self::Move(id, parent, index));
                }
                percent_decode(path.strip_prefix("open/")?).map(Self::Open)
            }
        }
    }
}

/// A color by its name, like `blue`.
fn parse_color(name: &str) -> Option<Color> {
    serde_json::from_value(serde_json::Value::String(name.to_string())).ok()
}

/// An opaque ARGB color from its hexadecimal RGB, like `1b0e1f`.
fn parse_rgb(hex: &str) -> Option<u32> {
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16)
        .ok()
        .map(|rgb| 0xff00_0000 | rgb)
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
    use arbalette_core::sidebar::Color;

    #[test]
    fn parses_known_commands() {
        assert_eq!(Command::parse("ready"), Some(Command::Ready));
        assert_eq!(Command::parse("back"), Some(Command::Back));
        assert_eq!(Command::parse("forward"), Some(Command::Forward));
        assert_eq!(Command::parse("reload"), Some(Command::Reload));
        assert_eq!(Command::parse("overlay/show"), Some(Command::ShowOverlay));
        assert_eq!(Command::parse("overlay/hide"), Some(Command::HideOverlay));
        assert_eq!(Command::parse("close"), Some(Command::Close(None)));
        assert_eq!(Command::parse("reopen"), Some(Command::Reopen(0)));
        assert_eq!(Command::parse("reopen/2"), Some(Command::Reopen(2)));
        assert_eq!(Command::parse("reopen/x"), None);
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
    fn parses_window_colors() {
        assert_eq!(
            Command::parse("window/colors/1b0e1f/F8F6FB"),
            Some(Command::WindowColors {
                window: 0xff1b_0e1f,
                card: 0xfff8_f6fb,
            })
        );
        assert_eq!(Command::parse("window/colors/1b0e1f"), None);
        assert_eq!(Command::parse("window/colors/1b0e1/f8f6fb"), None);
        assert_eq!(Command::parse("window/colors/+b0e1f/f8f6fb"), None);
        assert_eq!(Command::parse("window/colors/1b0e1fff/f8f6fb"), None);
    }

    #[test]
    fn parses_library_commands() {
        assert_eq!(Command::parse("library/show"), Some(Command::ShowLibrary));
        assert_eq!(
            Command::parse("card/offset/180"),
            Some(Command::CardOffset(180))
        );
        assert_eq!(
            Command::parse("card/offset/0"),
            Some(Command::CardOffset(0))
        );
        assert_eq!(Command::parse("card/offset/-3"), None);
        assert_eq!(Command::parse("card/offset/1.5"), None);
    }

    #[test]
    fn parses_tab_shortcuts() {
        assert_eq!(Command::parse("next"), Some(Command::NextTab));
        assert_eq!(Command::parse("previous"), Some(Command::PreviousTab));
        assert_eq!(Command::parse("favorite/1"), Some(Command::Favorite(1)));
        assert_eq!(Command::parse("favorite/9"), Some(Command::Favorite(9)));
        assert_eq!(Command::parse("favorite/0"), None);
        assert_eq!(Command::parse("favorite/10"), None);
        assert_eq!(Command::parse("favorite/x"), None);
    }

    #[test]
    fn parses_sidebar_commands() {
        assert_eq!(
            Command::parse("sidebar/toggle"),
            Some(Command::ToggleSidebar)
        );
        assert_eq!(
            Command::parse("sidebar/reveal"),
            Some(Command::RevealSidebar)
        );
        assert_eq!(
            Command::parse("sidebar/conceal"),
            Some(Command::ConcealSidebar)
        );
        assert_eq!(Command::parse("sidebar"), None);
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
    fn parses_profile_commands() {
        assert_eq!(Command::parse("profile/new"), Some(Command::NewProfile));
        let Some(Command::RenameProfile(id, name)) =
            Command::parse("profile/rename/1/Caf%C3%A9%2F2")
        else {
            panic!("not a rename")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "1");
        assert_eq!(name, "Café/2");
        let Some(Command::ColorProfile(id, color)) = Command::parse("profile/color/1/blue") else {
            panic!("not a color")
        };
        assert_eq!(
            serde_json::to_string(&(id, color)).unwrap(),
            r#"[1,"blue"]"#
        );
        assert_eq!(Command::parse("profile/color/1/teal"), None);
        assert_eq!(Command::parse("profile/color/x/blue"), None);
        assert_eq!(Command::parse("profile/rename/1"), None);
        assert_eq!(Command::parse("profile/rename/x/a"), None);
    }

    #[test]
    fn parses_a_move() {
        let Some(Command::Move(id, parent, index)) = Command::parse("move/12/6/3") else {
            panic!("not a move")
        };
        assert_eq!(serde_json::to_string(&(id, parent)).unwrap(), "[12,6]");
        assert_eq!(index, 3);
        assert_eq!(Command::parse("move/12/6"), None);
        assert_eq!(Command::parse("move/12/6/3/1"), None);
        assert_eq!(Command::parse("move/12/6/-1"), None);
        assert_eq!(Command::parse("move/x/6/3"), None);
    }

    #[test]
    fn parses_a_tab_selection() {
        let Some(Command::Select(id)) = Command::parse("select/12") else {
            panic!("not a selection")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "12");
    }

    #[test]
    fn parses_a_space_to_show() {
        let Some(Command::ShowSpace(id)) = Command::parse("space/4") else {
            panic!("not a Space")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "4");
        assert_eq!(Command::parse("space/x"), None);
        assert_eq!(Command::parse("space"), None);
    }

    #[test]
    fn parses_space_shortcuts() {
        assert_eq!(Command::parse("space/next"), Some(Command::NextSpace));
        assert_eq!(
            Command::parse("space/previous"),
            Some(Command::PreviousSpace)
        );
        assert_eq!(
            Command::parse("space/number/1"),
            Some(Command::SpaceNumber(1))
        );
        assert_eq!(
            Command::parse("space/number/9"),
            Some(Command::SpaceNumber(9))
        );
        assert_eq!(Command::parse("space/number/0"), None);
        assert_eq!(Command::parse("space/number/10"), None);
        assert_eq!(Command::parse("space/number/x"), None);
    }

    #[test]
    fn parses_space_commands() {
        assert_eq!(
            Command::parse("space/create/green/%F0%9F%8F%A1/Caf%C3%A9%2F2"),
            Some(Command::CreateSpace {
                title: "Café/2".into(),
                emoji: "🏡".into(),
                color: Color::Green,
            })
        );
        assert_eq!(
            Command::parse("space/create/blue//Home"),
            Some(Command::CreateSpace {
                title: "Home".into(),
                emoji: "".into(),
                color: Color::Blue,
            })
        );
        assert_eq!(Command::parse("space/create/teal//Home"), None);
        assert_eq!(Command::parse("space/create/blue/Home"), None);
        let Some(Command::RenameSpace(id, title)) = Command::parse("space/rename/4/Caf%C3%A9%2F2")
        else {
            panic!("not a rename")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "4");
        assert_eq!(title, "Café/2");
        let Some(Command::SetSpaceEmoji(id, emoji)) = Command::parse("space/emoji/4/%F0%9F%8F%A1")
        else {
            panic!("not an emoji")
        };
        assert_eq!(
            (serde_json::to_string(&id).unwrap(), emoji.as_str()),
            ("4".into(), "🏡")
        );
        let Some(Command::SetSpaceEmoji(_, emoji)) = Command::parse("space/emoji/4/") else {
            panic!("not an emoji")
        };
        assert_eq!(emoji, "");
        let Some(Command::ColorSpace(id, color)) = Command::parse("space/color/4/blue") else {
            panic!("not a color")
        };
        assert_eq!(
            serde_json::to_string(&(id, color)).unwrap(),
            r#"[4,"blue"]"#
        );
        assert_eq!(Command::parse("space/color/4/teal"), None);
        assert_eq!(Command::parse("space/color/x/blue"), None);
        assert_eq!(Command::parse("space/rename/4"), None);
        assert_eq!(Command::parse("space/emoji/x/a"), None);
    }

    #[test]
    fn parses_import_commands() {
        assert_eq!(Command::parse("import/show"), Some(Command::ShowImport));
        assert_eq!(Command::parse("import/ready"), Some(Command::ImportReady));
        assert_eq!(Command::parse("import/close"), Some(Command::CloseImport));
        assert_eq!(
            Command::parse("import/full-disk-access"),
            Some(Command::OpenFullDiskAccess)
        );
        // encodeURIComponent('[{"browser":1,"profile":0,"into":4,"bar":"pinnedFolder"}]')
        let encoded = "%5B%7B%22browser%22%3A1%2C%22profile%22%3A0%2C%22into%22%3A4%2C\
                       %22bar%22%3A%22pinnedFolder%22%7D%5D";
        let Some(Command::Import(choices)) = Command::parse(&format!("import/run/{encoded}"))
        else {
            panic!("not an import")
        };
        let expected = r#"[{"browser":1,"profile":0,"into":4,"bar":"pinnedFolder"}]"#;
        assert_eq!(choices, serde_json::from_str::<Vec<_>>(expected).unwrap());
        assert_eq!(choices[0].browser, 1);
        assert_eq!(
            Command::parse("import/run/%5B%5D"),
            Some(Command::Import(vec![]))
        );
        assert_eq!(Command::parse("import/run/%5B"), None);
        assert_eq!(Command::parse("import/run/%zz"), None);
        assert_eq!(Command::parse("import"), None);
    }

    #[test]
    fn ignores_unknown_commands() {
        assert_eq!(Command::parse("quit"), None);
        assert_eq!(Command::parse("select/tab"), None);
    }
}
