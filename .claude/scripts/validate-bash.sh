#!/bin/bash

# PreToolUse hook for Bash commands.
#
# Goal: keep the agent from reading/searching/listing generated artifact
# directories (build output, caches, vendored deps) so it doesn't waste time
# and context spelunking through them.
#
# Important: we only apply the deny-list to commands that actually READ or
# TRAVERSE the filesystem (cat, grep, ls, find, ...). We do NOT block a path
# just because the string appears somewhere on the command line — otherwise a
# commit message, PR body, heredoc, or a build tool invocation like
# `npm run build` / `node build` gets falsely rejected. Matching is done per
# command segment (split on pipes/`&&`/`;`/...) so `find . | grep build/` is
# still caught while `git commit -m "...build/..."` is not.

# Read JSON input from stdin
INPUT=$(cat)

# Extract the command from JSON
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

# If no command found, allow it
if [ -z "$COMMAND" ]; then
  exit 0
fi

# Forbidden paths (generated/artifact directories and noisy file types).
FORBIDDEN_PATTERNS=(
  "frontend/\.svelte-kit"
  "backend/target"
  "build/"
  "dist/"
  "__pycache__"
  "\.git/objects/"
  "venv/"
  "\.pyc$"
  "\.csv$"
  "\.log$"
)

# Commands that read/search/list the filesystem. The deny-list only applies
# when a segment's leading command is one of these.
READ_CMDS='cat|bat|head|tail|less|more|nl|od|xxd|hexdump|strings|grep|egrep|fgrep|zgrep|rg|ag|ack|ls|ll|find|fd|tree|awk|gawk|sed|cut|sort|uniq|wc|stat|file|diff|colordiff|vim|vi|view|nano|emacs|readlink|realpath'

# Prefixes that wrap a real command; skip them to find the actual command word.
is_prefix() {
  case "$1" in
    sudo | command | time | nice | nohup | env | xargs | then | do | else | exec | builtin) return 0 ;;
    *=*) return 0 ;; # leading VAR=value environment assignment
    *) return 1 ;;
  esac
}

# Split the command into segments on shell separators so each invocation is
# inspected on its own. This is a heuristic (not a real shell parser) and is
# intentionally biased toward allowing rather than blocking.
SEGMENTS=$(printf '%s\n' "$COMMAND" | sed -E 's/(\|\||&&|\||;|&|`|\$\()/\n/g')

while IFS= read -r segment; do
  # Trim leading whitespace
  seg="${segment#"${segment%%[![:space:]]*}"}"
  [ -z "$seg" ] && continue

  # Find the leading command word, skipping env assignments and wrapper prefixes
  read -ra toks <<<"$seg"
  cmd=""
  for t in "${toks[@]}"; do
    if is_prefix "$t"; then
      continue
    fi
    cmd="$t"
    break
  done
  [ -z "$cmd" ] && continue
  cmd="${cmd##*/}" # strip any path, e.g. /bin/cat -> cat

  # Only enforce the deny-list for read/search/list commands
  if [[ "$cmd" =~ ^(${READ_CMDS})$ ]]; then
    for pattern in "${FORBIDDEN_PATTERNS[@]}"; do
      if echo "$seg" | grep -qE "$pattern"; then
        echo "ERROR: Reading '$pattern' is blocked by security policy (generated artifact)" >&2
        exit 2 # Exit code 2 = blocking error
      fi
    done
  fi
done <<<"$SEGMENTS"

# Command is clean, allow it
exit 0
