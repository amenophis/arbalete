//! GitHub folders: live folders listing the user's pull requests, read from github.com's search
//! as the profile's own session sees it (decision 0013).

use crate::search;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// What a GitHub folder keeps: what it lists, what the user took out of it, and how its last
/// update went.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    /// The `LISTS` it shows together, by their keys.
    pub lists: Vec<String>,
    /// The pages of the results the user removed, which stay out of it.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub hidden: BTreeSet<String>,
    /// The repositories, as "owner/name", whose results it does not show.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub hidden_repositories: BTreeSet<String>,
    /// What its results' rows say of them, by their pages: merged, a draft, approved…
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub statuses: BTreeMap<String, String>,
    /// The pages of the results that came after its first ones and were not shown since.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub unseen: BTreeSet<String>,
    /// When it was last filled, in milliseconds since the Unix epoch; None until it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched: Option<u64>,
    /// Its last update failed, or found the session signed out.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub failed: bool,
    /// The organizations whose single sign-on hides their pull requests from it.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub sso: BTreeSet<String>,
}

impl Folder {
    /// What its menu says of its last update, `now` in milliseconds since the Unix epoch.
    pub fn state(&self, now: u64) -> String {
        match self.fetched {
            _ if self.failed => "Failed to update".to_string(),
            None => "Initializing…".to_string(),
            Some(fetched) => {
                let minutes = now.saturating_sub(fetched) / 60_000;
                let ago = match minutes {
                    0 => "just now".to_string(),
                    1 => "a minute ago".to_string(),
                    2..60 => format!("{minutes} minutes ago"),
                    60..120 => "an hour ago".to_string(),
                    120..1440 => format!("{} hours ago", minutes / 60),
                    _ => format!("{} days ago", minutes / 1440).replacen("1 days", "a day", 1),
                };
                format!("Last fetched {ago}")
            }
        }
    }

    /// Its row warns: its last update failed, or an organization's single sign-on hides some
    /// of its pull requests.
    pub fn warns(&self) -> bool {
        self.failed || !self.sso.is_empty()
    }
}

/// The repository, as "owner/name", of a page of github.com.
pub fn repository(url: &str) -> Option<String> {
    let path = url.strip_prefix("https://github.com/")?;
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    let (owner, name) = (parts.next()?, parts.next()?);
    Some(format!("{owner}/{name}"))
}

/// Whether a page is a pull request's, like `https://github.com/o/r/pull/12/files`.
pub fn is_pull_request(url: &str) -> bool {
    let Some(path) = url.strip_prefix("https://github.com/") else {
        return false;
    };
    let parts: Vec<_> = path.split(['/', '?', '#']).collect();
    parts.len() >= 4
        && parts[2] == "pull"
        && !parts[3].is_empty()
        && parts[3].bytes().all(|byte| byte.is_ascii_digit())
}

/// A search of pull requests, like Arc's filters and fetches.
#[derive(Debug, PartialEq)]
pub struct List {
    pub key: &'static str,
    pub title: &'static str,
    pub query: &'static str,
}

/// The searches a folder may show together, its menu's options; a new folder shows them all.
pub const LISTS: [List; 3] = [
    List {
        key: "authored",
        title: "Created by Me",
        query: "is:pr archived:false author:@me",
    },
    List {
        key: "review",
        title: "Review Requests",
        query: "is:pr archived:false user-review-requested:@me",
    },
    // Those asked of the user's teams too.
    List {
        key: "team",
        title: "Team Review Requests",
        query: "is:pr archived:false review-requested:@me",
    },
];

/// The searches every folder asks too, which add no result but tell the reviews of those shown.
pub const REVIEWS: [List; 2] = [
    List {
        key: "approved",
        title: "Approved",
        query: "is:pr archived:false review:approved involves:@me",
    },
    List {
        key: "changes",
        title: "Changes Requested",
        query: "is:pr archived:false review:changes_requested involves:@me",
    },
];

/// A list by its key.
pub fn list(key: &str) -> Option<&'static List> {
    LISTS.iter().find(|list| list.key == key)
}

/// Where a signed-out folder sends the user.
pub const SIGN_IN_URL: &str = "https://github.com/login";

/// How many days a pull request closed stays in the folders, with its status.
const KEPT_CLOSED_DAYS: u64 = 3;

/// A result: the page it opens, its title, and what its row says of it, like "Merged".
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub url: String,
    pub title: String,
    pub status: Option<String>,
}

/// What a search page answered.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    SignedOut,
    /// Its results, in its order, and the organizations whose single sign-on hides theirs.
    Results {
        items: Vec<Item>,
        sso: Vec<String>,
    },
}

