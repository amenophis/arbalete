# 0013 — GitHub folders

- Status: proposed
- Date: 2026-11-23

## Context

Issue #328 asks for a sidebar folder that keeps itself filled with the user's GitHub pull requests or issues
(review requested, assigned, authored), as Arc's live folder does with pull requests, updated as they change. Arbalete must read the user's GitHub data for it,
and so prove who the user is to GitHub.

The ways to read it:

1. **A token for GitHub's REST or GraphQL API** (`api.github.com`): a personal access token the user pastes, or one
   from an OAuth app's device flow, which needs an OAuth app registered for Arbalete. The API is documented and
   stable. Arbalete then holds a secret that reaches the user's repositories: it must keep it in the keychain,
   which asks again after each update of a self-signed release (decision 0006), offer to revoke and replace it,
   and keep it out of logs, crash reports and tests. The user signs in a second time, apart from the github.com
   session they already have in the profile.
2. **The profile's own github.com session**: github.com's search page, `https://github.com/search?q=…&type=…`,
   answers a request with `Accept: application/json` with the JSON its own interface renders: the results
   (repository owner and name, number, title with search highlights as HTML) and whether the request was signed
   in (`logged_in`). A request through the profile's request context carries the profile's github.com cookies, as
   the page itself would, so `@me` in the query is the user signed in there. Arbalete never sees or stores a
   secret: the cookies stay in the profile's web data, where CEF keeps them. The format is not a documented API
   and may change without notice; the search is rate limited as the user's own searches are.

## Decision

GitHub folders read github.com's search through the profile's session (2).

- **A live folder of the sidebar, like Arc's**: one "Pull Requests" folder a Space, created from the Space's
  menu, Live Folder ▸ GitHub, in its pinned tabs. It lists pull requests only: Arc's has no issues, and they would not fit
  its one folder and its three options. It keeps the searches it shows together (`NodeKind::Folder`'s `github`),
  each of `arbalete_core::github::LISTS`, its menu's options: Created by Me, Review Requests (asked of the user),
  Team Review Requests (asked of the user's teams too); a new one shows all three, and it always shows one, each
  result once. It is its profile's: a folder in a work Space lists what the work profile's GitHub account sees.
  Its icon is a folder bearing GitHub's logo, which also marks it in the menus: Primer Octicons' `mark-github`,
  under the MIT license, kept with its notice in `app/resources/github`.
- **Its menu**, like Arc's: a Pull Requests submenu first, saying how its last update went (Initializing…, Last
  fetched … ago, or Failed to update), then Refresh and the three options; below them, the repositories of its
  results, each of which may be hidden from it, its tabs going at once, and the organizations whose single
  sign-on hides results. Then Move to, Rename… and Delete Live Folder, which asks first.
- **Added on opening a pull request**: like Arc's, a tab of a Space without a GitHub folder loading a pull
  request's page adds one, and a toast tells. Deleting a GitHub folder turns that off, as Arc's does; creating one
  from the menu turns it back on. An incognito window adds none.
- **Its tabs are the results**, as pinned tabs on each result's page, in the search's order: the open ones and those
  closed in the last three days, most recently updated first, so that a pull request merged or closed stays a while,
  saying so. Like Arc's, whose full search asks open pull requests only and whose later ones, updated since,
  tell what became of those it shows, a pull request merged or closed stays only if the folder showed it open:
  it never adds one. Under its title, a row says what the search tells of its result: merged, closed, a draft, or, from
  the searches it is found in, changes requested, approved, or its review requested; like Arc's, every folder
  also asks the searches of pull requests approved and with changes requested that involve the user, which add
  no result but tell those. The search's results do not tell a pull request's checks or conflicts, which only its
  own page does: reading each pull request's page at each refresh would cost a request per result, so they are not
  shown.
  A result that comes after the folder's first ones is new until its tab is selected: its row shows a dot. Each refresh keeps
  the tabs of results still found, a page open in them included, adds the new ones and removes the others,
  closing their pages; the tab shown stays, last, until another one shows, so that a pull request merged while
  it is read does not vanish. A result the user removes stays out of the folder, which keeps its page's address.
  A long one shows its first eight results, and the tab selected, above a Show More row showing them all.
  Nothing else goes into the folder: tabs and folders cannot be moved or opened into it.
- **Refreshes**: once the profile's web data is ready, when the folder is created or its menu says Refresh, and
  every five minutes. Like Arc's, also when what it lists may have changed, unless it asked within ten seconds:
  when the app comes in front, its Space shows, one of its results is selected, or a page of github.com loads in
  its profile, which may be the user signing in, opening or reviewing a pull request. A failed search or an
  answer that cannot be read leaves the folder's tabs as they were; like Arc's, its row then shows a warning
  sign, and its menu says Failed to update, until an update works.
- **Signed out**: the folder holds a single tab, "Sign in to GitHub", on github.com's sign-in page, and warns as
  a failed update does; signing in there fills it at the next refresh.
- **Single sign-on**: the search's JSON names the organizations whose results it leaves out until the session
  signs in to them with their SSO (`protected_org_logins`). Like Arc's, the folder's row then warns, and its menu
  lists them under Single Sign-On Required, each opening its SSO page (`https://github.com/orgs/<org>/sso`); the
  next refresh after lists its results. That field's meaning is inferred from its name and Arc's warning: no test had such an
  organization.
- **The request**: a `GET` from the browser process with the profile's request context, its stored credentials
  allowed and github.com as its first party, so the session's `SameSite` cookies go with it, and `Accept:
  application/json`. Reading the JSON is pure code in `arbalete_core::github`, tested against generated fixtures
  only: no test reaches GitHub or uses a real session.

## Consequences

- No token, no OAuth app, no second sign-in, no secret in Arbalete's own storage; signing out of github.com in the
  profile empties the folder.
- A change to github.com's search JSON breaks the folders, which then stay as they were, until
  `arbalete_core::github` reads the new format. Moving to the documented API (1) remains possible later, with a
  token kept in the keychain, should the format prove unstable.
- Each folder costs one request per option shown, plus two for the reviews, every five minutes, and on the events above, from the profile's own
  network context.
