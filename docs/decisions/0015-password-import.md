# 0013 — Importing passwords

- Status: accepted
- Date: 2026-11-23

## Context

The import brings bookmarks, history, extensions and cookies (decision 0012), but no saved passwords (issue
#224): after switching, the user signs in again where the cookies have expired, and must find each password
elsewhere. Arc's import lists passwords among what it brings, next to cookies.

Arbalete has a password manager already: Chromium's. Its page opens in a tab (`arb://password-manager`, decision
0001's "Chromium's own pages in Alloy style"), it offers to save the passwords typed, and it fills them into
sign-in forms of Alloy-style pages: a password added to a profile's store is filled into a form of that profile's
pages (checked in a test copy: both fields `:-webkit-autofill` on load).

### Where Chromium browsers keep passwords, on macOS

- **Files**: two SQLite databases per profile, `<profile>/Login Data` (on the device) and `<profile>/Login Data
  For Account` (saved for the browser's account, Chrome's default once signed in). Locked while the browser
  runs, they are read from a copy, as the cookies are.
- **Table `logins`**: `origin_url` (the form's page), `username_value`, `password_value`, `signon_realm` (the
  site, with the user name the key of a credential), `blacklisted_by_user` (1 for "never save" on a site, with
  no password), `scheme` (0 a web form, 1 and 2 HTTP authentication, 3 other), `federation_url` (a federated
  sign-in, no password). Android apps' credentials have an `android://` realm.
- **Encryption**: `password_value` is `v10` and AES-128-CBC with the browser's key, the cookies' format
  (decision 0012, `import::safe_storage`), without the host hash cookies carry since version 24.

### How to write them into an Arbalete profile

1. **Write into CEF's `Login Data`**, encrypted with Arbalete's key: the reasons decision 0012 turned this down for
   cookies hold, the database being open while the profile is loaded.
2. **A function of Arbalete's CEF** (a patch of amenophis/cef-codecs, like the silent extension install) adding
   a credential to a profile's store: a C API to keep across Chromium updates, and a new CEF build.
3. **`chrome.passwordsPrivate.addPassword`**, the API the password manager page calls for its "Add password"
   dialog, run in that page: Chromium stores the password as its own, in the profile's store, whatever its
   schema, on a loaded profile, a new one or a merge. It turns down a password the store has for the same site
   and user name, an invalid URL or an empty password.

## Decision

- **Which**: from a Chromium-family profile imported (Arc included), the passwords of both its databases saved
  for web forms (`scheme` 0, an `http` or `https` page), with a password, each site and user name once. Left out:
  "never save" sites, HTTP authentication, apps' and federated sign-ins, and notes. Firefox (`logins.json`, NSS
  encryption) and Safari (the keychain) keep theirs otherwise: left for later.
- **Asked**: a "Saved passwords" switch under the cookies' one, in the import assistant and the welcome flow, on
  by default, for the Chromium browsers ticked. The browser's key is asked for once per browser for cookies and
  passwords alike: one macOS dialog.
- **Read** in `arbalete-core` (`import::passwords`) on the import's thread, as the cookies, with the same key.
- **Written** with `passwordsPrivate.addPassword` (option 3): the app opens the password manager page of the
  profile imported into in a browser without a window, runs a script adding them one after the other once it has
  loaded, and closes it once the script tells it is done, by the page's title. They go to the device's store, not
  an account's: Arbalete has none. A password the profile already has for the same site and user name is kept,
  as the cookies are.
- **Told**: with the cookies: the welcome flow counts the passwords imported and lists each browser whose
  passwords were not, with why; the assistant says it in its toast.
- **Kept secret**: as the cookies. Passwords exist only in memory, from the import thread to the page's script,
  never in the UI's JSON, logs or errors; the password type's `Debug` prints only its page.

## Consequences

- After importing from Arc or a Chromium browser, Arbalete fills in the saved passwords, and its password manager
  lists them.
- No Arbalete code encrypts passwords or knows CEF's schema. Writing depends on a private API of Chromium's
  WebUI, which a Chromium update can change; it shows as passwords not imported, the rest of the import
  unaffected. It needs the CEF build showing Chromium's pages in Alloy style.
- The passwords pass through a script run in Chromium's page, in the renderer's memory for the time it runs.
- Notes, "never save" sites, passkeys, and Firefox's and Safari's passwords are not imported.