/// The search page that answers a list's query with JSON, asked with `Accept: application/json`:
/// its open pull requests, and those closed in the last days, most recently updated first. `now`
/// is in milliseconds since the Unix epoch.
pub fn search_url(query: &str, now: u64) -> String {
    let since = date(now / 86_400_000 - KEPT_CLOSED_DAYS);
    let query = format!("{query} -closed:<{since} sort:updated-desc");
    format!(
        "https://github.com/search?q={}&type=pullrequests",
        search::encode(&query)
    )
}

/// The date, as YYYY-MM-DD, `days` after the Unix epoch.
fn date(days: u64) -> String {
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// What a search page's JSON says, or None when it cannot be read.
pub fn answer(json: &[u8]) -> Option<Answer> {
    let route = serde_json::from_slice::<Page>(json).ok()?.payload.route;
    if !route.logged_in {
        return Some(Answer::SignedOut);
    }
    let items = route.results.into_iter().map(|found| {
        let repository = found.repo.repository;
        Item {
            url: format!(
                "https://github.com/{}/{}/pull/{}",
                repository.owner_login, repository.name, found.number
            ),
            title: plain_text(&found.hl_title),
            status: match found.state.as_str() {
                "draft" => Some("Draft"),
                "merged" => Some("Merged"),
                "closed" => Some("Closed"),
                _ => None,
            }
            .map(str::to_string),
        }
    });
    Some(Answer::Results {
        items: items.collect(),
        sso: route.protected_org_logins,
    })
}

/// The results of a folder's searches, in their order, each once, with the organizations whose
/// single sign-on hides some; signed out if one search was. The `REVIEWS` searches add no result:
/// an open pull request's status is the search that tells most of it, changes requested,
/// approved, or its review requested.
pub fn combine(answers: Vec<(&List, Answer)>) -> Answer {
    let rank = |status: &Option<String>| {
        let statuses = ["Review Requested", "Approved", "Changes Requested"];
        status.as_deref().map_or(0, |status| {
            statuses
                .iter()
                .position(|&s| s == status)
                .map_or(9, |i| i + 1)
        })
    };
    let mut items: Vec<Item> = Vec::new();
    let mut protected: Vec<String> = Vec::new();
    for (list, answer) in answers {
        let Answer::Results {
            items: results,
            sso,
        } = answer
        else {
            return Answer::SignedOut;
        };
        for org in sso {
            if !protected.contains(&org) {
                protected.push(org);
            }
        }
        let told = match list.key {
            "review" | "team" => Some("Review Requested"),
            "approved" => Some("Approved"),
            "changes" => Some("Changes Requested"),
            _ => None,
        };
        let adds = !REVIEWS.contains(list);
        for mut item in results {
            if item.status.is_none() {
                item.status = told.map(str::to_string);
            }
            match items.iter_mut().find(|known| known.url == item.url) {
                Some(known) if rank(&item.status) > rank(&known.status) => {
                    known.status = item.status
                }
                Some(_) => {}
                None if adds => items.push(item),
                None => {}
            }
        }
    }
    Answer::Results {
        items,
        sso: protected,
    }
}

#[derive(Deserialize)]
struct Page {
    payload: Payload,
}

#[derive(Deserialize)]
struct Payload {
    #[serde(rename = "blackbirdSearchRoute")]
    route: Route,
}

#[derive(Deserialize)]
struct Route {
    logged_in: bool,
    results: Vec<Found>,
    /// The organizations whose results the search leaves out until the session signs in to
    /// them with their single sign-on.
    #[serde(default)]
    protected_org_logins: Vec<String>,
}

#[derive(Deserialize)]
struct Found {
    number: u64,
    hl_title: String,
    #[serde(default)]
    state: String,
    repo: Repo,
}

#[derive(Deserialize)]
struct Repo {
    repository: Repository,
}

#[derive(Deserialize)]
struct Repository {
    owner_login: String,
    name: String,
}

/// A title without the search's highlights, `<em>` tags, and with its HTML entities decoded.
fn plain_text(html: &str) -> String {
    let mut text = String::new();
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        rest = rest[start..]
            .find('>')
            .map_or("", |end| &rest[start + end + 1..]);
    }
    text.push_str(rest);
    [
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&#x27;", "'"),
        // Last, so that "&amp;lt;" stays "&lt;".
        ("&amp;", "&"),
    ]
    .iter()
    .fold(text, |text, (entity, char)| text.replace(entity, char))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(logged_in: bool, kind: &str, results: &str) -> Vec<u8> {
        format!(
            r#"{{"meta":{{"title":"Search results"}},"payload":{{"blackbirdSearchRoute":{{
                "logged_in":{logged_in},"type":"{kind}","results":[{results}],"result_count":2}}}}}}"#
        )
        .into_bytes()
    }

    fn found(owner: &str, name: &str, number: u64, title: &str) -> String {
        format!(
            r#"{{"id":"1","number":{number},"hl_title":"{title}","state":"open","labels":[],
                "repo":{{"repository":{{"id":2,"name":"{name}","owner_login":"{owner}"}}}},
                "issue":{{"issue":{{"pull_request_id":null}}}}}}"#
        )
    }

    #[test]
    fn lists_pull_requests_in_the_search_order() {
        let results = [
            found("octo-org", "widgets", 12, "Fix the <em>parser</em>"),
            found("someone", "tools", 3, "Add `Option&lt;T&gt;` &amp; more")
                .replace("open", "merged"),
        ];
        let answer = answer(&page(true, "pullrequests", &results.join(","))).unwrap();
        assert_eq!(
            answer,
            Answer::Results {
                items: vec![
                    Item {
                        url: "https://github.com/octo-org/widgets/pull/12".into(),
                        title: "Fix the parser".into(),
                        status: None,
                    },
                    Item {
                        url: "https://github.com/someone/tools/pull/3".into(),
                        title: "Add `Option<T>` & more".into(),
                        status: Some("Merged".into()),
                    },
                ],
                sso: vec![],
            }
        );
    }

    #[test]
    fn asks_to_sign_in_when_signed_out() {
        let signed_out = answer(&page(false, "pullrequests", "")).unwrap();
        assert_eq!(signed_out, Answer::SignedOut);
        let results = results(vec![item("https://github.com/o/r/pull/1")]);
        let combined = combine(vec![(&LISTS[0], results), (&LISTS[1], signed_out)]);
        assert_eq!(combined, Answer::SignedOut);
    }

    #[test]
    fn reads_nothing_from_another_format() {
        assert_eq!(answer(b"<!DOCTYPE html><html></html>"), None);
        assert_eq!(answer(br#"{"payload":{}}"#), None);
    }

    #[test]
    fn searches_pull_requests_open_or_closed_lately() {
        // 2026-11-23 at noon.
        let now = 20_780 * 86_400_000 + 43_200_000;
        assert_eq!(
            search_url("is:pr author:@me", now),
            "https://github.com/search?q=is%3Apr%20author%3A%40me%20-closed%3A%3C2026-11-20%20sort%3Aupdated-desc&type=pullrequests"
        );
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(11_016), "2000-02-29");
    }

    fn results(items: Vec<Item>) -> Answer {
        Answer::Results { items, sso: vec![] }
    }

    fn item(url: &str) -> Item {
        Item {
            url: url.to_string(),
            title: url.to_string(),
            status: None,
        }
    }

    #[test]
    fn combines_searches_each_result_once_with_what_tells_most() {
        let (a, b, c, d) = (
            item("https://a.test/"),
            item("https://b.test/"),
            item("https://c.test/"),
            item("https://d.test/"),
        );
        let merged = Item {
            status: Some("Merged".into()),
            ..c.clone()
        };
        let answers = vec![
            (
                list("authored").unwrap(),
                results(vec![a.clone(), b.clone()]),
            ),
            (&REVIEWS[0], results(vec![b.clone(), d])),
            (list("review").unwrap(), results(vec![b.clone(), merged])),
        ];
        let Answer::Results { items, .. } = combine(answers) else {
            panic!("results")
        };
        let statuses: Vec<_> = items
            .into_iter()
            .map(|item| (item.url, item.status))
            .collect();
        // The approved search adds no result of its own, like d.
        assert_eq!(
            statuses,
            [
                (a.url, None),
                (b.url, Some("Approved".into())),
                (c.url, Some("Merged".into())),
            ]
        );
    }

    #[test]
    fn tells_the_organizations_whose_sso_hides_results() {
        let json = String::from_utf8(page(true, "pullrequests", &found("o", "r", 1, "A"))).unwrap();
        let json = json.replace(
            r#""logged_in""#,
            r#""protected_org_logins":["acme"],"logged_in""#,
        );
        let Some(Answer::Results { items, sso }) = answer(json.as_bytes()) else {
            panic!("results")
        };
        assert_eq!(items[0].url, "https://github.com/o/r/pull/1");
        assert_eq!(sso, ["acme"]);
    }

    #[test]
    fn tells_how_its_last_update_went() {
        let mut folder = Folder::default();
        assert_eq!(folder.state(0), "Initializing…");
        folder.fetched = Some(0);
        let minute = 60_000;
        assert_eq!(folder.state(30_000), "Last fetched just now");
        assert_eq!(folder.state(5 * minute), "Last fetched 5 minutes ago");
        assert_eq!(folder.state(90 * minute), "Last fetched an hour ago");
        assert_eq!(folder.state(30 * 60 * minute), "Last fetched a day ago");
        assert!(!folder.warns());
        folder.failed = true;
        assert_eq!(folder.state(minute), "Failed to update");
        assert!(folder.warns());
    }

    #[test]
    fn keeps_an_escaped_entity_escaped() {
        assert_eq!(plain_text("a &amp;lt; b"), "a &lt; b");
    }
}
