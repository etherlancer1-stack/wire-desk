# Wire Desk — Anti-Loss Protocol (Output Block Z)

Run immediately after generating code, and after every meaningful work session.

```bash
# 1. Stage safe files (excludes secrets, DB, logs, backups, build artifacts)
bash scripts/snapshot_git.sh --tag wire-desk

# 2. Create a bundle backup
bash scripts/snapshot_git.sh --bundle 1

# 3. Tag and push
git tag -a "v0.1.0-$(date +%Y%m%d-%H%M%S)" -m "auto snapshot"
git push origin HEAD --tags

# 4. Backup runtime state
bash scripts/backup.sh

# 5. Verify integrity
bash scripts/restore.sh <backup>.tar.gz --dry-run  # or run doctor
```

Files never committed (`.gitignore`): `.env`, `*.db`, `backups/`, `logs/`,
`tmp/`, `target/`, `node_modules/`, `data/*.json`.
