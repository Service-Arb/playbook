#!/usr/bin/env bash
# git hook: each LFS object is one inode, shared by .git/lfs and every checkout; see docs/ARCHITECTURE.md#lfs
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
[ -z "$(git config --get lfs.storage)" ] || { echo "lfs-link: lfs.storage is set, objects are not under .git/lfs" >&2; exit 1; }
objects="$(git rev-parse --path-format=absolute --git-common-dir)/lfs/objects"

declare -A changed
while IFS= read -r -d '' p; do changed[$p]=1; done < <(git diff -z --name-only --no-renames HEAD)

git lfs ls-files -l | while read -r oid mark path; do
	[ "$mark" = '*' ] || continue # no local object; the file may be a bare pointer
	[ -z "${changed[$path]:-}" ] || continue # differs from HEAD, so not $oid's content
	o="$objects/${oid:0:2}/${oid:2:2}/$oid"
	[ "$path" -ef "$o" ] && continue
	ln -f "$o" "$path"
	chmod a-w "$o" # shared inode: an in-place write fails instead of rewriting the object under every checkout
done

git lfs prune >/dev/null
