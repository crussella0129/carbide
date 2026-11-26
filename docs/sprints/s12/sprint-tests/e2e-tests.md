# Sprint 12 End-to-End Tests Evidence

## `verify_github_actions_ci_on_dev` (INT-0001 AC-1, AC-2, AC-5, AC-6)
- **Branch:** `dev`
- **Head SHA:** `70e25fcfcf7d7ecafe495ab173e436de8ff9ba32`
- **Trigger:** `push` to `dev` (proving `T-003` `.github/workflows/ci.yml` `branches: [main, dev]` works end-to-end)
- **GitHub Actions Run:** `45866040577` — https://github.com/crussella0129/carbide/actions/runs/45866040577
- **Overall Conclusion:** `success` (exit code 0 via `gh run watch 45866040577 --exit-status`)
- **Jobs verified:**
  1. `rustfmt + clippy` (ID `140069000374`) — `success`
  2. `build + test (ubuntu-latest)` (ID `140069000299`) — `success`
  3. `build + test (windows-latest)` (ID `140069000251`) — `success`
  4. `native compile pipeline` (ID `140069000369`) — `success`
  5. `python engine` (ID `140069000420`) — `success`
  6. `web front-end (wasm + native)` (ID `140069000074`) — `success`

## Substrate & Book Validation
- `scripts/check-substrate.sh`: `substrate-complete` (contract 4, `provider=github`, `base=main`, `work=dev`, `mergePolicy=human-approve`)
- `scripts/check-book.sh`: `valid v2 Book (5 intent chapters)`
