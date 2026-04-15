# RFC-006 — Job Control Model

| Field          | Value                        |
|----------------|------------------------------|
| Status         | **In Review**                |
| Author         | Ajitem Sahasrabuddhe         |
| Created        | 2026-04-09                   |
| Last Updated   | 2026-04-15                   |
| Supersedes     | —                            |
| Superseded By  | —                            |

---

## Summary

This RFC defines the Forge Shell job control model — background execution,
foreground/background switching, job suspension, pipeline exit code semantics,
and shell exit behaviour. Job control is cross-platform where possible and
explicitly documented where platform differences are unavoidable. Windows
cannot suspend processes — this is a documented limitation, not a bug.

---

## Motivation

Job control is a fundamental shell capability. Developers run long-running
servers, build processes, and watchers in the background while continuing
to work in the foreground. bash and zsh implement job control via Unix
signals — a model that is powerful but fragile, poorly understood, and
fundamentally incompatible with Windows.

Forge Shell takes a pragmatic approach:
- Background execution works on all three platforms
- Suspension (`Ctrl+Z`) works on Unix — Windows offers a safe alternative
- Pipeline failures are structured and visible — not silently swallowed
- Shell exit with background jobs is always explicit — no orphaned processes

---

## Design

### 1. The Job Model

A `Job` is an abstraction over one or more OS processes started together.
Jobs are identified by a sequential `JobId` (1, 2, 3…).

```rust
pub struct Job {
    pub id:       JobId,
    pub status:   JobStatus,
    pub command:  String,         // command string as typed
    pub pids:     Vec<Pid>,       // pipelines have multiple PIDs
    pub handle:   PlatformHandle, // Unix: pgid, Windows: Job Object handle
}

pub enum JobStatus {
    Running,
    Stopped,              // suspended — Unix only
    Done(i32),            // exit code
    Failed(i32),          // non-zero exit code
}
```

---

### 2. Background Execution

Background execution is supported on all three platforms.

```forge
# Background with & suffix
run("server --port 8080") &

# Background via spawn — returns a handle
let job = spawn { run("server --port 8080") }
echo "Server started as job {job.id}"

# Wait for a specific job
job.await()?

# Wait for all background jobs
wait_all()?
```

**Unix:** Process placed in its own process group. Shell regains terminal
control immediately.

**Windows:** Process spawned with `CREATE_NEW_PROCESS_GROUP`. Shell continues
immediately. Windows Job Object tracks the process for resource management.

---

### 3. Job Listing — `jobs`

```bash
jobs
```

```
[1]  Running    server --port 8080
[2]  Done(0)    npm run build
[3]  Failed(1)  deploy.fgs --env staging
```

Output format is identical on all three platforms.

---

### 4. Foreground Control — `fg`

Bring a background or stopped job to the foreground.

```bash
fg          # bring most recent background job to foreground
fg %2       # bring job 2 to foreground
fg %server  # bring job matching "server" to foreground
```

**Unix:** Send `SIGCONT` to the process group, transfer terminal ownership
via `tcsetpgrp`. Shell waits for job completion.

**Windows:** Attach to the process console via `AttachConsole(pid)`. Shell
waits via `WaitForSingleObject`.

---

### 5. Background Resume — `bg`

Resume a stopped job in the background.

```bash
bg          # resume most recent stopped job in background
bg %2       # resume job 2 in background
```

**Unix:** Send `SIGCONT` to the process group. Job continues in background.

**Windows:** `bg` is not applicable — jobs cannot be stopped on Windows.
If called with no stopped jobs, prints a clear informational message:

```
No stopped jobs. Use & to start a job in the background.
```

---

### 6. Job Suspension — `Ctrl+Z`

**Unix:** Terminal sends `SIGTSTP` to the foreground process group. Shell
catches `SIGCHLD`, updates job status to `Stopped`, returns control to REPL.

```
[1]+  Stopped    server --port 8080
```

**Windows:** `Ctrl+Z` in a Windows console sends EOF to stdin — not a suspend
signal. Forge Shell intercepts this and offers the closest safe alternative:

```
Job suspension is not supported on Windows.
Move "server --port 8080" to background instead? [y/N]
```

- User confirms `y` → process moved to background
- User declines `n` → process continues in foreground, prompt clears
- Process is never terminated without explicit `Ctrl+C` or `kill`

---

### 7. Pipeline Exit Code Semantics

#### 7.1 Default — Fail Fast

Pipelines halt at the first stage failure. Downstream stages never run on
bad or incomplete data.

