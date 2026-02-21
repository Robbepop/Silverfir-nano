#!/usr/bin/env python3
"""
Claude Code PreToolUse permission hook.
Single source of truth for all permissions — never falls through to allow/deny list.

Rules:
  1. DENY  all git invocations (even inside compound commands)
  2. ASK   for gh (GitHub CLI) invocations
  3. DENY  file operations outside workspace and /tmp
  4. ALLOW everything else
"""

import json
import os
import re
import sys

# ── Configuration ──────────────────────────────────────────────────────────────

# Derive workspace from script location: <workspace>/.claude/hooks/pre-tool-use.py
WORKSPACE = os.path.realpath(os.path.join(os.path.dirname(__file__), "..", ".."))

ALLOWED_PATH_PREFIXES = [
    WORKSPACE,
    "/tmp",
    "/private/tmp",                     # macOS: /tmp → /private/tmp
    os.path.expanduser("~/.claude"),    # Claude Code config/memory/skills
]

# Tools that take an explicit file path
FILE_PATH_TOOLS = {
    "Write":        "file_path",
    "Edit":         "file_path",
    "Read":         "file_path",
    "NotebookEdit": "notebook_path",
}

# Tools with an optional search path (defaults to cwd when absent)
SEARCH_PATH_TOOLS = {
    "Glob": "path",
    "Grep": "path",
}

# Command prefixes that appear before the real command
_CMD_PREFIXES = frozenset({
    "sudo", "env", "time", "timeout", "nice", "nohup",
    "command", "builtin", "exec", "caffeinate",
})

# ── Helpers ────────────────────────────────────────────────────────────────────

def emit(decision, reason=""):
    json.dump({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": decision,
            "permissionDecisionReason": reason,
        }
    }, sys.stdout)
    sys.exit(0)


def allow(reason="approved"):
    emit("allow", reason)


def deny(reason="blocked"):
    emit("deny", reason)


def is_path_allowed(path):
    """Check if a resolved path falls under any allowed prefix."""
    if not path:
        return True
    try:
        resolved = os.path.realpath(os.path.expanduser(path))
    except (ValueError, OSError):
        return False
    for prefix in ALLOWED_PATH_PREFIXES:
        if resolved == prefix or resolved.startswith(prefix + "/"):
            return True
    return False


def find_blocked_command(command):
    """Check if any sub-command in a (possibly compound) shell string is blocked.

    Returns:
        ("deny",  reason) — for hard-blocked commands (git)
        ("ask",   reason) — for commands needing user approval (gh)
        None              — command is clean
    """
    # Commands that are always denied vs require user approval
    DENY_CMDS = {"git"}
    ASK_CMDS  = {"gh"}

    # Command substitutions: $(cmd ...) or `cmd ...`
    for name in DENY_CMDS:
        if re.search(rf'\$\([^)]*\b{name}\b', command):
            return ("deny", f"{name} blocked: {command[:80]}")
        if re.search(rf'`[^`]*\b{name}\b', command):
            return ("deny", f"{name} blocked: {command[:80]}")
    for name in ASK_CMDS:
        if re.search(rf'\$\([^)]*\b{name}\b', command):
            return ("ask", f"{name} requires approval: {command[:80]}")
        if re.search(rf'`[^`]*\b{name}\b', command):
            return ("ask", f"{name} requires approval: {command[:80]}")

    # Split on shell operators: ;  &&  ||  |  (  )
    for part in re.split(r'[;&|()]+', command):
        tokens = part.split()
        for tok in tokens:
            # Skip env-var assignments like FOO=bar
            if '=' in tok and not tok.startswith('-'):
                continue
            # Skip known command prefixes
            if tok in _CMD_PREFIXES:
                continue
            # First real token is the command name
            base = tok.rsplit("/", 1)[-1]  # handle /usr/bin/git etc.
            if base in DENY_CMDS:
                return ("deny", f"{base} blocked: {command[:80]}")
            if base in ASK_CMDS:
                return ("ask", f"{base} requires approval: {command[:80]}")
            break  # not blocked → this sub-command is fine

    return None


# ── Main ───────────────────────────────────────────────────────────────────────

def main():
    try:
        data = json.load(sys.stdin)
    except Exception:
        emit("ask", "hook: failed to parse input")
        return

    tool = data.get("tool_name", "")
    inp  = data.get("tool_input", {})

    # ── Rule 1+2: blocked/ask commands in Bash ─────────────────────────────
    if tool == "Bash":
        cmd = inp.get("command", "")
        result = find_blocked_command(cmd)
        if result:
            emit(result[0], result[1])
        allow()

    # ── Rule 3: file tools — path must be in workspace or /tmp ─────────────
    if tool in FILE_PATH_TOOLS:
        key  = FILE_PATH_TOOLS[tool]
        path = inp.get(key, "")
        if not is_path_allowed(path):
            deny(f"path outside workspace: {path}")
        allow()

    if tool in SEARCH_PATH_TOOLS:
        key  = SEARCH_PATH_TOOLS[tool]
        path = inp.get(key)  # often None → defaults to cwd
        if path and not is_path_allowed(path):
            deny(f"path outside workspace: {path}")
        allow()

    # ── Rule 4: everything else → allow ────────────────────────────────────
    allow(f"{tool} approved")


if __name__ == "__main__":
    main()
