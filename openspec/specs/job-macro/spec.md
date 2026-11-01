# job-macro Specification

## Purpose
The `#[lucidity::job]` attribute turns a plain Rust function into a job: the function stays callable as written, and the
macro generates functions that run it in its own lunatic process, on this node or on another node in the cluster.

## Requirements
### Requirement: The annotated function stays callable directly
The macro SHALL emit the annotated function unchanged, so calling it by its own name runs it in the calling process,
like any other function.

#### Scenario: Direct call
- **WHEN** `#[lucidity::job] fn square(a: u32) -> u32 { a * a }` is called as `square(3)`
- **THEN** it returns `9`
- **AND** no process is spawned

### Requirement: Blocking local and remote functions
For an annotated function `name`, the macro SHALL generate `name_local` and `name_remote`, with the function's
visibility, arguments, and return type. Each SHALL run the function as a job in a new process and block the calling
process (not a thread) until the job finishes, then return its result. `name_local` SHALL run the job on the calling
node; `name_remote` SHALL run it on a node chosen as `node-placement` describes. Both SHALL panic when the job fails, as
`job-lifecycle` describes.

#### Scenario: Local blocking call
- **WHEN** `square_local(3)` is called
- **THEN** the calling process blocks until the job finishes
- **AND** the call returns `9`

#### Scenario: Remote blocking call
- **WHEN** `square_remote(5)` is called with at least one other node connected
- **THEN** the job runs on one of the other nodes
- **AND** the call returns `25`

### Requirement: Async local and remote functions return a Job handle
The macro SHALL generate `name_local_async` and `name_remote_async`, with the function's visibility and arguments,
returning `lucidity::Job<NameJob>` (see the generated job type) as soon as the job is spawned, without waiting for it to
finish. Node choice SHALL match `name_local` and `name_remote` respectively.

#### Scenario: Await an async job
- **WHEN** `square_local_async(4)` is called
- **THEN** it returns a `lucidity::Job<SquareJob>` immediately
- **AND** calling `await_get()` on it returns `16`

#### Scenario: Two jobs run concurrently
- **WHEN** a job calls `square_remote_async(a)` and `square_remote_async(b)` before awaiting either
- **THEN** both jobs run at the same time
- **AND** awaiting each returns its own square

### Requirement: Remote fanout function
The macro SHALL generate `name_remote_fanout(args_list: Vec<Args>) -> Vec<Output>`, with the function's visibility,
where `Args` is the packed argument type (see argument packing). It SHALL run one job per element across the other
nodes using the fanout strategy from the `fanout` attribute, block until all finish, and return the results in the
order of `args_list`, as `node-placement` describes.

#### Scenario: Fanout over argument tuples
- **WHEN** `pythagorean_remote_fanout(vec![(3, 4), (6, 8), (5, 12)])` is called with other nodes connected
- **THEN** it returns `vec![5.0, 10.0, 13.0]`

### Requirement: A job type is generated for each annotated function
The macro SHALL generate a unit struct named after the function in PascalCase with a `Job` suffix (`pythagorean` becomes
`PythagoreanJob`), with the function's visibility. This type parameterizes the `lucidity::Job` returned by the async
functions and can be passed to `lucidity::fanout` and `lucidity::Job::spawn` directly.

#### Scenario: Job type name
- **WHEN** `#[lucidity::job] pub fn pythagorean(num1: u32, num2: u32) -> f32` is expanded
- **THEN** a `pub struct PythagoreanJob` is generated
- **AND** `pythagorean_local_async` returns `lucidity::Job<PythagoreanJob>`

### Requirement: The job type implements JobFn
The generated job type SHALL implement `lucidity::JobFn` with `Args` the packed argument type, `Output` the return type
(`()` when there is none), `NAME` the function's name, `MAX_MEMORY` and `MAX_FUEL` from the attributes, and `call`
invoking the function.

#### Scenario: Two-argument function
- **WHEN** `#[lucidity::job] fn pythagorean(num1: u32, num2: u32) -> f32` is expanded
- **THEN** `struct PythagoreanJob` implements `lucidity::JobFn`
- **AND** its `Args` is `(u32, u32)` and its `Output` is `f32`
- **AND** its `NAME` is `"pythagorean"`

### Requirement: Arguments are packed into one value
The job's arguments SHALL travel to its process as one value: `()` for no arguments, the argument's own type for one
argument, and a tuple of the argument types in declaration order for two or more. The same type SHALL be the element
type of the fanout function's `Vec`.

