# Arbalette

A cross-platform browser inspired by Arc: vertical sidebar, Spaces, profiles and link routing.
Built with Rust and React on Chromium.

> **Status:** early development. Nothing is usable yet.

## Goals

- A vertical sidebar with Favorites, pinned tabs and today's tabs.
- Spaces, each with its own tabs and theme, attached to isolated profiles.
- Chrome extensions, installed from the Chrome Web Store.
- Automatic archiving of inactive tabs, and resource saving for background tabs.
- Link routing: send links opened from other apps to the right Space.
- A Command Bar for URLs, search, tabs and history.
- Import from Arc and from the browsers installed on your system.
- macOS first, then Linux and Windows.

See [docs/PLAN.md](docs/PLAN.md) for the full plan.

## Development

The tools (Rust, Node.js, pnpm, CMake, Ninja) are pinned in [.config/mise.toml](.config/mise.toml).
With [mise](https://mise.jdx.dev/) installed:

```sh
mise install     # install the tools
mise run setup   # install bundle-cef-app, the CEF bundling tool
mise run run     # build the UI, bundle the app and run it
```

The first build downloads CEF (about 350 MB). Set `CEF_PATH` to an exported CEF directory to share it between
builds.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) first: simplicity is the project's doctrine.
To report a security issue, see [SECURITY.md](SECURITY.md).

## License

[Mozilla Public License 2.0](LICENSE).

Arbalette is an independent project, not affiliated with or endorsed by The Browser Company.
Arc is a trademark of its owner.
