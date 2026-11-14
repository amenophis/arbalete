# 0012 — Importing cookies

- Status: proposed
- Date: 2026-11-14

## Context

The import (decision 0007 and the import assistant) brings Arc's sidebar and other browsers' bookmarks,
history and extensions (decision 0008), but no cookies: after switching, every site asks the user to sign in
again (issue #322). `browsers.rs` leaves them out on purpose, as the browsers encrypt them with the keychain.
Arc imports them: its import lists cookies among what it brings, next to history and bookmarks, with a switch.
Its importer reads the source profile's cookie database in a utility process, decrypts it there, and writes
each cookie through the destination profile's cookie manager, leaving out cookies the profile already has.

### Where Chromium browsers keep cookies, on macOS

- **File**: one SQLite database per profile, `<profile>/Cookies` (some versions and builds keep it in
  `<profile>/Network/Cookies`: look in both). It is locked and partly in a write-ahead log while the browser
  runs, as `History` is, so it is read from a copy with its `-wal` (`import::Copy`, as for the history).
- **Table `cookies`** (database version in `meta`, 24 in current Chromium): `host_key` (a leading `.` for a
  domain cookie, none for a host-only one), `name`, `value` (empty in practice), `encrypted_value`, `path`,
  `creation_utc`, `expires_utc`, `last_access_utc`, `last_update_utc` (µs since 1601, like the history's),
  `is_secure`, `is_httponly`, `has_expires`, `is_persistent`, `samesite` (-1 unspecified, 0 none, 1 lax, 2
  strict), `priority` (0 low, 1 medium, 2 high), `source_scheme`, `source_port`, `top_frame_site_key`
  (non-empty for a partitioned cookie, CHIPS), `has_cross_site_ancestor`, `source_type`. Older versions lack
  some of the last columns: only the columns read must exist.
- **Encryption** (Chromium's `os_crypt` on macOS): `encrypted_value` starts with `v10`, then AES-128-CBC with
  PKCS#7 padding and an IV of 16 spaces. The key is PBKDF2-HMAC-SHA1 of a password, salt `saltysalt`, 1003
  iterations, 16 bytes. The password is a random string the browser keeps in the login keychain as a generic
  password, service `<Name> Safe Storage`, account `<Name>`: `Chrome Safe Storage`, `Brave Safe Storage`,
  `Microsoft Edge Safe Storage`, `Vivaldi Safe Storage`, `Opera Safe Storage`, Arc's under its own name (to
  confirm on a machine with Arc, by the item's attributes: see below), Dia's likewise. Since database version
  24, the decrypted value starts with the SHA-256 of `host_key` (32 bytes), to drop and check. Windows' newer
  app-bound encryption (`v20`) does not exist on macOS.
- **Arbalete's own key** is `Arbalete Safe Storage`, account `Arbalete`, which CEF uses for the profiles'
  cookies under the name its `--keychain-name` switch gives (a patch of amenophis/cef-codecs' CEF, like
  Electron's apps; issue #358). Up to 0.5.0 it was Chromium's default, `Chromium Safe Storage`, which Chromium
  itself shares: `mac::ensure_keychain_access` copies it into Arbalete's item once, before CEF starts, and leaves
  it in place. It also makes sure Arbalete can read its key: without it, Chromium deletes every cookie it
  cannot decrypt.

### What reading another browser's key means for the user

- macOS shows its own dialog once per browser, not per profile: Arbalete wants to use the confidential
  information in the item “Chrome Safe Storage”, with the login keychain password to type and Always Allow,
  Deny and Allow. Nothing in Arbalete can answer or skip it, and it must not try.
- **Allow** is enough: the import reads the key once. **Always Allow** would add Arbalete to the other
  browser's item for good, which the import does not need; with Arbalete's self-signed releases (decision 0006)
  macOS would ask again after each update anyway. The assistant says to choose Allow.
- **Deny** or closing the dialog: that browser's cookies are not imported; the rest of the import goes on.
- Listing the keychain's items by their attributes (service, account), without their data, asks nothing:
  Arbalete can find which `… Safe Storage` item a browser uses before asking for it.
- Development builds use a mock keychain for Arbalete's own key, but reading another browser's key needs the
  real one: they ask too, at each import.

### How to write them into an Arbalete profile

1. **Write into CEF's `Cookies` database** directly, encrypted with Arbalete's own key, before the profile
   loads. CEF holds the database open and keeps every cookie in memory while the profile is loaded: it only
   works for a profile not loaded yet, which a merge into an existing profile never is. It also ties Arbalete
   to the exact schema of its CEF's Chromium, whose version Chromium refuses when newer than its own, and
   means encrypting with Arbalete's key in Arbalete's code.
2. **`CefCookieManager::SetCookie`** on the profile's request context (`profile_context`), one call per
   cookie, asynchronous, then `FlushStore`. Chromium encrypts and stores them as its own, whatever its schema;
   it works on a loaded profile, a new one or a merge. Limits of `CefCookie`: no partition key, so partitioned
   cookies cannot be set as such; no source scheme or port, which Chromium derives from the URL given; and it
   checks each cookie like a site's (no `;` in a value, `__Host-` cookies host-only on `/`, `Secure` on
   HTTPS), failing the call rather than storing a bad one.

## Decision

- **Which**: from a Chromium-family profile (Arc included) imported, every cookie of its `cookies` table that
  has not expired, session cookies included (Arbalete's profiles keep them across launches,
  `persist_session_cookies`, and many sign-ins live in one), except partitioned ones (`top_frame_site_key` not empty), which `CefCookie` cannot express;
  they are third-party embeds' state, not sign-ins. Firefox and Safari keep cookies otherwise (Firefox's
  `cookies.sqlite` in clear, Safari's `Cookies.binarycookies` behind Full Disk Access): left for later.
- **Asked**: a "Cookies (stay signed in)" switch per browser in the import assistant and the welcome flow's
  import step, on by default, with one line under it: macOS will ask for that browser's key, choose Allow. The
  key is asked for only when the switch is on and the import starts, once per browser.
- **Read** in `arbalete-core` (`import::cookies`), on the import's own thread, like the history: copy the
  database, read the rows, decrypt with the key given, check the host hash, convert. The key comes from
  `app` (`mac.rs`, next to `ensure_keychain_access`), found by the browser's service name and read with
  `SecItemCopyMatching`; decryption uses small pure-Rust crates (`aes`, `cbc`, `pbkdf2`, `sha1`, MIT or
  Apache-2.0), so the core stays testable without a keychain.
- **Written** with `CefCookieManager::SetCookie` (option 2), into the profile imported into, once its request
  context is ready: the URL is `https://` (or `http://` when not secure) plus `host_key` without its dot plus
  `path`; a domain cookie keeps its `domain`, a host-only one gets none; expiry, `httponly`, `secure`,
  SameSite and priority map one to one. A cookie the Arbalete profile already has (same name, domain and path)
  is kept, not replaced: it is the newer sign-in, as in Arc. `FlushStore` at the end.
- **Told**: "Import done" counts the cookies imported, and lists each browser whose cookies were not, with
  why (key not allowed, database missing or unreadable); "Try again" re-runs only the cookies of that browser.
  Cookies failing `SetCookie` one by one are counted, not listed.
- **Kept secret**: decrypted values exist only in memory, from the import thread to the cookie manager, never
  in the sidebar, the UI's JSON, logs, errors or crash reports. The cookie type's `Debug` prints neither name
  nor value; errors name the browser and a count. The other browser's key is dropped once its profiles are
  read and never stored; the database copy, still encrypted, goes with the import's temporary directory.

## Consequences

- After importing from Arc or a Chromium browser, sites stay signed in, as after Arc's import.
- One macOS keychain dialog per browser whose cookies are imported, asking for the user's password; a
  denial costs only the cookies.
- No Arbalete code encrypts cookies or knows CEF's schema: an update of CEF changes nothing here. Reading
  depends on the other browsers' schema and `v10` format, stable for years; a change shows as "cookies not
  imported" for that browser, the rest of the import unaffected.
- Partitioned cookies, sites' local storage and IndexedDB, and Firefox and Safari cookies are not imported:
  some sites keep part of a sign-in there and will ask again.
- Some sites bind a session to more than cookies (device-bound session credentials, IP or user-agent checks)
  and may still sign the user out.
- Four small crypto crates more in `arbalete-core`.
