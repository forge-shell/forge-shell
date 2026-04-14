# RFC-006 — Job Control Model (Cross-Platform)

| Field          | Value                        |
|----------------|------------------------------|
| Status         | Draft                        |
| Author         | Ajitem Sahasrabuddhe         |
| Created        | 2026-04-09                   |
| Last Updated   | 2026-04-09                   |
| Supersedes     | —                            |
| Superseded By  | —                            |

---

## Summary

This RFC defines the Forge Shell job control model — how foreground and
background processes, process groups, and job suspension are managed across
Linux, macOS, and Windows. Job control is the most deeply Unix-specific
feature of any shell. This RFC establishes what Forge Shell can guarantee
cross-platform and what is documented as a Windows limitation.

---

## Motivation

Job control — the ability to run commands in the background (`cmd &`), suspend
a foreground job (`Ctrl+Z`), and bring it back (`fg`) — is fundamental to
interactive shell use. However, it is built on Unix primitives that have no
direct Windows equivalents:

| Unix Primitive | Windows Equivalent | Parity |
|---|---|---|
| Process groups (`setpgid`) | Job Objects | Partial |
| `SIGSTOP` / `SIGCONT` | No equivalent | ❌ None |
| `SIGCHLD` | No equivalent | ❌ None |
| `waitpid(-1)` | `WaitForMultipleObjects` | Partial |
| Terminal ownership (`tcsetpgrp`) | Console ownership | Partial |
| `Ctrl+Z` → `SIGTSTP` | No equivalent | ❌ None |

This RFC defines the Forge job model as an abstraction over these primitives,
with clear documentation of Windows limitations.

---

## Design

### 1. The Forge Job Model

A `Job` in Forge Shell is an abstraction over one or more OS processes that
were started together. Jobs are identified by a sequential `JobId` (1, 2, 3…).

```
Job {
  id:       JobId         -- sequential, shell-scoped
  status:   JobStatus     -- Running | Stopped | Done(i32) | Failed(i32)
  command:  string        -- the command string as typed
  pids:     [Pid]         -- one or more PIDs (pipelines have multiple)
  handle:   PlatformHandle -- Unix: pgid, Windows: Job Object handle
}

enum JobStatus {
  Running
  Stopped          -- suspended, can be resumed (Unix only)
  Done(exit_code)
  Failed(exit_code)
}
```

---

### 2. Background Execution

Background execution (`cmd &`) is supported on all three platforms.

```forge
# Run in background
run("long-task") &

# Background with explicit job reference
let job = spawn("server --port 8080")
print("Server started as job {job.id}")

# Wait for a specific job
job.wait()?

# Wait for all background jobs
wait_all()?
```

**Unix:** Spawned process is placed in its own process group. Shell regains
terminal control immediately.

**Windows:** Spawned via `CreateProcess` with `CREATE_NEW_PROCESS_GROUP` flag.
Shell continues immediately. Job Object tracks the process.

---

### 3. Job Listing

```bash
# List all jobs
jobs

# Output (all platforms)
[1]  Running    server --port 8080
[2]  Done(0)    npm run build
[3]  Failed(1)  deploy.fgs --env staging
```

---

### 4. Foreground / Background Control

#### `fg` — Bring job to foreground

**Unix:** Send `SIGCONT` to the process group, transfer terminal ownership
via `tcsetpgrp`. Shell waits for job completion.

**Windows:** Attach to the process's console. `AttachConsole(pid)`. Shell
waits via `WaitForSingleObject`.

```bash
fg        # bring most recent background job to foreground
fg %2     # bring job 2 to foreground
fg %server  # bring job matching "server" to foreground
```

#### `bg` — Resume a stopped job in the background

**Unix:** Send `SIGCONT` to the process group. Job continues in background.

**Windows:** Not applicable — jobs cannot be stopped on Windows. `bg`
with no stopped jobs prints a clear message.

```bash
bg        # resume most recent stopped job in background
bg %2     # resume job 2 in background
```

#### `Ctrl+Z` — Suspend foreground job

**Unix:** Terminal sends `SIGTSTP` to the foreground process group. Shell
catches `SIGCHLD`, updates job status to `Stopped`, returns control to REPL.

**Windows:** ❌ Not supported. `Ctrl+Z` in a Windows console sends EOF to
stdin (not a suspend signal). Forge Shell intercepts this and displays:

```
Note: Job suspension (Ctrl+Z) is not supported on Windows.
      Use Ctrl+C to terminate, or run commands with & to background them.
```

---

### 5. Signal Sending

```bash
# Send signal to job (Unix)
kill %1           # SIGTERM to job 1
kill -9 %1        # SIGKILL to job 1
kill -SIGINT %1   # SIGINT to job 1

# Windows — kill terminates the process (no signal concept)
kill %1           # TerminateProcess(handle, 1)
kill -9 %1        # same — all kills are equivalent on Windows
```

---

### 6. Pipeline Jobs

A pipeline (`cmd1 | cmd2 | cmd3`) is a single job containing multiple
processes.

