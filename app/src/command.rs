//! Commands the UI sends to the app, as `https://arbalete.ui/command/<name>` requests.

use arbalete_core::import::Choice;
use arbalete_core::sidebar::{Color, NodeId, ProfileId, SpaceId};
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// The UI is listening: send it the current state.
    Ready,
    Back,
    Forward,
    Reload,
    /// Reloads the page without its cache, from the menu.
    ForceReload,
    /// Stops loading the page, from the menu.
    Stop,
    /// Zooms the page, from the menu.
    Zoom(Zoom),
    /// Prints the page, from the menu.
    Print,
    /// Shows the find bar above the page, from the menu.
    ShowFind,
    /// Finds text in the page, forward or backward: `find/forward/<percent-encoded text>`,
    /// `find/backward/<text>`. The same text again goes to the next match. From the menu, the last
    /// text, `None`.
    Find {
        text: Option<String>,
        forward: bool,
    },
    /// Hides the find bar and stops finding: `find/hide`.
    HideFind,
    /// Acts on a download from the sidebar: `download/<cancel, open, show or remove>/<id>`.
    Download(DownloadAction, u32),
    /// Opens the library on the downloads, from the menu.
    ShowDownloads,
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
    /// Creates a folder for the UI to rename, at the end of the displayed Space's pinned tabs,
    /// `folder/new`, or of a folder, from its menu.
    NewFolder(Option<NodeId>),
    /// Closes every tab of a folder, from its menu.
    CloseFolderTabs(NodeId),
    /// Removes a tab from the favorites, from its menu.
    RemoveFavorite(NodeId),
    /// Hides the box inviting to drag tabs into the empty favorites of a Space's profile:
    /// `favorites/hide-hint/<space id>`.
    HideFavoritesHint(SpaceId),
    /// Closes every "Today" tab of the displayed Space: `today/clear`.
    ClearToday,
    /// Puts the url of the displayed Space's selected tab on the clipboard: `link/copy`.
    CopyLink,
    /// Moves a tab to another Space's "Today" tabs, from its menu.
    MoveToSpace(NodeId, SpaceId),
    /// Opens the native menu of a sidebar row at the pointer: `menu/node/<tab or folder id>`, or
    /// of a Space's header, `menu/space/<space id>`.
    Menu(Target),
    /// The rows and Space header a sidebar shows, where a right-click opens their menu:
    /// `menu/targets/<docked or revealed>/<percent-encoded JSON array of targets>`, like
    /// `[{"target":{"node":12},"x":7,"y":180,"width":250,"height":41}]`.
    MenuTargets {
        revealed: bool,
        targets: Vec<HitTarget>,
    },
    /// Runs an item of a menu without showing it, by the item's id (app/src/menu.rs), as the
    /// end-to-end checks cannot pick in native menus: `menu/choose/<node or space>/<id>/<item id>`.
    ChooseMenu(Target, String),
    /// Collapses or expands a folder: `toggle/<folder id>`.
    ToggleFolder(NodeId),
    /// Renames a folder: `rename/<folder id>/<percent-encoded title>`.
    RenameFolder(NodeId, String),
    /// Moves a tab or a folder to an index among a container's or a folder's children, as the
    /// app counts them once the node has left: `move/<id>/<parent id>/<index>`.
    Move(NodeId, NodeId, usize),
    /// Puts a tab beside another in a split row, in the other's place: `split/<tab id>/<target
    /// id>`.
    Split(NodeId, NodeId),
    /// Adds a copy of a tab at an index among a container's or a folder's children:
    /// `duplicate/<tab id>/<parent id>/<index>`.
    Duplicate(NodeId, NodeId, usize),
    /// Puts a copy of a tab beside another in a split row, in the other's place:
    /// `split-duplicate/<tab id>/<target id>`.
    SplitDuplicate(NodeId, NodeId),
    /// Takes a tab out of its split row, as the row after it: `separate/<tab id>`.
    Separate(NodeId),
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
    /// none, and `/<tab id>` after it to move a tab into it.
    CreateSpace {
        title: String,
        emoji: String,
        color: Color,
        tab: Option<NodeId>,
    },
    /// Renames a Space: `space/rename/<space id>/<percent-encoded title>`.
    RenameSpace(SpaceId, String),
    /// Sets a Space's emoji, or removes it when empty: `space/emoji/<space id>/<percent-encoded
    /// emoji>`.
    SetSpaceEmoji(SpaceId, String),
    /// Changes a Space's color: `space/color/<space id>/<color>`, like `blue`.
    ColorSpace(SpaceId, Color),
    /// Hides a Space's tabs in the sidebar, or shows them again: `space/toggle/<space id>`.
    ToggleSpace(SpaceId),
    /// Deletes a Space with its tabs, once confirmed: `space/delete/<space id>`.
    DeleteSpace(SpaceId),
    /// Moves a Space to an index in sidebar order, as the app counts them once the Space has
    /// left: `space/move/<space id>/<index>`.
    MoveSpace(SpaceId, usize),
    /// Attaches a Space to another profile, from its menu.
    SetSpaceProfile(SpaceId, ProfileId),
    /// Attaches a Space to a new profile for the UI to rename, from its menu.
    NewSpaceProfile(SpaceId),
    /// Opens the import assistant: `import/show`.
    ShowImport,
    /// The import assistant is listening: send it the browsers found, `import/ready`.
    ImportReady,
    /// Closes the import assistant without importing: `import/close`.
    CloseImport,
    /// Imports browser profiles: `import/run/<percent-encoded JSON array of choices>`.
    Import(Vec<Choice>),
    /// Opens the System Settings pane where the user gives Arbalete Full Disk Access:
    /// `import/full-disk-access`.
    OpenFullDiskAccess,
    /// Paints the window around the sidebar and the card, and an empty Space's card, with colors
    /// as opaque ARGB: `window/colors/<window rrggbb>/<card rrggbb>`.
    WindowColors {
        window: u32,
        card: u32,
    },
}

