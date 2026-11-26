# Sprint 13 End-to-End Tests Evidence

## `verify_github_actions_ci_on_dev` (INT-0002 AC-2, AC-3)
- **Branch:** `dev`
- **Head SHA:** `2c65eaa6418e055a9d197ff5237e8b64793b05b3` (includes `T-001`–`T-004` + clean merge of user-approved Dependabot PRs #194 and #195 on `origin/dev`)
- **GitHub Actions Run:** `45872819726` — https://github.com/crussella0129/carbide/actions/runs/45872819726
- **Overall Conclusion:** `success` (exit code 0 via `gh run watch 45872819726 --exit-status`)
- **Jobs verified:**
  1. `rustfmt + clippy` (ID `140092309208`) — `success`
  2. `build + test (ubuntu-latest)` (ID `140092309259`) — `success`
  3. `build + test (windows-latest)` (ID `140092309281`) — `success`
  4. `native compile pipeline` (ID `140092309124`) — `success`
  5. `python engine` (ID `140092309211`) — `success`
  6. `web front-end (wasm + native)` (ID `140092308801`) — `success`

## Substrate & Book Validation
- `scripts/check-substrate.sh`: `substrate-complete` (contract 4, `provider=github`, `base=main`, `work=dev`, `mergePolicy=human-approve`)
- `scripts/check-book.sh`: `valid v2 Book (5 intent chapters)`
