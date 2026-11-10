//! Writes a data folder of demo tabs, Spaces and history for the README's screenshots
//! (docs/screenshots/README.md): neutral public sites, no account.
//!
//! cargo run -p arbalete-core --example screenshot-demo -- <data dir> <home dir> <light|dark> [icons dir]
//!
//! The home folder gets other browsers' profiles, without any data, for the import screen of a
//! first launch (`ARBALETE_HOME`). The icons folder holds one `<host>.png` per site, which the
//! tabs show before they load.

use arbalete_core::{
    favicon,
    history::History,
    sidebar::{Mode, NodeId, Sidebar, SpaceId, Theme},
    store::{self, Place, Session},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Every window shot has this size, in points.
const WIDTH: f64 = 1280.0;
const HEIGHT: f64 = 800.0;

fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: screenshot-demo <data dir> <home dir> <light|dark> [icons dir]";
    let dir = PathBuf::from(args.next().expect(usage));
    let home = PathBuf::from(args.next().expect(usage));
    let mode = match args.next().as_deref() {
        Some("light") => Mode::Light,
        Some("dark") => Mode::Dark,
        _ => panic!("{usage}"),
    };
    let icons = args.next().map(PathBuf::from);
    fs::create_dir_all(&dir).expect("a data folder");
    write_home(&home);

    let mut sidebar = Sidebar::empty();
    let profile = sidebar.add_profile("Personal");
    // The Command Bar then shows the open tabs and the history, and sends nothing typed away.
    sidebar.set_search_suggestions(profile, false).unwrap();
    // Bitwarden and uBlock Origin Lite, by the address once installed from the Chrome Web Store.
    for id in [
        "nngceckbapebfimnlniiiahkandclblb",
        "ddkjiahejlhfcafbddmgiahcphecmpfh",
    ] {
        sidebar.pin_extension(profile, id, true).unwrap();
    }
    let profile_dir = dir.join(format!(
        "Profile {}",
        serde_json::to_string(&profile).unwrap()
    ));

    let favorites = sidebar.profiles()[0].favorites;
    for (url, title) in [
        ("https://en.wikipedia.org/wiki/Main_Page", "Wikipedia"),
        ("https://developer.mozilla.org/en-US/", "MDN Web Docs"),
        ("https://docs.rs/", "Docs.rs"),
        ("https://crates.io/", "crates.io"),
        ("https://arbalete.co/", "Arbalete"),
        ("https://www.openstreetmap.org/", "OpenStreetMap"),
    ] {
        sidebar.add_tab(favorites, url, title).unwrap();
    }

    let dev = space(
        &mut sidebar,
        profile,
        "Dev",
        "🛠️",
        &["#5b7cfa", "#a352ff"],
        mode,
    );
    let (pinned, today) = roots(&sidebar, dev);
    let rust = sidebar.add_folder(pinned, "Rust").unwrap();
    for (url, title) in [
        (
            "https://doc.rust-lang.org/book/",
            "The Rust Programming Language",
        ),
        ("https://doc.rust-lang.org/std/", "std - Rust"),
        ("https://doc.rust-lang.org/nomicon/", "The Rustonomicon"),
    ] {
        sidebar.add_tab(rust, url, title).unwrap();
    }
    sidebar
        .add_tab(
            pinned,
            "https://github.com/amenophis/arbalete",
            "amenophis/arbalete",
        )
        .unwrap();
    sidebar
        .add_tab(
            pinned,
            "https://github.com/chromiumembedded/cef",
            "chromiumembedded/cef",
        )
        .unwrap();
    let ownership = sidebar
        .add_tab(
            today,
            "https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html",
            "What Is Ownership? - The Rust Programming Language",
        )
        .unwrap();
    let docs = sidebar
        .add_tab(today, "https://docs.rs/serde/latest/serde/", "serde - Rust")
        .unwrap();
    let code = sidebar
        .add_tab(
            today,
            "https://github.com/serde-rs/serde/blob/master/serde/src/lib.rs",
            "serde/serde/src/lib.rs",
        )
        .unwrap();
    sidebar
        .add_tab(
            today,
            "https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_grid_layout",
            "CSS grid layout - MDN",
        )
        .unwrap();
    sidebar
        .add_tab(
            today,
            "https://www.swift.org/documentation/",
            "Swift Documentation",
        )
        .unwrap();
    sidebar.split(code, docs).unwrap();
    sidebar.select(dev, ownership).unwrap();

    let reading = space(
        &mut sidebar,
        profile,
        "Reading",
        "📚",
        &["#2fb67c", "#c8e66b"],
        mode,
    );
    let (pinned, today) = roots(&sidebar, reading);
    sidebar
        .add_tab(
            pinned,
            "https://en.wikipedia.org/wiki/Wikipedia:Featured_articles",
            "Featured articles",
        )
        .unwrap();
    let crossbow = sidebar
        .add_tab(
            today,
            "https://en.wikipedia.org/wiki/Crossbow",
            "Crossbow - Wikipedia",
        )
        .unwrap();
    sidebar
        .add_tab(
            today,
            "https://en.wikipedia.org/wiki/Typography",
            "Typography - Wikipedia",
        )
        .unwrap();
    sidebar
        .add_tab(today, "https://www.gutenberg.org/", "Project Gutenberg")
        .unwrap();
    sidebar.select(reading, crossbow).unwrap();

    let travel = space(
        &mut sidebar,
        profile,
        "Travel",
        "🧭",
        &["#ff8a3d", "#ff5d8f"],
        mode,
    );
    let (pinned, today) = roots(&sidebar, travel);
    sidebar
        .add_tab(
            pinned,
            "https://en.wikivoyage.org/wiki/Main_Page",
            "Wikivoyage",
        )
        .unwrap();
    let lisbon = sidebar
        .add_tab(
            today,
            "https://en.wikivoyage.org/wiki/Lisbon",
            "Lisbon - Wikivoyage",
        )
        .unwrap();
    sidebar
        .add_tab(
            today,
            "https://www.openstreetmap.org/#map=13/38.7223/-9.1393",
            "OpenStreetMap",
        )
        .unwrap();
    sidebar
        .add_tab(
            today,
            "https://en.wikivoyage.org/wiki/Porto",
            "Porto - Wikivoyage",
        )
        .unwrap();
    sidebar.select(travel, lisbon).unwrap();
    sidebar.display(dev).unwrap();

    if let Some(icons) = icons {
        set_icons(&mut sidebar, &profile_dir, &icons);
    }

    let session = Session {
        sidebar,
        // An unknown screen places the window on the main one.
        place: Some(Place {
            screen: "demo".into(),
            x: 120.0,
            y: 80.0,
            width: WIDTH,
            height: HEIGHT,
            full_screen: false,
        }),
        windows: Vec::new(),
    };
    store::save(&session, &dir.join(store::FILE_NAME)).unwrap();
    fs::write(
        dir.join("settings.json"),
        r#"{ "onboarded": true, "sidebarHintShown": true }"#,
    )
    .unwrap();
    write_history(&profile_dir);
}

