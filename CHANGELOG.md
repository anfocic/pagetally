# Changelog

Notable changes to this project. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/).

## [Unreleased]

### Changed
- **Renamed the project from `pagetally` to `dullahan`** — crate, binary, and
  repository. (A headless rider of Irish folklore: a mythic nod to a *headless*
  backend.) Nothing was published under the old name, so there is no upgrade path
  to worry about.
- The browser tracker is **no longer a published npm package**. It is now a
  private build tool in `tracker/`, compiled into the server
  (`server/assets/pt.js`) and served at `/pt.js`. `cargo install dullahan` needs
  no Node, and the shipped artifact is a single Rust binary that serves its own
  tracker.

### Added
- **Blog / content API**: `GET /posts`, `GET /posts/:slug`,
  `POST /posts/:slug/view` (public), and `POST /posts`, `PATCH /posts/:id`,
  `DELETE /posts/:id` (admin) — a headless content store with an atomic per-post
  view counter, reusing the existing `ADMIN_TOKEN` bearer auth. Markdown is
  stored and returned raw (rendered by the caller).
