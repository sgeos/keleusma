#!/bin/sh
#
# check-md-links.sh -- verify that relative links in Markdown files
# resolve to existing files.
#
# Usage:
#   scripts/check-md-links.sh [path ...]
#
# With no arguments it scans the current directory tree. Each argument
# may be a directory (scanned recursively) or an individual Markdown
# file. The directories target, .git, node_modules, and tmp (a
# scratch area, see tmp/.gitkeep) are pruned.
#
# For every inline Markdown link `[text](target)` (and image link
# `![alt](target)`) in a non-code-fenced line, the target is resolved
# relative to the containing file's directory and checked for
# existence. A target that does not resolve is reported as
# `file:line: broken link -> target`. The script exits 0 when every
# relative link resolves and 1 when any does not, so it is usable as a
# continuous-integration gate.
#
# Scope and limitations, stated so the check is not mistaken for more
# than it is:
#   - External links (http, https, ftp, mailto, and protocol-relative
#     `//host`) are skipped.
#   - A pure-anchor link (`#section`) is skipped, and an anchor fragment
#     on a relative link (`file.md#section`) is stripped before the
#     existence check. Fragments are not validated against headings.
#   - Reference-style links (`[text][ref]` with a separate `[ref]: url`
#     definition) are not resolved; only inline links are.
#   - Backtick code fences are skipped so link-shaped text inside code
#     examples is not flagged. Indented (four-space) code blocks are not
#     skipped.
#   - A link to a directory passes when the directory exists.
#
# POSIX sh. No bashisms.

set -u

if [ "$#" -eq 0 ]; then
	set -- .
fi

tmp_dir="${TMPDIR:-/tmp}"
links="$tmp_dir/check-md-links.$$"
trap 'rm -f "$links"' EXIT INT TERM

# Extract every candidate link as a tab-separated
# "file<TAB>line<TAB>target" record. One awk invocation per file keeps
# the file name explicit and the code-fence state local to the file.
find "$@" \
	-type d \( -name target -o -name .git -o -name node_modules -o -name tmp \) -prune \
	-o -type f -name '*.md' -print \
| while IFS= read -r f; do
	awk -v F="$f" '
		# Count the leading run of backtick characters on a line.
		function backtick_run(line,    i) {
			i = 0
			while (substr(line, i + 1, 1) == "`") {
				i++
			}
			return i
		}
		{
			n = backtick_run($0)
			if (in_fence) {
				# A closing fence is a backtick run at least as
				# long as the opening run, followed only by
				# whitespace.
				if (n >= fence_len && substr($0, n + 1) ~ /^[[:space:]]*$/) {
					in_fence = 0
				}
				next
			}
			if (n >= 3) {
				in_fence = 1
				fence_len = n
				next
			}
			# Pull every "](target)" occurrence from the line.
			s = $0
			while ((p = index(s, "](")) > 0) {
				s = substr(s, p + 2)
				q = index(s, ")")
				if (q == 0) {
					break
				}
				tgt = substr(s, 1, q - 1)
				print F "\t" FNR "\t" tgt
				s = substr(s, q + 1)
			}
		}
	' "$f"
done > "$links"

broken=0
checked=0

# A literal tab is the field separator written by awk above.
while IFS='	' read -r file lineno target; do
	# Drop an optional link title: take the text before the first
	# space. Then drop an anchor fragment.
	path=${target%% *}
	path=${path%%#*}

	case "$path" in
		"")
			# Pure anchor or empty target; nothing to resolve.
			continue
			;;
		http://* | https://* | ftp://* | mailto:* | //*)
			# External or protocol-relative; out of scope.
			continue
			;;
	esac

	checked=$((checked + 1))
	dir=$(dirname "$file")
	resolved="$dir/$path"
	if [ ! -e "$resolved" ]; then
		printf '%s:%s: broken link -> %s\n' "$file" "$lineno" "$target"
		broken=$((broken + 1))
	fi
done < "$links"

if [ "$broken" -gt 0 ]; then
	printf '\n%d broken relative link(s) found (of %d checked).\n' \
		"$broken" "$checked" >&2
	exit 1
fi

printf 'All %d relative Markdown links resolve.\n' "$checked"
exit 0
