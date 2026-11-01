# job-lifecycle Specification

## Purpose
How a spawned job runs and ends, through `lucidity::Job<J>`: delivering its result, failing loudly when its process
dies or its node goes down, honoring its resource limits, and being cancelled when its handle is dropped.

## Requirements
### Requirement: Spawning a job on a node
`lucidity::Job::<J>::spawn(node, args)` SHALL start the `JobFn` `J` with `args` in a new process on `node`, which may be
the calling node, and return a `Job<J>` without waiting for the job to finish. When `node` is not part of the cluster,
the calling process SHALL panic.

#### Scenario: Spawn on the calling node
- **WHEN** a process calls `Job::<SquareJob>::spawn(lucidity::local_node(), 7)`
- **THEN** it gets a `Job<SquareJob>` back before the job finishes
- **AND** `await_get()` on it returns `49`

### Requirement: Checking a job without blocking
`Job::try_get` SHALL never block: it SHALL return `Some(result)` once the job has finished and `None` while it is
still running. Once a result has arrived, every later `try_get` or `await_get` on the same `Job` SHALL return that
same result.

#### Scenario: Job still running
- **WHEN** `try_get()` is called on a job that has not finished
- **THEN** it returns `None` immediately

#### Scenario: Job finished
- **WHEN** `try_get()` is called on a job that has returned `16`
- **THEN** it returns `Some(16)`
- **AND** calling `try_get()` or `await_get()` again returns `16` again

### Requirement: Awaiting a job without timeouts or polling
`Job::await_get` SHALL block the calling process until the job's outcome arrives, then return its result. Neither
`await_get` nor `try_get` SHALL time out, retry, or poll the job's process, so a slow job only delays its caller and
never fails it.

#### Scenario: Slow job
- **WHEN** `await_get()` is called on a job that takes several minutes to return `0`
- **THEN** the caller stays blocked until the job returns
- **AND** `await_get()` returns `0`

### Requirement: Each job's outcome reaches only its own handle
Each `Job` SHALL receive its own outcome, whatever order its caller's jobs finish or are awaited in.

#### Scenario: Awaiting out of completion order
- **WHEN** a process spawns jobs A and B, B finishes first, and the process awaits A and then B
- **THEN** A's `await_get()` returns A's result
- **AND** B's `await_get()` returns B's result

### Requirement: A job that dies panics its caller
`try_get` and `await_get` SHALL panic when the job's process dies before returning (it panics, traps, or runs out of
memory or fuel), with ``lucidity: job `<name>` on node <id> died before returning a result``, where `<name>` is
the job function's name and `<id>` the node it ran on. The caller SHALL NOT hang.

#### Scenario: Job runs out of fuel
- **WHEN** a process calls `spin_local()` for `#[lucidity::job(fuel = 1)] fn spin() -> u64` that loops past its fuel
- **THEN** the calling process panics with a message containing ``job `spin` on node 0 died before returning a result``
- **AND** `spin_local()` never returns

### Requirement: A job whose node goes down panics its caller
When the node running a job goes down before the job returns, `try_get` and `await_get` on its `Job` SHALL panic with
``lucidity: job `<name>` on node <id> was lost: its node went down before it returned a result``. The panic SHALL come
once the cluster's control node drops the silent node, without the caller polling for it.

#### Scenario: Peer killed mid-job
- **WHEN** a process calls `linger_remote()` for a job that sleeps for ten minutes, and the peer node running it is killed
- **THEN** the calling process panics with a message containing ``job `linger` on node`` and `was lost: its node went down`
- **AND** `linger_remote()` never returns

### Requirement: Dropping an unfinished job cancels it
Dropping a `Job` whose result has not yet been received SHALL kill the job's process. Dropping a `Job` SHALL NOT panic,
whether the job finished, failed, or is still running.

#### Scenario: Drop before the result arrives
- **WHEN** a process drops the `Job` returned by `linger_remote_async()` while the job is still sleeping
- **THEN** the job's process on the remote node is killed
- **AND** the dropping process continues normally

### Requirement: Job resource limits
Each job's process SHALL run with a maximum memory of `J::MAX_MEMORY` bytes and, when `J::MAX_FUEL` is `Some(n)`, a
total budget of `n` fuel units (about 100,000 WASM instructions each). A job that exceeds either limit SHALL die, which
its caller sees as a dead job.

#### Scenario: Fuel limit reached
- **WHEN** a job declared with `fuel = 1` runs more than about 100,000 instructions
- **THEN** its process dies
- **AND** its caller panics with the died-before-returning message

### Requirement: Jobs can spawn jobs
A job's process SHALL be allowed to spawn processes, create process configs, and compile modules, so a job MAY call any
generated job function, local or remote, including from a job already running on another node.

#### Scenario: Remote jobs spawning remote jobs
- **WHEN** `pythagorean_spread_remote_fanout(vec![(8, 15), (7, 24), (20, 21)])` runs, where each `pythagorean_spread`
  job awaits `square_remote_async(a)` and `square_remote_async(b)`
- **THEN** it returns `vec![17.0, 25.0, 29.0]`
