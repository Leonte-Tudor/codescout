# Experiments Branch Documentation Workflow — Design

**Date:** 2026-03-15
**Status:** Approved
**Topic:** Keeping documentation in sync with features developed on the `experiments` branch

---

## Problem

Features land on the `experiments` branch before reaching `master`. Without a rule, documentation
lags behind the code — sometimes never catching up. Users who want to try bleeding-edge features
have nowhere to look, and the graduation step (cherry-pick to `master`) requires a separate doc
effort that often gets skipped.

## Goals

- Documentation is written at the same time as the feature, not after
- Experimental docs serve as draft user-facing content that graduates into the manual on merge
- Users on `master` can discover that an `experiments` branch exists and browse its features
- The graduation step is mechanical and small — no doc rewrite needed

## Non-Goals

- Bug fixes do not require documentation
- Experimental pages are not wired into the published mdBook until graduated
- No tracking table or target-chapter metadata — chapter placement is developer judgment at graduation time

---

## File Structure

```
docs/manual/src/experimental/
├── index.md                   ← landing page: instability warning + feature list
├── <feature-name>.md          ← one file per feature on experiments
└── ...
```

Experimental pages are **not added to `docs/manual/src/SUMMARY.md`** while on `experiments`.
They are browsable as raw markdown on GitHub via the experiments branch URL.

### `index.md`

- Bold instability warning at the top
- Short description of how to try experimental features (build from the branch)
- Note that this reflects what is currently in development — it may be ahead of the latest release
- Simple list of links to each feature page
- When no features are currently in development, the list says "No experimental features at this time." — the file is never deleted (doing so would break the README link)

### Feature pages (`<feature-name>.md`)

- Single `> ⚠ Experimental — may change without notice.` callout at the top
- Full user-facing documentation — written as if it will be published, because it will be
- No additional experimental boilerplate beyond the one callout

---

## CLAUDE.md Rule

Added to the **Git Workflow** section, after the existing branch strategy:

### Documenting Features on `experiments`

When adding a feature commit to `experiments`, you MUST include documentation in the same commit:

1. Create `docs/manual/src/experimental/<feature-name>.md` — written as final
   user-facing docs with a single `> ⚠ Experimental — may change without notice.`
   callout at the top.
2. Add a line to `docs/manual/src/experimental/index.md` linking to the new page.

**Only features, not bug fixes.** Bug fixes need no experimental doc.

**If a feature is removed from `experiments`** (reverted or abandoned), delete its page and
remove its entry from `index.md` in the same commit.

### Graduating a Feature (`experiments` → `master`)

When cherry-picking a feature to `master`, use `--no-commit` to bundle the doc graduation into
the same commit:

```bash
git cherry-pick --no-commit <sha>
# then make the four graduation changes:
# 1. Move docs/manual/src/experimental/<feature-name>.md to its target chapter
# 2. Remove the `> ⚠ Experimental` callout from the top of the page
# 3. Add the page to docs/manual/src/SUMMARY.md in the right place
# 4. Remove the feature's entry from docs/manual/src/experimental/index.md
git commit -m "feat(...): <description>"
```

**Rebase note:** Because the graduation commit on `master` includes additional doc changes
(file moves, callout removal, SUMMARY update), its patch differs from the original `experiments`
commit. Git will **not** auto-skip it during the subsequent `git rebase master` on `experiments`.
After rebasing, drop the now-superseded original commit manually:

```bash
git checkout experiments
git rebase master          # the original feature commit will NOT be auto-dropped
git rebase -i master       # drop the original feature commit from the list
```

---

## README Addition

A short section added after **Contributing**:

```markdown
## Experimental Features

New features land on the `experiments` branch before reaching `master`.
They may change or be removed without notice, and may not be in your installed release yet.

→ [Browse experimental features](https://github.com/mareurs/codescout/blob/experiments/docs/manual/src/experimental/index.md)
```

---

## Implementation Steps

1. Create `docs/manual/src/experimental/index.md` with instability warning and empty feature list
2. Add the **Documenting Features on `experiments`** and **Graduating a Feature** sections to `CLAUDE.md`
3. Add the **Experimental Features** section to `README.md`
4. Backfill: audit existing features on `experiments` that lack docs and add pages for them
