---
name: release-sideglance
disable-model-invocation: true
description: Create and verify a major or minor GitHub release for sideglance, with an explicit version and publish confirmation.
---

# Release sideglance

Use this skill only when explicitly invoked to release this repository. Model invocation is disabled.

## Preflight before confirmation

Complete read-only checks before asking the user to choose or approve anything. Do not edit files, stash work, switch branches, update local branches, commit, push, tag, or create a release yet.

- Check the current branch, upstream, working tree, GitHub CLI authentication, remote repository, and default branch. Release from `main` only. Inspect `origin/main` on GitHub and compare its commit with local `main` to identify a needed update or divergence without changing local state.
- Read the version from `package.json`, `package-lock.json`, and `src-tauri/Cargo.toml` at the intended release base on `origin/main`. They must agree. Report any differences from the local checkout so the version choices describe what will actually be released.
- Compute the major and minor candidates. If the invocation already selects a bump or exact version, use that candidate; otherwise present both, for example `2.1.0 → 2.2.0` and `2.1.0 → 3.0.0`.
- Check each offered `v<version>` tag locally and remotely and its GitHub release. Do not offer an existing tag or release as available, and never overwrite, retag, or delete one.
- Identify the previous published, non-draft, non-prerelease GitHub release whose tag is an ancestor of the intended release base. Use the nearest such tag in that history, not an unrelated newer tag or the most recently edited release. Resolve both endpoints to commits and review `<previous-tag>..<release-base>` with commit subjects, bodies, and the net diff. Inspect relevant PR descriptions and individual diffs where commit messages do not explain the behavior. If no previous release exists, review history through the release base and label this as the initial release.
- Draft release notes using the guidance below and include them in the combined confirmation so the user can review what will be published.
- Gather all issues in this pass, including uncommitted files, a different current branch, divergence, version disagreement, authentication failures, and unavailable candidates. Present safe resolutions where possible. Do not silently include or discard unrelated work.

## One combined confirmation

Present one concise request containing the repository, release base commit on `main`, current and proposed versions, preflight results, and all decisions needed to proceed. Include any proposed stash, branch switch, or fast-forward update in that request. If an issue prevents a safe publication plan, report it together with the other findings rather than asking for publication prematurely.

The user must choose a major or minor bump and give an unambiguous go-ahead for publication and the presented resolutions. One reply can authorize all of these. For example: "Publish 2.2.0, stash the listed changes, and update main." Do not ask separately for the version and then for preflight decisions already discoverable above. Honor authorization already given in the invocation or session; ask only for missing decisions.

After confirmation, apply only the agreed resolutions, preserving local work. Recheck the release base, working tree, version, tag, and release availability before preparing the release. Routine execution of the approved plan needs no further confirmation. Stop only for a genuinely new blocker or a material change to the approved version, release base commit, or scope. The release version commit created below is part of the approved plan. Approval from an earlier attempt does not authorize a different version or release base.

## Write meaningful release notes

Write for Sideglance users deciding what changed or whether to update. Synthesize the commits since the previous release into concrete changes and their effects. Do not paste a commit log or rely on GitHub's generated PR-title list.

- Lead with a short overview of the most useful changes. Group bullets under headings such as `Added`, `Improved`, and `Fixed` only where useful; omit empty sections.
- Describe visible behavior and the conditions of a fix. For example, prefer "Status updates from simultaneous sessions no longer overwrite each other" over "Fix locking". Include breaking changes and required upgrade actions when the evidence supports them.
- Combine related commits into one item. Account for reverts and follow-up fixes so notes describe the final behavior at the release base. Omit version bumps, merge commits, and routine internal refactors, tests, or tooling unless they affect users or contributors in a meaningful way.
- Ground every claim in the reviewed commits, diffs, or PRs. Do not invent benefits, performance gains, or compatibility claims. Add PR or issue links where they help explain a change.
- End with a full comparison link from the previous release tag to `v<new-version>` when a previous release exists. For an initial release, summarize the delivered capabilities without claiming a previous-version comparison.

Save the reviewed Markdown to a temporary notes file outside the repository. Before publishing, verify its scope against the exact release commit, including any release-related fixes made during preparation. The version-only commit needs no separate note. A material scope change still requires confirmation under the rules above.

## Prepare and publish

1. Update the version consistently in:
   - `package.json`
   - `package-lock.json`
   - `src-tauri/Cargo.toml`
   - the root-package entry in `src-tauri/Cargo.lock`, if Cargo updates it

   Prefer `npm version <new-version> --no-git-tag-version` for the npm files. Update the Cargo manifest, then run a Cargo command that refreshes its lockfile when needed. Do not change dependency versions.
2. Inspect the diff and run `git diff --check`, `npm run build`, and `cargo check --manifest-path src-tauri/Cargo.toml`. Resolve release-related failures before committing.
3. Commit only the version changes with `Release v<new-version>`, then push that commit to `origin/main`.
4. Create the GitHub release with GitHub CLI, targeting the pushed commit and using tag `v<new-version>`. Publish the reviewed notes with `--notes-file`, without `--generate-notes`. The release workflow builds every signed platform bundle, then uploads one combined update manifest after the builds pass.

   ```sh
   gh release create "v<new-version>" --target "$(git rev-parse HEAD)" --title "sideglance v<new-version>" --notes-file /absolute/path/to/release-notes.md
   ```

## Follow through to completion

The release workflow is `.github/workflows/release.yml`; it starts when a GitHub release is created, builds macOS, Linux, and Windows bundles, then uploads `latest.json` after all build jobs succeed.

1. Find the workflow run associated with the new release, allowing briefly for it to appear, then follow it with `gh run watch <run-id> --exit-status`.
2. Treat the release as successful only when all workflow jobs have succeeded and `gh release view v<new-version> --json isDraft,url,assets` confirms a published (non-draft) release with uploaded assets. Report the release URL and run URL.
3. If the run fails, inspect `gh run view <run-id> --log-failed` and the failed job metadata, diagnose the cause, and fix issues that are clearly in scope.
   - For a transient infrastructure failure, rerun the failed jobs with `gh run rerun <run-id> --failed`, then watch and verify again.
   - For a source fix, commit and push the correction. The existing tag points to the old commit, so do not claim that new commit has fixed the current release.
   - Never force-move a tag or replace a release. Explain the failure and ask the user for approval of a new release version and publication plan before making a corrective release.
4. Continue the check/fix/retry cycle until verified success or until a user decision is required. Do not report success from a release creation command alone.
