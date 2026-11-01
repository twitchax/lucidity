# fly-provisioning Specification

## Purpose
The optional `fly` feature lets an app running on fly.io provision its own lunatic worker nodes as fly Machines,
each registering with a control node on the calling machine, so remote jobs have peers to run on.

## Requirements
### Requirement: Feature-gated module
The `lucidity::fly` module SHALL exist only when the `fly` cargo feature is enabled. A default build SHALL NOT include
it or its HTTP and JSON dependencies.

#### Scenario: Default build
- **WHEN** a crate depends on `lucidity` without the `fly` feature
- **THEN** `lucidity::fly` does not resolve

#### Scenario: Feature enabled
- **WHEN** a crate depends on `lucidity` with `features = ["fly"]`
- **THEN** `lucidity::fly::ensure_machines` and `lucidity::fly::list_machines` are available

### Requirement: Machine provisioning reads the fly environment
`lucidity::fly::ensure_machines(key, count)` SHALL read the app name, the calling machine's id, and the region from
`FLY_APP_NAME`, `FLY_MACHINE_ID`, and `FLY_REGION`, and SHALL panic with `<VAR> not set.` when any of them is missing.
`key` is the fly API token, sent as a bearer token on every request.

#### Scenario: Missing region
- **WHEN** `ensure_machines` is called with `FLY_REGION` unset
- **THEN** the calling process panics with `FLY_REGION not set.`

### Requirement: Worker machines are recreated, not reused
`ensure_machines(key, count)` SHALL provision `count` machines named `lucid-1` through `lucid-<count>` in the calling
app and region, all concurrently. For each name it SHALL stop and delete any existing machine with that name (ignoring
failures), wait 30 seconds, create a new machine, and wait another 30 seconds. It SHALL return only after every machine
has been handled, so a successful call takes at least 60 seconds.

#### Scenario: Re-running replaces the workers
- **WHEN** `ensure_machines(key, 2)` is called in an app that already has machines `lucid-1` and `lucid-2`
- **THEN** both existing machines are stopped and deleted
- **AND** two new machines named `lucid-1` and `lucid-2` are created in the caller's region
- **AND** the call returns `Ok(())` after at least 60 seconds

### Requirement: Worker machine configuration
Each created machine SHALL use the image `twitchax/lunatic:2024.01.10` with a shared CPU, 1 vCPU, 1024 MB of memory, an
`always` restart policy, `auto_destroy` set, and a UDP service on internal port 3031. Its init command SHALL be
`/app.entrypoint.sh http://<FLY_MACHINE_ID>.vm.<FLY_APP_NAME>.internal:3030/`, so the calling machine MUST run a lunatic
control node on port 3030.

#### Scenario: Created machine settings
- **WHEN** `ensure_machines` creates `lucid-1` from machine `abc` of app `my-app`
- **THEN** the machine runs `twitchax/lunatic:2024.01.10` on one shared vCPU with 1024 MB
- **AND** its init command is `/app.entrypoint.sh http://abc.vm.my-app.internal:3030/`

### Requirement: Worker image entrypoint
The repository's worker image entrypoint (`docker/app.entrypoint.sh`) SHALL start `lunatic node` bound to
`[$FLY_PRIVATE_IP]:3031` and registered with the control URL passed as its first argument.

#### Scenario: Worker joins the caller's cluster
- **WHEN** `ensure_machines` creates a worker from an app whose machine runs `lunatic control --bind-socket [::]:3030`
- **THEN** the worker starts a lunatic node on its private IP, port 3031
- **AND** the node registers with the control node on the calling machine

### Requirement: Provisioning errors
`ensure_machines` SHALL return `Err(String)` for the first machine, in name order, whose create request could not be
sent, with the message prefixed by the failing steps (`[ensure_machine] Failed to create machine.` followed by
`[create_machine] Failed to send request.` and the transport error).

#### Scenario: Network failure
- **WHEN** the Machines API is unreachable while `ensure_machines(key, 1)` creates `lucid-1`
- **THEN** it returns an `Err` whose message starts with `[ensure_machine] Failed to create machine.`

### Requirement: Listing machines
`lucidity::fly::list_machines(key, app_name)` SHALL request `GET https://api.machines.dev/v1/apps/<app_name>/machines`
with `key` as the bearer token and return the response body parsed as a JSON array of machine objects. It SHALL return
`Err` prefixed `[list_machines] Failed to send request.` when the request cannot be sent, and
`[list_machines] Failed to parse response.` when the body is not a JSON array.

#### Scenario: List an app's machines
- **WHEN** `list_machines(key, "my-app")` is called for an app with two machines
- **THEN** it returns `Ok` with two JSON values, each carrying the machine's `id` and `name`