```forge
ls /home/user | grep "forge" | sort

# If grep fails:
# - sort never runs
# - pipeline returns Err(PipelineError)
```

**Error type:**

```rust
pub struct PipelineError {
    pub failed_stage: usize,        // 0-based index of failed stage
    pub command:      String,       // "grep forge"
    pub exit_code:    i32,
    pub stderr:       String,       // captured stderr
    pub completed:    Vec<String>,  // stages that succeeded before failure
}
```

**ForgeScript error handling:**

```forge
match ls /home/user | grep "forge" | sort {
    Ok(output) => echo "{output}"
    Err(e)     => {
        echo "Pipeline failed at: {e.command}"
        echo "Exit code: {e.exit_code}"
        echo "Completed: {e.completed}"
    }
}
```

**Terminal error rendering:**

```
error: pipeline failed
  stage 1 ✅  ls /home/user
  stage 2 ❌  grep "forge" — exit code 1 (no matches found)
  stage 3 ⏭️  sort — not run
```

#### 7.2 Opt-in Continuation — `|?` Operator

The `|?` operator passes output to the next stage even if the current stage
fails. Applied per pipe boundary — surgical, not global.

```forge
# grep failure continues to sort — sort runs on empty input
ls /home/user |? grep "forge" | sort
```

```forge
# Precise per-boundary control
ls /home/user |? grep "forge" | sort
#              ↑                ↑
#           continue         fail-fast
# grep fail → sort runs     sort fail → pipeline stops
```

**Result with `|?`:**

```forge
# If grep fails — sort runs on empty input
# result is Ok with warnings
# OutputMetadata.warnings = ["grep: exit code 1 — no matches, continuing"]
```

**Terminal warning rendering:**

```
warning: pipeline stage failed — continuing
  stage 1 ✅  ls /home/user
  stage 2 ⚠️  grep "forge" — exit code 1 (no matches, continuing)
  stage 3 ✅  sort
```

#### 7.3 Pipeline Operator Summary

| Operator | Behaviour |
|---|---|
| `\|` | Fail-fast — halt if this stage fails |
| `\|?` | Continue — pass output to next stage even on failure |

---

### 8. Shell Exit with Background Jobs

When the user exits Forge Shell with background jobs running, Forge Shell
always makes the exit explicit — no silent orphaning of processes.

**Orphaned processes are not allowed.** Processes that outlive their parent
shell without explicit management belong in a process manager (`systemd`,
`launchd`, `pm2`, Windows Services) — not in a background shell job.

#### 8.1 Prompt Behaviour (default)

```
You have 2 running background jobs:
  [1] Running    server --port 8080
  [2] Running    npm run watch

  [k] Kill all jobs and exit
  [s] Stay in shell
```

No third option. The user either cleans up and exits or stays and manages
their jobs before exiting.

#### 8.2 `config.toml` Configuration

```toml
[jobs]
on_exit      = "prompt"   # "prompt" | "kill"
warn_on_exit = true       # show job list even in kill mode
```

| Value | Behaviour |
|---|---|
| `"prompt"` | Ask — kill all or stay in shell (default) |
| `"kill"` | Always terminate all background jobs on exit |

**`"kill"` with `warn_on_exit = true`:**

```
Warning: terminating 2 background jobs on exit
  [1] server --port 8080
  [2] npm run watch
```

**`"kill"` with `warn_on_exit = false`:**

```
# Silent exit — jobs terminated, no output
```

#### 8.3 Platform Consistency

Both options behave identically on Linux, macOS, and Windows:

| Option | Linux/macOS | Windows |
|---|---|---|
| `"kill"` | `SIGTERM` → `SIGKILL` | `TerminateProcess` |
| `"prompt"` → kill | Same as above | Same as above |
| `"prompt"` → stay | Return to shell | Return to shell |

No orphaned processes. No Job Object edge cases. No platform-specific
behaviour divergence.

---

### 9. Platform Limitation Summary

| Feature | Linux | macOS | Windows |
|---|---|---|---|
| Background execution (`&`) | ✅ | ✅ | ✅ |
| `jobs` listing | ✅ | ✅ | ✅ |
| `fg` — foreground | ✅ | ✅ | ✅ |
| `bg` — background resume | ✅ | ✅ | ⚠️ No stopped jobs |
| `Ctrl+Z` — suspend | ✅ | ✅ | ⚠️ Offer background instead |
| `kill` — send signal | ✅ | ✅ | ⚠️ SIGTERM mapped to TerminateProcess |
| Pipeline fail-fast | ✅ | ✅ | ✅ |
| `\|?` continuation | ✅ | ✅ | ✅ |