fn space(
    sidebar: &mut Sidebar,
    profile: arbalete_core::sidebar::ProfileId,
    title: &str,
    emoji: &str,
    colors: &[&str],
    mode: Mode,
) -> SpaceId {
    let space = sidebar.add_space(title, profile).unwrap();
    sidebar.set_space_emoji(space, emoji).unwrap();
    let theme = Theme {
        colors: colors.iter().map(|color| color.to_string()).collect(),
        mode,
        intensity: 0.75,
        noise: 0.35,
    };
    sidebar.set_space_theme(space, theme).unwrap();
    space
}

/// A Space's pinned tabs and "Today" tabs.
fn roots(sidebar: &Sidebar, space: SpaceId) -> (NodeId, NodeId) {
    let space = sidebar.spaces().iter().find(|s| s.id == space).unwrap();
    (space.pinned, space.today)
}

/// Gives each tab its site's icon, from `<host>.png` in `icons`, cached as the app caches them.
fn set_icons(sidebar: &mut Sidebar, profile_dir: &Path, icons: &Path) {
    let tabs: Vec<_> = sidebar
        .tabs()
        .map(|(id, tab)| (id, tab.url.clone()))
        .collect();
    for (id, url) in tabs {
        let host = url.split('/').nth(2).unwrap_or_default();
        let Ok(png) = fs::read(icons.join(format!("{host}.png"))) else {
            continue;
        };
        let source = format!("https://{host}/favicon.ico");
        favicon::write(profile_dir, &source, &png).unwrap();
        sidebar.tab_mut(id).unwrap().favicon = Some(source);
    }
}

/// Pages visited earlier, which the Command Bar suggests.
fn write_history(profile_dir: &Path) {
    fs::create_dir_all(profile_dir).unwrap();
    let history = History::open(&profile_dir.join("History.sqlite")).unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let pages = [
        (
            "https://doc.rust-lang.org/book/ch08-02-strings.html",
            "Storing UTF-8 Encoded Text with Strings - The Rust Programming Language",
        ),
        (
            "https://doc.rust-lang.org/std/string/struct.String.html",
            "String in std::string - Rust",
        ),
        (
            "https://docs.rs/serde_json/latest/serde_json/",
            "serde_json - Rust",
        ),
        (
            "https://doc.rust-lang.org/rust-by-example/std/str.html",
            "Strings - Rust By Example",
        ),
        (
            "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String",
            "String - JavaScript | MDN",
        ),
        (
            "https://en.wikipedia.org/wiki/String_(computer_science)",
            "String (computer science) - Wikipedia",
        ),
    ];
    for (rank, (url, title)) in pages.iter().enumerate() {
        let hours_ago = rank as i64 * 3_600_000;
        for _ in rank..pages.len() {
            history.visit(url, now - hours_ago).unwrap();
        }
        history.set_title(url, title).unwrap();
    }
}

/// Browsers the import screen finds: Arc, Chrome and Firefox, each with empty profiles.
fn write_home(home: &Path) {
    let support = home.join("Library/Application Support");
    for (dir, profiles) in [
        (
            "Arc/User Data",
            &[("Default", "Personal"), ("Profile 1", "Work")][..],
        ),
        ("Google/Chrome", &[("Default", "Personal")][..]),
    ] {
        let dir = support.join(dir);
        let mut cache = serde_json::Map::new();
        for (profile, name) in profiles {
            fs::create_dir_all(dir.join(profile)).unwrap();
            cache.insert(profile.to_string(), serde_json::json!({ "name": name }));
        }
        let local_state = serde_json::json!({ "profile": { "info_cache": cache } });
        fs::write(dir.join("Local State"), local_state.to_string()).unwrap();
    }
    let firefox = support.join("Firefox");
    fs::create_dir_all(firefox.join("Profiles/demo.default-release")).unwrap();
    fs::write(
        firefox.join("profiles.ini"),
        "[Profile0]\nName=default-release\nIsRelative=1\nPath=Profiles/demo.default-release\n",
    )
    .unwrap();
}