**Unix:** All processes in the pipeline are placed in the same process group.
The shell sends signals to the group, not individual processes. `waitpid`
collects exit codes from each process.

**Windows:** Each process in the pipeline is added to the same Job Object.
`WaitForMultipleObjects` collects completions. The exit code of the last
process is the pipeline exit code.

```forge
# This is one job with three processes
let result = run("git log --oneline") | run("grep feat") | run("wc -l")
```

---

### 7. The `PlatformJobControl` Trait

```rust
pub trait PlatformJobControl: Send + Sync {
    fn spawn_background(&self, cmd: &SpawnProcess) -> Result<Job>;
    fn spawn_foreground(&self, cmd: &SpawnProcess) -> Result<ExitStatus>;
    fn bring_to_foreground(&self, job: &Job) -> Result<ExitStatus>;
    fn send_to_background(&self, job: &Job) -> Result<()>;
    fn suspend(&self, job: &Job) -> Result<()>;     // Unix: SIGSTOP, Windows: Err
    fn resume(&self, job: &Job) -> Result<()>;      // Unix: SIGCONT, Windows: Err
    fn terminate(&self, job: &Job) -> Result<()>;
    fn kill(&self, job: &Job) -> Result<()>;
    fn wait(&self, job: &Job) -> Result<ExitStatus>;
    fn wait_any(&self) -> Result<(Job, ExitStatus)>;
    fn list_jobs(&self) -> Vec<Job>;
}
```

---

### 8. Cross-Platform Guarantee Summary

| Feature | Linux | macOS | Windows |
|---|---|---|---|
| Background execution (`&`) | ✅ | ✅ | ✅ |
| Job listing (`jobs`) | ✅ | ✅ | ✅ |
| Bring to foreground (`fg`) | ✅ | ✅ | ✅ (limited) |
| Suspend (`Ctrl+Z`) | ✅ | ✅ | ❌ documented |
| Resume in background (`bg`) | ✅ | ✅ | ❌ documented |
| Signal sending (`kill -SIGNAL`) | ✅ | ✅ | ⚠️ terminate only |
| Pipeline job grouping | ✅ | ✅ | ✅ (Job Objects) |
| Exit code collection | ✅ | ✅ | ✅ |

---

## Drawbacks

- **Windows job control is fundamentally limited** — this RFC documents the
  limitation rather than resolving it. There is no complete resolution
  without OS-level changes.
- **Process group management is complex** — getting `tcsetpgrp` and
  `SIGCHLD` handling right on Unix is notoriously tricky. Signal races
  are a known hazard.
- **Job state synchronisation** — tracking job state across signal delivery
  and process exit requires careful synchronisation.

---

## Alternatives Considered

### Alternative A — No Job Control on Windows

**Approach:** `&`, `fg`, `bg`, `jobs` are simply not available on Windows.
**Rejected because:** Background execution (`cmd &`) is a fundamental shell
feature. Removing it entirely on Windows would make Forge Shell unusable for
many workflows. Background execution is achievable on Windows — only
suspension is not.

### Alternative B — WSL for Windows Job Control

**Approach:** On Windows, route job control through WSL.
**Rejected because:** Forge Shell is designed to run natively on Windows
without WSL. Requiring WSL contradicts the cross-platform goal.

---

## Unresolved Questions

- [ ] Should `Ctrl+Z` on Windows offer to terminate instead of suspend?
      (i.e. "Suspend not supported — terminate instead? [y/N]")
- [ ] How should pipeline exit codes be reported when individual stages fail?
      Last process? First failure? Configurable?
- [ ] Should there be a `forge set pipefail` option (like bash `set -o pipefail`)?
- [ ] How are orphaned background jobs handled when the shell exits?

---

## Implementation Plan

### Affected Crates

- `forge-core` — `Job` type, `JobTable` (tracks all active jobs)
- `forge-lower` — `PlatformJobControl` trait implementations
- `forge-builtins` — `jobs`, `fg`, `bg`, `kill` built-in commands
- `forge-repl` — `Ctrl+Z` handling, `SIGCHLD` handler, job notifications

### Dependencies

- Requires RFC-002 (Evaluation Pipeline) — job spawning integrates with
  the execution engine
- Requires RFC-003 (Built-in Commands) — `jobs`, `fg`, `bg`, `kill` are built-ins

### Milestones

1. Define `Job`, `JobId`, `JobStatus` types in `forge-core`
2. Implement `PlatformJobControl` for Unix (process groups, signals)
3. Implement `PlatformJobControl` for Windows (Job Objects)
4. Implement `SIGCHLD` handler and job state updates on Unix
5. Implement `jobs`, `fg`, `bg`, `kill` built-ins
6. Implement `Ctrl+Z` handling in REPL (Unix suspend, Windows message)
7. Integration tests — background execution and job lifecycle

---

## References

- [Advanced Programming in the UNIX Environment — Job Control](https://apuebook.com)
- [Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
- [Bash Job Control](https://www.gnu.org/software/bash/manual/bash.html#Job-Control)
- [Fish Shell Job Control](https://fishshell.com/docs/current/language.html#job-control)