/// What a menu is about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DownloadAction {
    /// Stops a download in progress.
    Cancel,
    /// Opens a finished download's file.
    Open,
    /// Shows a finished download's file in the file manager.
    Show,
    /// Removes a download from the list, leaving its file.
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    In,
    Out,
    /// Back to the page's actual size.
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    /// A tab, a folder or a favorite.
    Node(NodeId),
    /// A Space, from its header.
    Space(SpaceId),
    /// The profiles a Space may be attached to, from its card in the library.
    #[serde(rename = "space-profile")]
    SpaceProfile(SpaceId),
}

/// Where a menu's target shows in a sidebar, in CSS pixels from the view's top left corner.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct HitTarget {
    pub target: Target,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl HitTarget {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        (self.x..self.x + self.width).contains(&x) && (self.y..self.y + self.height).contains(&y)
    }
}

impl Target {
    fn parse(path: &str) -> Option<Self> {
        if let Some(id) = path.strip_prefix("space/") {
            return serde_json::from_str(id).ok().map(Self::Space);
        }
        if let Some(id) = path.strip_prefix("space-profile/") {
            return serde_json::from_str(id).ok().map(Self::SpaceProfile);
        }
        let id = path.strip_prefix("node/")?;
        serde_json::from_str(id).ok().map(Self::Node)
    }
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
            "folder/new" => Some(Self::NewFolder(None)),
            "link/copy" => Some(Self::CopyLink),
            "today/clear" => Some(Self::ClearToday),
            "next" => Some(Self::NextTab),
            "previous" => Some(Self::PreviousTab),
            "profile/new" => Some(Self::NewProfile),
            "space/next" => Some(Self::NextSpace),
            "space/previous" => Some(Self::PreviousSpace),
            "import/show" => Some(Self::ShowImport),
            "import/ready" => Some(Self::ImportReady),
            "import/close" => Some(Self::CloseImport),
            "import/full-disk-access" => Some(Self::OpenFullDiskAccess),
            "find/hide" => Some(Self::HideFind),
            _ => {
                if let Some((action, id)) = path
                    .strip_prefix("download/")
                    .and_then(|r| r.split_once('/'))
                {
                    let action = match action {
                        "cancel" => DownloadAction::Cancel,
                        "open" => DownloadAction::Open,
                        "show" => DownloadAction::Show,
                        "remove" => DownloadAction::Remove,
                        _ => return None,
                    };
                    return id.parse().ok().map(|id| Self::Download(action, id));
                }
                if let Some((direction, text)) =
                    path.strip_prefix("find/").and_then(|r| r.split_once('/'))
                {
                    let forward = match direction {
                        "forward" => true,
                        "backward" => false,
                        _ => return None,
                    };
                    let text = Some(percent_decode(text)?);
                    return Some(Self::Find { text, forward });
                }
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
                    let mut args = args.split('/');
                    let (color, emoji, title) = (args.next()?, args.next()?, args.next()?);
                    let tab = match args.next() {
                        Some(tab) => Some(serde_json::from_str(tab).ok()?),
                        None => None,
                    };
                    if args.next().is_some() {
                        return None;
                    }
                    return Some(Self::CreateSpace {
                        title: percent_decode(title)?,
                        emoji: percent_decode(emoji)?,
                        color: parse_color(color)?,
                        tab,
                    });
                }
                if let Some((view, targets)) = path
                    .strip_prefix("menu/targets/")
                    .and_then(|r| r.split_once('/'))
                {
                    let revealed = match view {
                        "docked" => false,
                        "revealed" => true,
                        _ => return None,
                    };
                    let targets = serde_json::from_str(&percent_decode(targets)?).ok()?;
                    return Some(Self::MenuTargets { revealed, targets });
                }
                if let Some(args) = path.strip_prefix("menu/choose/") {
                    // The target's kind and id, then the item's id, which may hold slashes.
                    let mut args = args.splitn(3, '/');
                    let (kind, id, item) = (args.next()?, args.next()?, args.next()?);
                    let target = Target::parse(&format!("{kind}/{id}"))?;
                    return Some(Self::ChooseMenu(target, item.to_string()));
                }
                if let Some(target) = path.strip_prefix("menu/") {
                    return Target::parse(target).map(Self::Menu);
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
                if let Some(id) = path.strip_prefix("favorites/hide-hint/") {
                    return serde_json::from_str(id).ok().map(Self::HideFavoritesHint);
                }
                if let Some(id) = path.strip_prefix("space/toggle/") {
                    return serde_json::from_str(id).ok().map(Self::ToggleSpace);
                }
                if let Some(id) = path.strip_prefix("space/delete/") {
                    return serde_json::from_str(id).ok().map(Self::DeleteSpace);
                }
                if let Some((id, index)) = path
                    .strip_prefix("space/move/")
                    .and_then(|r| r.split_once('/'))
                {
                    let id = serde_json::from_str(id).ok()?;
                    return index.parse().ok().map(|index| Self::MoveSpace(id, index));
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
                if let Some((tab, target)) =
                    path.strip_prefix("split/").and_then(|r| r.split_once('/'))
                {
                    let tab = serde_json::from_str(tab).ok()?;
                    return serde_json::from_str(target)
                        .ok()
                        .map(|target| Self::Split(tab, target));
                }
                if let Some((tab, target)) = path
                    .strip_prefix("split-duplicate/")
                    .and_then(|r| r.split_once('/'))
                {
                    let tab = serde_json::from_str(tab).ok()?;
                    return serde_json::from_str(target)
                        .ok()
                        .map(|target| Self::SplitDuplicate(tab, target));
                }
                if let Some(id) = path.strip_prefix("separate/") {
                    return serde_json::from_str(id).ok().map(Self::Separate);
                }
                if let Some(args) = path.strip_prefix("move/") {
                    let (id, parent, index) = parse_place(args)?;
                    return Some(Self::Move(id, parent, index));
                }
                if let Some(args) = path.strip_prefix("duplicate/") {
                    let (tab, parent, index) = parse_place(args)?;
                    return Some(Self::Duplicate(tab, parent, index));
                }
                percent_decode(path.strip_prefix("open/")?).map(Self::Open)
            }
        }
    }
}

