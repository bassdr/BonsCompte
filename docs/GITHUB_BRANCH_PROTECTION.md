# GitHub Branch Protection Setup

This document explains how to configure GitHub to require tests to pass before merging pull requests.

## Branch Protection Rules

To prevent merging pull requests until the test workflow passes, you need to enable branch protection rules on GitHub.

### Setup Instructions

1. **Go to your repository on GitHub**
   - Navigate to https://github.com/bassdr/BonsCompte

2. **Open Settings**
   - Click on "Settings" tab (requires admin access)

3. **Navigate to Branch Protection**
   - In the left sidebar, click "Branches" under "Code and automation"

4. **Add a Branch Protection Rule**
   - Click "Add branch protection rule" or "Add rule"

5. **Configure the Rule**

   **Branch name pattern:**
   ```
   main
   ```

   **Required settings to enable:**

   ✅ **Require a pull request before merging**
   - Require approvals: 1 (optional, for code review)
   - Dismiss stale pull request approvals when new commits are pushed (recommended)

   ✅ **Require status checks to pass before merging** (IMPORTANT)
   - Require branches to be up to date before merging (recommended)
   - **Add required status checks:**
     - `Backend Tests` (from the workflow)
     - `Frontend Tests` (from the workflow)

   ✅ **Require conversation resolution before merging** (optional but recommended)

   ✅ **Do not allow bypassing the above settings** (recommended)
   - This prevents even admins from bypassing the rules

6. **Save Changes**
   - Click "Create" or "Save changes"

### Optional: Protect the `develop` branch too

Repeat the above steps with branch name pattern:
```
develop
```

## How It Works

Once configured:

1. **Direct pushes to main are blocked** - all changes must go through pull requests
2. **PRs cannot be merged until tests pass** - both backend and frontend tests must succeed
3. **Status checks are visible** - PR page shows test status clearly
4. **Merge button is disabled** - until all requirements are met

## Visual Confirmation

After setup, pull requests will show:

```
✅ Backend Tests — Passed
✅ Frontend Tests — Passed
✅ All checks have passed
```

The "Merge pull request" button will only be enabled when all checks pass.

## Troubleshooting

### "Required status checks not found"

If the status checks don't appear in the dropdown:

1. Make sure you've run the workflow at least once (push to main or create a test PR)
2. The status check names must exactly match the job names in `.github/workflows/test.yml`:
   - `backend-tests` → Shows as "Backend Tests"
   - `frontend-tests` → Shows as "Frontend Tests"

### "I can still merge without tests passing"

- Check that you enabled "Do not allow bypassing the above settings"
- Make sure you're not using "Rebase and merge" or "Squash and merge" if those are configured differently
- Verify the rule is applied to the correct branch

## Testing the Setup

1. Create a test branch that intentionally breaks a test
2. Open a pull request to `main`
3. Verify that:
   - Tests run automatically
   - The failing test blocks the merge
   - The "Merge" button is disabled or shows a warning

## Reference

- [GitHub Branch Protection Rules Documentation](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
- [GitHub Status Checks Documentation](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/collaborating-on-repositories-with-code-quality-features/about-status-checks)
