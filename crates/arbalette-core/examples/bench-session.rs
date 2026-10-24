//! Writes the performance prototype's session: 6 favorites, then 5 Spaces of 100 tabs, half of them
//! in folders. The last 2 Spaces belong to a second profile, to check that profiles keep their web
//! data apart.
//!
//! Usage: `cargo run -p arbalette-core --example bench-session -- <data dir>`, then run the app with
//! `ARBALETTE_DATA_DIR=<data dir>`.

use arbalette_core::{sidebar::Sidebar, store};
use std::path::PathBuf;

const FAVORITES: usize = 6;
const SPACES: usize = 5;
const WORK_SPACES: usize = 2;
const FOLDERS: usize = 5;
const TABS_PER_FOLDER: usize = 10;
const TODAY_TABS: usize = 50;

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: bench-session <data dir>"),
    );
    let mut sidebar = Sidebar::default();
    let default = sidebar.profiles()[0].id;
    let work = sidebar.add_profile("Work");
    for index in 1..SPACES {
        let profile = if index < SPACES - WORK_SPACES {
            default
        } else {
            work
        };
        sidebar
            .add_space(&format!("Space {}", index + 1), profile)
            .unwrap();
    }
    let mut count = 0;
    for space in sidebar.spaces().to_vec() {
        let mut open = |sidebar: &mut Sidebar| {
            count += 1;
            let url = format!("https://example.com/?tab={count}");
            let tab = sidebar.open_tab(space.id, &url).unwrap();
            sidebar.tab_mut(tab).unwrap().title = format!("Tab {count}");
            tab
        };
        for folder in 1..=FOLDERS {
            let folder = sidebar
                .add_folder(space.pinned, &format!("Folder {folder}"))
                .unwrap();
            for _ in 0..TABS_PER_FOLDER {
                let tab = open(&mut sidebar);
                sidebar.move_node(tab, folder, usize::MAX).unwrap();
            }
        }
        for _ in 0..TODAY_TABS {
            open(&mut sidebar);
        }
    }
    let home = sidebar.spaces()[0].id;
    let favorites = sidebar.profiles()[0].favorites;
    for index in 1..=FAVORITES {
        let url = format!("https://example.com/?favorite={index}");
        let tab = sidebar.open_tab(home, &url).unwrap();
        sidebar.tab_mut(tab).unwrap().title = format!("Favorite {index}");
        sidebar.move_node(tab, favorites, usize::MAX).unwrap();
        count += 1;
    }
    let home = sidebar.spaces()[0].clone();
    let first = sidebar.node(home.today).unwrap().children[0];
    sidebar.select(home.id, first).unwrap();

    std::fs::create_dir_all(&dir).unwrap();
    store::save(&sidebar, &dir.join(store::FILE_NAME)).unwrap();
    println!("{count} tabs written to {}", dir.display());
}
