# Contributing to Hiero Runtime SDK

First off, thanks for taking the time to contribute! ❤️

All types of contributions are encouraged and valued — code, documentation, bug reports, or just spreading the word. The community looks forward to your contributions. 🎉

> If you like the project but don't have time to contribute right now, that's completely fine. Other ways to support:
> - Star the repo
> - Tweet about it
> - Mention it at meetups or in your own project's README

## Code of Conduct

This project is governed by the [Linux Foundation Decentralized Trust Code of Conduct](https://www.lfdecentralizedtrust.org/code-of-conduct). By participating you are expected to uphold this code. Please report unacceptable behavior to Mike Dolan (mdolan@linuxfoundation.org) or Angela Brown (angela@linuxfoundation.org).

## Table of Contents

- [Contributing to Hiero Runtime SDK](#contributing-to-hiero-runtime-sdk)
  - [Code of Conduct](#code-of-conduct)
  - [Table of Contents](#table-of-contents)
  - [I Have a Question](#i-have-a-question)
  - [Reporting Bugs or Suggesting Enhancements](#reporting-bugs-or-suggesting-enhancements)
  - [Your First Code Contribution](#your-first-code-contribution)
  - [Pull Requests](#pull-requests)
    - [Forking](#forking)
    - [Sign Off](#sign-off)
    - [PR Lifecycle](#pr-lifecycle)
      - [Submitting](#submitting)
      - [Triage](#triage)
      - [Reviewing](#reviewing)
      - [Merge or Close](#merge-or-close)
  - [Development Setup](#development-setup)
  - [Commit Message Format](#commit-message-format)
  - [Testing](#testing)
  - [License](#license)

---

## I Have a Question

Before asking, search existing [Issues](https://github.com/WhiteFlash14/Hiero-runtime-sdk/issues) — someone may have already asked. If you still need to ask:

- Open an [Issue](https://github.com/WhiteFlash14/Hiero-runtime-sdk/issues/new)
- Provide as much context as possible about what you're running into

We'll get back to it as soon as we can.

---

## Reporting Bugs or Suggesting Enhancements

If you found a bug or have an idea for an improvement, please [open an issue](https://github.com/WhiteFlash14/Hiero-runtime-sdk/issues/new). Include:

- A clear description of what happened vs. what you expected
- Steps to reproduce (for bugs)
- The network (`testnet`, `mainnet`, `previewnet`) and Node.js version if relevant

---

## Your First Code Contribution

Look for issues labelled [**good first issue**](https://github.com/WhiteFlash14/Hiero-runtime-sdk/labels/good%20first%20issue). Comment on the issue to let us know you're working on it — a maintainer will assign it to you so no one duplicates the work.

That said, contributions are welcome beyond those issues. The most important thing is that an issue exists before the PR. If there isn't one, open it first.

---

## Pull Requests

### Forking

1. [Fork](https://guides.github.com/activities/forking/) the repository.

2. Clone your fork locally:

   ```sh
   git clone https://github.com/<your-handle>/Hiero-runtime-sdk.git
   cd Hiero-runtime-sdk
   ```

3. Add the upstream remote to stay in sync:

   ```sh
   git remote add upstream https://github.com/WhiteFlash14/Hiero-runtime-sdk.git
   ```

4. Sync your local `main` branch before starting:

   ```sh
   git pull upstream main
   ```

5. Create a branch for your change:

   ```sh
   git checkout -b feat/my-improvement
   ```

6. Make your changes, build, and test locally (see [Development Setup](#development-setup)).

7. Stage the files you changed:

   ```sh
   git add <file>
   ```

8. Enable GPG signing:

   ```sh
   git config commit.gpgsign true
   ```

9. Commit with sign-off and GPG signature:

   ```sh
   git commit --signoff -S -m "feat(mirror): add token balance endpoint"
   ```

10. Push and [open a pull request](https://github.com/WhiteFlash14/Hiero-runtime-sdk/pulls) against `main`.

---

### Sign Off

All commits must include a `Signed-off-by` line. This certifies you have the right to submit the contribution under the Apache-2.0 license, per the [Developer Certificate of Origin](https://developercertificate.org/).

Your commit should look like this in `git log`:

```
Author: Joe Smith <joe.smith@example.com>
Date:   Thu Feb 2 11:41:15 2018 -0800

    feat(mirror): add token balance endpoint

    Signed-off-by: Joe Smith <joe.smith@example.com>
```

The `Author` and `Signed-off-by` lines must match — PRs with mismatches will be rejected by the automated DCO check. Use your real name; no pseudonyms.

If you configure your Git identity, the `-s` flag adds the sign-off automatically:

```sh
git config --global user.name "Joe Smith"
git config --global user.email "joe.smith@example.com"
git commit -s -S -m "feat(mirror): add token balance endpoint"
```

---

### PR Lifecycle

#### Submitting

- Link the PR to a related issue when one exists. Use [closing keywords](https://help.github.com/en/articles/closing-issues-using-keywords) (`Fixes #123`) to auto-close the issue on merge.
- Keep commits small and independently functional — each commit should compile and pass tests on its own.
- Add tests and documentation relevant to the change. Code coverage should stay the same or increase.
- If your PR is still in progress, open it as a [Draft PR](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/about-pull-requests#draft-pull-requests) and mark it ready when done.
- After submitting, ensure all GitHub Actions checks pass before requesting a review.
- PR titles must follow the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) specification.

#### Triage

A maintainer will apply labels and assign a reviewer. If you'd like to review, feel free to assign yourself.

#### Reviewing

- All reviews use the GitHub review tool.
- A **Comment** review is for questions that don't require code changes — it does not count as approval.
- A **Changes Requested** review means code needs to change before it can merge.
- `LGTM` in a review comment signals the reviewer is happy with the change.
- PR owners should stay responsive — answer questions or update the code promptly.
- Once all comments are resolved and all reviewers have approved, the PR is ready to merge.

#### Merge or Close

PRs should stay open until merged or closed. If a PR has had no activity for 30 days, it may be closed to keep the queue manageable. It can always be reopened.

---

## Development Setup

**Prerequisites:**

| Tool | Version |
|---|---|
| Rust (stable) | ≥ 1.76 |
| Node.js | ≥ 20 |
| pnpm | ≥ 10 |

**Install and build:**

```bash
pnpm install
pnpm build          # compiles Rust native addon + TypeScript SDK
```

**Run all Rust tests:**

```bash
cargo test --workspace
```

**Run TypeScript SDK tests:**

```bash
pnpm --filter @hiero-runtime/sdk test
```

**Live testnet smoke tests** (requires credentials — see README):

```bash
HIERO_TEST_NETWORK=testnet pnpm --filter @hiero-runtime/sdk test
```

---

## Commit Message Format

Use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/):

```
<type>(<scope>): <short summary>

[optional body]

Signed-off-by: Full Name <email@example.com>
```

**Types:** `feat`, `fix`, `docs`, `test`, `refactor`, `chore`, `ci`

**Scopes:** `mirror`, `tx`, `schedule`, `core`, `bindings`, `sdk`, `ci`, `docs`

**Examples:**

```
feat(schedule): handle KeyList signatories recursively

Signed-off-by: Alice Smith <alice@example.com>
```

```
fix(mirror): normalize transaction ID to dash format for REST paths

Signed-off-by: Bob Jones <bob@example.com>
```

---

## Testing

| Suite | Command | Required |
|---|---|:---:|
| Rust unit tests | `cargo test --workspace` | Always |
| TypeScript SDK tests | `pnpm --filter @hiero-runtime/sdk test` | Always |
| Live testnet smoke | `HIERO_TEST_NETWORK=testnet pnpm --filter @hiero-runtime/sdk test` | Optional |

CI runs both Rust and TypeScript tests on every push and pull request. Make sure both pass locally before opening a PR.

---

## License

By contributing you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).
