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
- Gather all issues in this pass, including uncommitted files, a different current branch, divergence, version disagreement, authentication failures, and unavailable candidates. Present safe resolutions where possible. Do not silently include or discard unrelated work.

## One combined confirmation

Present one concise request containing the repository, release base commit on `main`, current and proposed versions, preflight results, and all decisions needed to proceed. Include any proposed stash, branch switch, or fast-forward update in that request. If an issue prevents a safe publication plan, report it together with the other findings rather than asking for publication prematurely.

The user must choose a major or minor bump and give an unambiguous go-ahead for publication and the presented resolutions. One reply can authorize all of these. For example: "Publish 2.2.0, stash the listed changes, and update main." Do not ask separately for the version and then for preflight decisions already discoverable above. Honor authorization already given in the invocation or session; ask only for missing decisions.

After confirmation, apply only the agreed resolutions, preserving local work. Recheck the release base, working tree, version, tag, and release availability before preparing the release. Routine execution of the approved plan needs no further confirmation. Stop only for a genuinely new blocker or a material change to the approved version, release base commit, or scope. The release version commit created below is part of the approved plan. Approval from an earlier attempt does not authorize a different version or release base.

## Prepare and publish

1. Update the version consistently in:
   - `package.json`
   - `package-lock.json`
   - `src-tauri/Cargo.toml`
   - the root-package entry in `src-tauri/Cargo.lock`, if Cargo updates it

   Prefer `npm version <new-version> --no-git-tag-version` for the npm files. Update the Cargo manifest, then run a Cargo command that refreshes its lockfile when needed. Do not change dependency versions.
2. Inspect the diff and run `git diff --check`, `npm run build`, and `cargo check --manifest-path src-tauri/Cargo.toml`. Resolve release-related failures before committing.
3. Commit only the version changes with `Release v<new-version>`, then push that commit to `origin/main`.
4. Create the GitHub release with GitHub CLI, targeting the pushed commit and using tag `v<new-version>`. The release workflow builds every signed platform bundle, then uploads one combined update manifest after the builds pass.

   ```sh
   gh release create "v<new-version>" --target "$(git rev-parse HEAD)" --title "sideglance v<new-version>" --generate-notes
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