---

## Drawbacks

- **`Ctrl+Z` on Windows requires UX explanation.** Developers with Unix
  muscle memory will press `Ctrl+Z` and get a prompt instead of suspension.
  The prompt must be clear and non-disruptive.
- **Fail-fast pipelines break bash muscle memory.** Developers accustomed
  to bash's silent failure model may be surprised that `grep | sort` halts
  on grep failure. The error output is designed to be immediately actionable.
- **No process survival across shell exit.** Developers who want long-running
  processes to survive must use a process manager. This is a deliberate
  constraint — not a missing feature.

---

## Alternatives Considered

### Alternative A — Silent pipeline failure (bash default)

**Rejected:** Last-process-wins exit codes silently swallow failures.
`false | true` exits 0 in bash. This is the class of bug ForgeScript
exists to prevent.

### Alternative B — Global `pipefail` setting

**Rejected:** Not needed — fail-fast is already the default. The `|?`
operator provides per-boundary opt-in continuation without a global flag.

### Alternative C — Allow orphaned processes (`"leave"` / `"disown"`)

**Rejected:** Orphaned process behaviour differs significantly between
Unix (re-parented to PID 1) and Windows (Job Object termination risk).
The platform inconsistency is unacceptable. Long-running processes belong
in a process manager, not a background shell job.

### Alternative D — `Ctrl+Z` on Windows offers termination

**Rejected:** The user pressed `Ctrl+Z` specifically because they do not
want to terminate. Offering termination is the wrong semantic. Backgrounding
is the closest safe equivalent.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | `Ctrl+Z` on Windows | Offer to background — `[y/N]` prompt |
| UQ-2 | Pipeline exit codes | Fail-fast — halt at first failure, structured `PipelineError` |
| UQ-3 | `pipefail` option | Not needed — `\|?` operator for per-boundary opt-in |
| UQ-4 | Orphaned background jobs | Two options only — `"prompt"` or `"kill"`, no orphaning |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-core` | `Job`, `JobId`, `JobStatus`, `JobTable`, `PipelineError` |
| `forge-backend/unix` | Process group management, signal handling, `SIGCHLD` |
| `forge-backend/windows` | Job Object management, `WaitForSingleObject`, `TerminateProcess` |
| `forge-engine` | Pipeline execution, fail-fast, `\|?` operator |
| `forge-repl` | `Ctrl+Z` interception, job notifications, exit prompt |
| `forge-cli/builtins` | `jobs`, `fg`, `bg`, `kill` built-in implementations |

### Dependencies

- Requires RFC-001 (ForgeScript syntax) — `&` operator, `spawn { }`, `join! { }`
- Requires RFC-002 (evaluation pipeline) — job spawning via execution engine
- Requires RFC-003 (built-in commands) — `jobs`, `fg`, `bg`, `kill` as built-ins
- Requires RFC-013 (shell config) — `[jobs]` config section

### Milestones

1. Define `Job`, `JobId`, `JobStatus`, `JobTable`, `PipelineError` in `forge-core`
2. Implement Unix backend — process groups, `SIGCHLD`, `tcsetpgrp`
3. Implement Windows backend — Job Objects, `WaitForSingleObject`
4. Implement `|` fail-fast pipeline execution in `forge-engine`
5. Implement `|?` continuation operator in `forge-engine`
6. Implement `jobs`, `fg`, `bg`, `kill` built-ins
7. Implement `Ctrl+Z` interception — Unix suspend, Windows background prompt
8. Implement shell exit prompt — `[k]` kill / `[s]` stay
9. Implement `[jobs]` config section in RFC-013
10. Integration tests on ubuntu-latest, macos-latest, windows-latest

---

## References

- [Advanced Programming in the UNIX Environment — Job Control](https://apuebook.com)
- [Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
- [Bash Job Control](https://www.gnu.org/software/bash/manual/bash.html#Job-Control)
- [Fish Shell Job Control](https://fishshell.com/docs/current/language.html#job-control)
- [POSIX Signal Handling](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/signal.h.html)
- [RFC-001 — ForgeScript Syntax](./RFC-001-forgescript-syntax.md)
- [RFC-002 — Evaluation Pipeline](./RFC-002-evaluation-pipeline.md)
- [RFC-003 — Built-in Commands](./RFC-003-builtin-commands.md)
- [RFC-013 — Shell Configuration Model](./RFC-013-shell-config.md)