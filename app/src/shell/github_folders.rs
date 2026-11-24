//! GitHub folders: fills them with github.com's search results, read through their profile's
//! own github.com session (decision 0013).

use super::*;
use arbalete_core::github;

/// How often every GitHub folder is refreshed.
const REFRESH_MS: i64 = 5 * 60_000;

/// How soon a GitHub folder is asked again when something may have changed what it lists.
const SOON: std::time::Duration = std::time::Duration::from_secs(10);

/// How much of a search's answer is read, the rest ignored: a page of results weighs about 50 KB.
const SEARCH_SIZE: usize = 4 * 1024 * 1024;

thread_local! {
    /// The folders whose search is under way, one at a time each.
    static SEARCHING: RefCell<HashSet<NodeId>> = RefCell::default();
    /// When each folder last asked github.com.
    static ASKED: RefCell<HashMap<NodeId, std::time::Instant>> = RefCell::default();
}

/// Adds a GitHub folder at the end of `parent`, and fills it; opening a pull request adds one
/// again, should the user have turned that off by deleting one. Like a selection, an unknown
/// parent is ignored.
pub(super) fn new_github_folder(parent: NodeId) {
    let folder = update_sidebar(|sidebar| {
        sidebar.set_github_on_pull_request(true);
        sidebar.add_github_folder(parent)
    });
    if let Ok(folder) = folder {
        refresh_github_folder(folder);
    }
}

/// Like Arc's, a tab opening a pull request adds a GitHub folder of the user's pull requests and
/// reviews to its Space, if it has none and the user did not turn that off; a toast tells.
pub(super) fn opened_github_page(profile: ProfileId, tab: NodeId, url: &str) {
    // An incognito window keeps nothing.
    if !github::is_pull_request(url)
        || INCOGNITO.with_borrow(|incognito| incognito.contains_key(&profile))
    {
        return;
    }
    let Some(pinned) = STATE.with_borrow(|state| state.sidebar.github_folder_wanted(tab)) else {
        return;
    };
    let folder = update_sidebar(|sidebar| sidebar.add_github_folder(pinned));
    if let Ok(folder) = folder {
        refresh_github_folder(folder);
        toast(json!({
            "text": "Your pull requests and reviews will show up in “Pull Requests”",
            "icon": "arrow.triangle.pull",
        }));
    }
}

/// Shows one of `github::LISTS` in a GitHub folder too, or no more, and fills it again.
pub(super) fn toggle_github_list(folder: NodeId, list: usize) {
    let Some(list) = github::LISTS.get(list) else {
        return;
    };
    if update_sidebar(|sidebar| sidebar.toggle_github_list(folder, list.key)).is_ok() {
        refresh_github_folder(folder);
    }
}

/// Hides a repository from a GitHub folder, or shows it again, asking github.com for its results.
pub(super) fn toggle_github_repository(folder: NodeId, repository: &str) {
    if update_sidebar(|sidebar| sidebar.toggle_github_repository(folder, repository)).is_ok() {
        refresh_github_folder(folder);
    }
}

/// Refreshes the GitHub folders whose profile's web data is ready; the others are refreshed
/// once it is (ProfileContextHandler).
pub(super) fn refresh_github_folders() {
    let folders = STATE.with_borrow(|state| state.sidebar.github_folders());
    for (folder, _) in folders {
        refresh_github_folder(folder);
    }
}

/// Refreshes the GitHub folders `which` picks unless they asked within ten seconds, like Arc's:
/// when the app comes in front, a Space shows, a result is selected, or a page of github.com
/// loads, which may be the user signing in, opening or reviewing a pull request.
pub(super) fn refresh_github_folders_soon(which: impl Fn(&Sidebar, NodeId) -> bool) {
    let folders: Vec<_> = STATE.with_borrow(|state| {
        let sidebar = &state.sidebar;
        let folders = sidebar.github_folders().into_iter().map(|(id, _)| id);
        folders.filter(|&id| which(sidebar, id)).collect()
    });
    for folder in folders {
        let asked = ASKED.with_borrow(|asked| asked.get(&folder).copied());
        if asked.is_none_or(|at| at.elapsed() >= SOON) {
            refresh_github_folder(folder);
        }
    }
}

/// Refreshes the GitHub folders every five minutes.
pub(super) fn schedule_github_folders() {
    post_delayed_task(
        ThreadId::UI,
        Some(&mut RefreshGitHubFolders::new()),
        REFRESH_MS,
    );
}

wrap_task! {
    struct RefreshGitHubFolders {}

    impl Task {
        fn execute(&self) {
            refresh_github_folders();
            schedule_github_folders();
        }
    }
}

