# Releasing amigo-sofarr

**release-please** keeps a release PR open on `main`; merging it tags the version, and
`release.yml` builds the container image `ghcr.io/amigo-labs/amigo-sofarr` for `linux/amd64`
and `linux/arm64`, pushes it and publishes the draft release.

## TL;DR

- **Write Conventional Commits** (`feat(server): …`, `fix(core): …`, AGENTS.md). `feat` /
  `fix` / `perf` / `revert` end up in the changelog and make a release; `docs`, `ci`, `chore`,
  `refactor`, `test`, `build` don't.
- **Every push to `main` opens or refreshes the release PR** ("chore: release amigo-sofarr
  X.Y.Z"): the version in `Cargo.toml` and `Cargo.lock`, plus the new `CHANGELOG.md` entry.
- **Merge the release PR to release.** release-please tags `vX.Y.Z` and creates the release as
  a draft with the changelog; `release.yml` builds and pushes the image, then publishes the
  release.

## Versioning

SemVer, one version for the whole workspace (`[workspace.package]` in `Cargo.toml`; every crate
inherits it). release-please derives the next version from the commits since the last release;
before 1.0.0 a breaking change bumps the minor version (`bump-minor-pre-major`):

| Commits since the last release | Next version |
| ------------------------------ | ------------ |
| a breaking change (`feat!:`, `BREAKING CHANGE:` footer) | minor (major from 1.0.0 on) |
| at least one `feat` | minor |
| only `fix` / `perf` / `revert` | patch |
| only hidden types (`docs`, `ci`, `chore`, …) | no release PR |

A footer `Release-As: X.Y.Z` in a commit on `main` forces a version.

Configuration: `release-please-config.json` and `.release-please-manifest.json` (the last
released version). sofarr had no release before; the manifest starts at 0.0.0 and
`bootstrap-sha` is the last commit before phase 6, so the first release PR is 0.1.0 with the
features of phase 6 as its changelog.

## What runs

| Workflow / job | Runs when | Result |
| -------------- | --------- | ------ |
| `release-please.yml` → `release-please` | every push to `main` | Opens / updates the release PR; commits `Cargo.lock` to it; on the merge of a release PR creates the tag and the draft release |
| `release.yml` → `image` (2 runners) | after a release PR is merged, or by hand | Checks tag = crate version; builds the image natively on `ubuntu-24.04` (amd64) and `ubuntu-24.04-arm` (arm64) and pushes each by digest |
| `release.yml` → `manifest` | both images pushed | Joins both into one multi-platform image tagged `X.Y.Z`, `X.Y` and `latest`; checks that it holds `linux/amd64` and `linux/arm64` |
| `release.yml` → `publish` | image complete | Publishes the draft release as latest |

The image is complete before the release is published, so the release never points to an image
that is not there. If a job fails, the release stays a draft: fix the cause and run **Actions →
Release → Run workflow** with the tag (`vX.Y.Z`); it rebuilds the image and publishes the
release.

## One-time setup

- **Token for the release PR:** the organization secret `RELEASE_PLEASE_TOKEN` (a fine-grained
  PAT with contents and pull-requests write), so CI also runs on the release PR. Without it
  the workflow's own token is used; that needs **Settings → Actions → General → Allow GitHub
  Actions to create and approve pull requests** and starts no CI on the release PR.
- **Image visibility:** the first push creates the package `amigo-sofarr` in the organization's
  packages, private by default. Make it public once under **Package settings → Change
  visibility**, so `docker pull` works without a login. The image's
  `org.opencontainers.image.source` label links the package to this repository.

## Using a release

```sh
docker pull ghcr.io/amigo-labs/amigo-sofarr:latest   # or :0.1 / :0.1.0
```

`compose.example.yml` uses the image; pin a version (`:0.1`) to update deliberately.
