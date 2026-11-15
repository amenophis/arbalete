---
name: release
description: Cut an Arbalete release - pick the next version, bump app/Cargo.toml through a release PR, tag vX.Y.Z on main and follow the release workflow. Use when the user asks whether a release is due, or to release, publish or tag a version.
---

# Release

A release is an annotated `vX.Y.Z` tag on main whose version matches `app/Cargo.toml`. Pushing the tag
starts `.github/workflows/release.yml`, which builds and signs the disk image, publishes the GitHub
release with notes from `packaging/release-notes.sh`, updates the cask in `amenophis/homebrew-tap` and
deploys the website (whose appcast feeds the in-app updates).

## 1. Is a release due?

```bash
git fetch -q origin
t=$(git describe --tags --abbrev=0 origin/main)
git log --format='%s' "$t"..origin/main
```

Only `feat` and `fix` commits reach users; `docs`, `build`, `ci`, `refactor`, `test` and `chore` alone are
no reason to release. Summarize the user-facing changes for the user, in the language they speak to you.

Pick the version (SemVer, still 0.x):

- any `feat` → bump the minor: `0.5.0` → `0.6.0`;
- only `fix` → bump the patch: `0.5.0` → `0.5.1`;
- a breaking change (`!` or `BREAKING CHANGE`) → bump the minor while in 0.x.

Propose the version and **wait for the user's yes**: the tag publishes to every user.

## 2. Bump the version through a PR

Work from an up-to-date main, with nothing else in the change:

```bash
git switch main && git pull --ff-only
git switch -c build/release-X.Y.Z
sed -i '' 's/^version = "OLD"/version = "X.Y.Z"/' app/Cargo.toml
cargo update -p arbalete --offline
git diff --stat          # app/Cargo.toml and Cargo.lock only
mise run check
git add app/Cargo.toml Cargo.lock
git commit -m "build: release X.Y.Z"
git push -u origin build/release-X.Y.Z
gh pr create --title "build: release X.Y.Z" \
  --body "Bumps the app to X.Y.Z. Once merged, tag vX.Y.Z on main to publish the release."
```

The commit and PR carry no tool attribution (CONTRIBUTING.md, CLAUDE.md).

## 3. Merge and tag

When the PR's `check` is green (`gh pr checks N --watch`):

```bash
gh pr merge N --rebase --delete-branch
git switch main && git pull --ff-only
git log -1 --format=%s      # build: release X.Y.Z
git tag -a vX.Y.Z -m "Arbalete X.Y.Z"
git push origin vX.Y.Z
git branch -D build/release-X.Y.Z
```

The workflow refuses a tag that is not the app's version or not on main.

## 4. Follow the workflow

```bash
gh run list --workflow release.yml --limit 1
gh run watch <id> --exit-status
gh release view vX.Y.Z
```

When it succeeds, give the user the release link. When a step fails, read its log
(`gh run view <id> --log-failed`) and report it; never delete or move a pushed tag without the user's yes.