/// Asks github.com for a GitHub folder's searches with its profile's cookies, as its own page
/// would, unless it is already asking.
pub(super) fn refresh_github_folder(folder: NodeId) {
    let found = STATE.with_borrow(|state| {
        let sidebar = &state.sidebar;
        let (_, lists) = sidebar
            .github_folders()
            .into_iter()
            .find(|(id, _)| *id == folder)?;
        Some((sidebar.profile_of(folder)?, lists))
    });
    let Some((profile, mut lists)) = found else {
        return;
    };
    // They tell the reviews of the pull requests it shows.
    lists.extend(github::REVIEWS.iter());
    let context = match CONTEXTS.with_borrow(|contexts| contexts.get(&profile).cloned()) {
        Some(Some(context)) => context,
        Some(None) => return,
        None => return create_profile_context(profile),
    };
    if !SEARCHING.with_borrow_mut(|searching| searching.insert(folder)) {
        return;
    }
    ASKED.with_borrow_mut(|asked| asked.insert(folder, std::time::Instant::now()));
    let answers = Rc::new(RefCell::new(Answers {
        left: lists.len(),
        lists: lists.clone(),
        answers: vec![None; lists.len()],
        failed: false,
    }));
    for (index, list) in lists.iter().enumerate() {
        let mut client = GitHubSearch::new(folder, index, Rc::default(), answers.clone());
        if !search(
            &github::search_url(list.query, now() as u64),
            &mut client,
            context.clone(),
        ) {
            answered(folder, &answers, index, None);
        }
    }
}

/// Starts a search's request, with the profile's cookies; false if it could not start.
fn search(url: &str, client: &mut UrlrequestClient, mut context: RequestContext) -> bool {
    let Some(mut request) = request_create() else {
        return false;
    };
    request.set_url(Some(&CefString::from(url)));
    request.set_method(Some(&CefString::from("GET")));
    request.set_header_by_name(
        Some(&CefString::from("Accept")),
        Some(&CefString::from("application/json")),
        1,
    );
    // So that the session's SameSite cookies go with it, as with a page of github.com.
    request.set_first_party_for_cookies(Some(&CefString::from("https://github.com/")));
    let flags = sys::cef_urlrequest_flags_t::UR_FLAG_ALLOW_STORED_CREDENTIALS.0
        | sys::cef_urlrequest_flags_t::UR_FLAG_SKIP_CACHE.0;
    request.set_flags(flags as i32);
    urlrequest_create(Some(&mut request), Some(client), Some(&mut context)).is_some()
}

/// A folder's searches under way, and what they answered.
struct Answers {
    left: usize,
    lists: Vec<&'static github::List>,
    answers: Vec<Option<github::Answer>>,
    failed: bool,
}

/// Notes what a folder's search answered, None if it failed; once all have, fills the folder
/// with their results, or notes that it failed: one search failed, or the session is signed
/// out, which a sign-in tab tells.
fn answered(
    folder: NodeId,
    answers: &Rc<RefCell<Answers>>,
    index: usize,
    answer: Option<github::Answer>,
) {
    let done = {
        let mut answers = answers.borrow_mut();
        answers.failed |= answer.is_none();
        answers.answers[index] = answer;
        answers.left -= 1;
        answers.left == 0
    };
    if !done {
        return;
    }
    SEARCHING.with_borrow_mut(|searching| searching.remove(&folder));
    let Answers {
        lists,
        answers,
        failed,
        ..
    } = answers.replace(Answers {
        left: 0,
        lists: Vec::new(),
        answers: Vec::new(),
        failed: true,
    });
    let answered = lists.into_iter().zip(answers.into_iter().flatten());
    let combined = (!failed).then(|| github::combine(answered.collect()));
    let now = now() as u64;
    let changed = STATE.with_borrow_mut(|state| {
        let sidebar = &mut state.sidebar;
        match combined {
            Some(github::Answer::Results { items, sso }) => {
                sidebar.fill_github_folder(folder, &items, &sso, now)
            }
            Some(github::Answer::SignedOut) => {
                let sign_in = github::Item {
                    url: github::SIGN_IN_URL.to_string(),
                    title: "Sign in to GitHub".to_string(),
                    status: None,
                };
                let filled = sidebar.fill_github_folder(folder, &[sign_in], &[], now);
                Ok(filled? | sidebar.fail_github_folder(folder)?)
            }
            None => sidebar.fail_github_folder(folder),
        }
    });
    if changed == Ok(true) {
        update_sidebar(|_| ());
    }
}

wrap_urlrequest_client! {
    struct GitHubSearch {
        folder: NodeId,
        index: usize,
        body: Rc<RefCell<Vec<u8>>>,
        answers: Rc<RefCell<Answers>>,
    }

    impl UrlrequestClient {
        fn on_download_data(&self, _request: Option<&mut Urlrequest>, data: *const u8, data_length: usize) {
            if !data.is_null() {
                // SAFETY: CEF hands the chunk received, `data_length` bytes long.
                let chunk = unsafe { std::slice::from_raw_parts(data, data_length) };
                let mut body = self.body.borrow_mut();
                let room = SEARCH_SIZE.saturating_sub(body.len());
                body.extend_from_slice(&chunk[..chunk.len().min(room)]);
            }
        }

        /// A failed search or an answer that cannot be read leaves the folder as it was.
        fn on_request_complete(&self, request: Option<&mut Urlrequest>) {
            let ok = request.is_some_and(|request| {
                request.request_status() == UrlrequestStatus::SUCCESS
                    && request.response().is_some_and(|response| response.status() == 200)
            });
            let answer = ok.then(|| github::answer(&self.body.borrow())).flatten();
            answered(self.folder, &self.answers, self.index, answer);
        }
    }
}
