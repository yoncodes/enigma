# Contributing

1. Open or claim an issue.
2. Branch from `dev`.
3. Follow the current code style and conventions.
4. Test the change.
5. Open a focused pull request into `dev` and link the issue.

## Releases

Land changes in `dev`; do not commit directly to `main` or merge a `dev` pull
request with GitHub's merge button. Promote a release by fast-forwarding
`main` to the existing `dev` commit:

```powershell
git fetch origin
git push origin refs/remotes/origin/dev:refs/heads/main
```

Git rejects the push if `main` cannot fast-forward, preserving both branches
without a force push. After a successful promotion, `dev` and `main` point to
the same commit.

## AI usage

AI use is fine. Verify the result before submitting and check the box in the
pull request template.
