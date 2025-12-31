Make a release of CIHub.

Version or release type: "$ARGUMENTS"

## Step-by-Step Process:

### 1. Determine the target version

The `$ARGUMENTS` can be either:
- An explicit version number (e.g., `v0.0.4`) - recommended for making a specific release
- A release type: `patch`, `minor`, or `major` - which will bump from the current version

If `$ARGUMENTS` is an explicit version (e.g., `v0.0.4`):
- Use that version directly as `$NEW_VERSION`
- This is the recommended approach as it allows retrying failed releases

If `$ARGUMENTS` is a release type (`patch`, `minor`, or `major`):
- Determine what the new version will be by getting the most up-to-date tag using command. You can find it with `git describe --tags --abbrev=0`.
- Then use this `$NEW_VERSION` for the rest of the process

If no argument is provided, ask the user which version or type to use.

### 2. Update the changelog

Run the `/update-changelog` command to ensure the changelog is up to date with recent changes.

### 3. Verify the version number

Double-check that `$NEW_VERSION` is correct before proceeding.

### 4. Update CHANGELOG.md

Edit the `CHANGELOG.md` file:
- Change the `# Unreleased` heading to `# $NEW_VERSION`
- Add a new `# Unreleased` section at the top (empty for now)

### 5. Update the version module

The module version keeps the current version of the project. Update it based on new version.

### 6. Show push instructions

After the release script completes, show the user the commands to push:

```bash
git push origin main && git push origin $NEW_VERSION
```

**Important:** Do NOT automatically push. Let the user review the commit and tag first, then they can manually run the push commands.