#### Scenario: One argument
- **WHEN** `fn square(a: u32) -> u32` is annotated
- **THEN** `SquareJob::Args` is `u32`
- **AND** `square_remote_fanout` takes `Vec<u32>`

#### Scenario: No arguments and no return type
- **WHEN** `fn ping() {}` is annotated
- **THEN** `PingJob::Args` is `()`
- **AND** `PingJob::Output` is `()`

### Requirement: Arguments and results must be serializable
The packed argument type MUST implement `serde::Serialize` and `serde::DeserializeOwned`, and the return type MUST
implement those plus `Clone`, since both cross process (and node) boundaries. A function whose types do not SHALL fail
to compile.

#### Scenario: Return type without Clone
- **WHEN** an annotated function returns a type that implements serde's traits but not `Clone`
- **THEN** the crate fails to compile at the generated `JobFn` implementation

### Requirement: Only plain identifier parameters are accepted
Every parameter of the annotated function MUST be a plain identifier pattern (`a: u32`). A `self` receiver or a
destructuring pattern SHALL fail compilation with `Invalid argument pattern.`

#### Scenario: Destructured parameter
- **WHEN** `#[lucidity::job] fn sum((a, b): (u32, u32)) -> u32` is compiled
- **THEN** compilation fails with `Invalid argument pattern.`

### Requirement: Job attribute options
The attribute SHALL accept comma-separated `key = value` options, each optional:
`memory`, the job process's maximum memory in bytes as an unsuffixed integer literal, defaulting to `104857600`
(100 MiB); `fuel`, the job process's total fuel budget in units of about 100,000 WASM instructions as an unsuffixed
integer literal, defaulting to no limit; and `fanout`, the fanout strategy as the string literal `"roundrobin"` or
`"random"`, defaulting to `"roundrobin"`.

#### Scenario: Defaults
- **WHEN** a function is annotated with `#[lucidity::job]` and no options
- **THEN** its job type's `MAX_MEMORY` is `104857600`
- **AND** its `MAX_FUEL` is `None`
- **AND** its fanout function uses `lucidity::Fanout::RoundRobin`

#### Scenario: All options set
- **WHEN** a function is annotated with `#[lucidity::job(memory = 1024, fuel = 50, fanout = "random")]`
- **THEN** its job type's `MAX_MEMORY` is `1024`
- **AND** its `MAX_FUEL` is `Some(50)`
- **AND** its fanout function uses `lucidity::Fanout::Random`

### Requirement: Invalid options fail compilation
The macro SHALL reject, at compile time, with a message naming the offending token: an unknown option name
("Invalid attribute argument name"); a `memory` or `fuel` value that is not an unsuffixed integer ("Invalid attribute
argument value"); and a `fanout` value other than `"roundrobin"` or `"random"` ("Invalid fanout", listing the
two accepted values).

#### Scenario: Unknown fanout strategy
- **WHEN** a function is annotated with `#[lucidity::job(fanout = "sideways")]`
- **THEN** compilation fails with a message starting `Invalid fanout`

### Requirement: Removed polling options fail compilation
The macro SHALL reject the six polling options from 0.1 at compile time, rather than ignore them, with a message that
the option was removed because jobs no longer poll: `init_retry_interval_ms`, `sync_retry_interval_ms`,
`async_init_retry_interval_ms`, `async_get_retry_interval_ms`, `async_set_retry_interval_ms`, and
`shutdown_retry_interval_ms`.

#### Scenario: Removed polling option
- **WHEN** a function is annotated with `#[lucidity::job(sync_retry_interval_ms = 10)]`
- **THEN** compilation fails with a message starting `` `sync_retry_interval_ms` was removed``

### Requirement: A crate needs only the lucidity dependency
Generated code SHALL reach the runtime through paths under `lucidity::`, so the annotating crate MUST depend on the
`lucidity` crate under that name. The crate SHALL re-export the lunatic SDK (`lucidity::lunatic`, plus
`lucidity::abstract_process`), `lucidity::serde`, and `lucidity::rand`, so a job crate needs no direct dependency on
them.

#### Scenario: Using lunatic through lucidity
- **WHEN** a crate whose only runtime dependency is `lucidity` annotates a job that calls `lucidity::lunatic::sleep`
- **THEN** the crate compiles for `wasm32-wasip1`