/// A node, a parent and an index among its children: `<id>/<parent id>/<index>`.
fn parse_place(args: &str) -> Option<(NodeId, NodeId, usize)> {
    let mut args = args.split('/');
    let (id, parent, index) = (args.next()?, args.next()?, args.next()?);
    if args.next().is_some() {
        return None;
    }
    Some((
        serde_json::from_str(id).ok()?,
        serde_json::from_str(parent).ok()?,
        index.parse().ok()?,
    ))
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
    use super::{Command, Target};
    use arbalete_core::sidebar::Color;

    #[test]
    fn parses_known_commands() {
        assert_eq!(Command::parse("ready"), Some(Command::Ready));
        assert_eq!(Command::parse("back"), Some(Command::Back));
        assert_eq!(Command::parse("forward"), Some(Command::Forward));
        assert_eq!(Command::parse("reload"), Some(Command::Reload));
        assert_eq!(Command::parse("find/hide"), Some(Command::HideFind));
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
        assert_eq!(Command::parse("link/copy"), Some(Command::CopyLink));
        assert_eq!(Command::parse("today/clear"), Some(Command::ClearToday));
        assert!(matches!(
            Command::parse("favorites/hide-hint/4"),
            Some(Command::HideFavoritesHint(_))
        ));
        assert_eq!(Command::parse("favorites/hide-hint/x"), None);
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
    fn parses_download_commands() {
        use super::DownloadAction;
        assert_eq!(
            Command::parse("download/cancel/3"),
            Some(Command::Download(DownloadAction::Cancel, 3))
        );
        assert_eq!(
            Command::parse("download/show/12"),
            Some(Command::Download(DownloadAction::Show, 12))
        );
        assert_eq!(Command::parse("download/delete/3"), None);
        assert_eq!(Command::parse("download/open/x"), None);
    }

    #[test]
    fn parses_find_commands() {
        assert_eq!(
            Command::parse("find/forward/caf%C3%A9%2F"),
            Some(Command::Find {
                text: Some("café/".to_string()),
                forward: true,
            })
        );
        assert_eq!(
            Command::parse("find/backward/a"),
            Some(Command::Find {
                text: Some("a".to_string()),
                forward: false,
            })
        );
        assert_eq!(Command::parse("find/sideways/a"), None);
    }

    #[test]
    fn parses_folder_commands() {
        assert_eq!(Command::parse("folder/new"), Some(Command::NewFolder(None)));
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
    fn parses_a_split() {
        let Some(Command::Split(tab, target)) = Command::parse("split/4/7") else {
            panic!("not a split")
        };
        assert_eq!(serde_json::to_string(&(tab, target)).unwrap(), "[4,7]");
        assert_eq!(Command::parse("split/4"), None);
        assert_eq!(Command::parse("split/4/x"), None);
    }

    #[test]
    fn parses_a_duplicate_a_split_of_a_duplicate_and_a_separation() {
        let Some(Command::Duplicate(tab, parent, index)) = Command::parse("duplicate/12/6/3")
        else {
            panic!("not a duplicate")
        };
        assert_eq!(serde_json::to_string(&(tab, parent)).unwrap(), "[12,6]");
        assert_eq!(index, 3);
        assert_eq!(Command::parse("duplicate/12/6"), None);
        let Some(Command::SplitDuplicate(tab, target)) = Command::parse("split-duplicate/4/7")
        else {
            panic!("not a split of a duplicate")
        };
        assert_eq!(serde_json::to_string(&(tab, target)).unwrap(), "[4,7]");
        assert_eq!(Command::parse("split-duplicate/4"), None);
        let Some(Command::Separate(tab)) = Command::parse("separate/4") else {
            panic!("not a separation")
        };
        assert_eq!(serde_json::to_string(&tab).unwrap(), "4");
        assert_eq!(Command::parse("separate/x"), None);
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
                tab: None,
            })
        );
        let Some(Command::CreateSpace { tab: Some(tab), .. }) =
            Command::parse("space/create/blue//Home/12")
        else {
            panic!("not a Space with a tab")
        };
        assert_eq!(serde_json::to_string(&tab).unwrap(), "12");
        assert_eq!(Command::parse("space/create/blue//Home/x"), None);
        assert_eq!(Command::parse("space/create/blue//Home/12/3"), None);
        assert_eq!(
            Command::parse("space/create/blue//Home"),
            Some(Command::CreateSpace {
                title: "Home".into(),
                emoji: "".into(),
                color: Color::Blue,
                tab: None,
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
        let Some(Command::ToggleSpace(id)) = Command::parse("space/toggle/4") else {
            panic!("not a toggle")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "4");
        let Some(Command::DeleteSpace(id)) = Command::parse("space/delete/4") else {
            panic!("not a deletion")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "4");
        let Some(Command::MoveSpace(id, 2)) = Command::parse("space/move/4/2") else {
            panic!("a Space to move");
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "4");
        assert_eq!(Command::parse("space/move/4/x"), None);
        assert_eq!(Command::parse("space/move/x/2"), None);
        assert_eq!(Command::parse("space/move/4"), None);
        assert_eq!(Command::parse("space/toggle/x"), None);
        assert_eq!(Command::parse("space/delete/x"), None);
        assert_eq!(Command::parse("space/color/4/teal"), None);
        assert_eq!(Command::parse("space/color/x/blue"), None);
        assert_eq!(Command::parse("space/rename/4"), None);
        assert_eq!(Command::parse("space/emoji/x/a"), None);
    }

    #[test]
    fn parses_menu_commands() {
        let Some(Command::Menu(Target::Node(id))) = Command::parse("menu/node/12") else {
            panic!("not a menu")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "12");
        let Some(Command::ChooseMenu(Target::Node(id), item)) =
            Command::parse("menu/choose/node/12/move/4")
        else {
            panic!("not a menu choice")
        };
        assert_eq!(
            (serde_json::to_string(&id).unwrap(), item.as_str()),
            ("12".into(), "move/4")
        );
        let Some(Command::Menu(Target::Space(id))) = Command::parse("menu/space/4") else {
            panic!("not a Space's menu")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "4");
        let Some(Command::Menu(Target::SpaceProfile(id))) = Command::parse("menu/space-profile/4")
        else {
            panic!("not a Space's profiles")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "4");
        let Some(Command::ChooseMenu(Target::Space(id), item)) =
            Command::parse("menu/choose/space/4/profile/1")
        else {
            panic!("not a Space's menu choice")
        };
        assert_eq!(
            (serde_json::to_string(&id).unwrap(), item.as_str()),
            ("4".into(), "profile/1")
        );
        // encodeURIComponent('[{"target":{"space":4},"x":7,"y":180.5,"width":250,"height":32}]')
        let encoded = "%5B%7B%22target%22%3A%7B%22space%22%3A4%7D%2C%22x%22%3A7%2C%22y%22%3A180.5\
                       %2C%22width%22%3A250%2C%22height%22%3A32%7D%5D";
        let Some(Command::MenuTargets { revealed, targets }) =
            Command::parse(&format!("menu/targets/revealed/{encoded}"))
        else {
            panic!("not menu targets")
        };
        assert!(revealed);
        let Target::Space(id) = targets[0].target else {
            panic!("not a Space")
        };
        assert_eq!(serde_json::to_string(&id).unwrap(), "4");
        assert!(targets[0].contains(7.0, 180.5) && targets[0].contains(256.9, 212.4));
        assert!(!targets[0].contains(257.0, 200.0) && !targets[0].contains(10.0, 212.5));
        assert!(matches!(
            Command::parse("menu/targets/docked/%5B%5D"),
            Some(Command::MenuTargets {
                revealed: false,
                ..
            })
        ));
        assert_eq!(Command::parse("menu/targets/floating/%5B%5D"), None);
        assert_eq!(Command::parse("menu/targets/docked/%5B"), None);
        assert_eq!(Command::parse("menu/space/x"), None);
        assert_eq!(Command::parse("menu/node/x"), None);
        assert_eq!(Command::parse("menu/tab/12"), None);
        assert_eq!(Command::parse("menu/choose/node/12"), None);
        assert_eq!(Command::parse("menu/choose/node/x/pin"), None);
        assert_eq!(Command::parse("menu"), None);
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
