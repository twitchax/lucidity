# node-placement Specification

## Purpose
Where jobs run: the calling node for local jobs, the other nodes of the lunatic cluster for remote jobs, and how a
fanout spreads one job per argument set across those nodes. Also the node queries (`local_node`, `remote_nodes`,
`random_node`) that expose the same choices to callers.

## Requirements
### Requirement: The calling node's id
`lucidity::local_node()` SHALL return the id of the node the calling process runs on. Under a plain `lunatic run`,
with no control node, that id SHALL be `0`.

#### Scenario: No cluster
- **WHEN** a program run with `lunatic run` and no control node calls `lucidity::local_node()`
- **THEN** it returns `0`

### Requirement: Remote nodes exclude the calling node
`lucidity::remote_nodes()` SHALL return the nodes in the cluster other than the calling node, and SHALL return an empty
list when no other nodes are connected or there is no cluster.

#### Scenario: Plain run
- **WHEN** a program run with `lunatic run` and no control node calls `lucidity::remote_nodes()`
- **THEN** it returns an empty list

#### Scenario: Two peers
- **WHEN** an app node registered with a control node that has two peer nodes calls `lucidity::remote_nodes()`
- **THEN** it returns the two peers' ids
- **AND** not the app node's own id

### Requirement: Local jobs need no cluster
`name_local` and `name_local_async` SHALL run the job on the calling node, and SHALL work under a plain `lunatic run`
with no control node or peers.

#### Scenario: Local flavors without a cluster
- **WHEN** the `pythagorean` example runs under `lunatic run` with no control node
- **THEN** `square_local(3)`, `square_local_async(4).await_get()`, and `pythagorean_local(3, 4)` return `9`, `16`, and
  `5.0`
- **AND** the program exits successfully

### Requirement: Remote jobs run on a random other node
`name_remote`, `name_remote_async`, and `lucidity::random_node()` SHALL pick a node uniformly at random from
`remote_nodes()`, so a remote job never runs on the calling node. When `remote_nodes()` is empty they SHALL panic with
`lucidity: no remote nodes are connected`.

#### Scenario: Remote call with no peers
- **WHEN** `square_remote(5)` is called by a process whose node has no peers
- **THEN** the calling process panics with `lucidity: no remote nodes are connected`

#### Scenario: Remote call with peers
- **WHEN** `square_remote(5)` is called on an app node whose cluster has two peers
- **THEN** the job runs on one of the two peers
- **AND** the call returns `25`

### Requirement: Fanout spreads one job per argument set
A fanout SHALL spawn one job per element of `args` before waiting on any of them, for both
`lucidity::fanout::<J>(args, strategy)` and the generated `name_remote_fanout(args)` (whose strategy is the job's
`fanout` attribute), placing the `k`-th job on `remote_nodes()[k % n]` (with `n` remote nodes) under
`Fanout::RoundRobin`, or on a uniformly random remote node for each job under `Fanout::Random`. It SHALL then block
until every job finishes and return the results in the order of `args`.

#### Scenario: Round robin over two peers
- **WHEN** `square_remote_fanout(vec![1, 2, 3])` is called with round robin and two remote nodes, P1 then P2
- **THEN** the jobs for `1` and `3` run on P1 and the job for `2` runs on P2
- **AND** the call returns `vec![1, 4, 9]`

#### Scenario: Results follow argument order
- **WHEN** `pythagorean_remote_fanout(vec![(3, 4), (6, 8), (5, 12)])` is called and the jobs finish in any order
- **THEN** it returns `vec![5.0, 10.0, 13.0]`

### Requirement: Fanout fails when a job fails or no peers exist
A fanout SHALL panic when any of its jobs dies or is lost, with that job's message as `job-lifecycle` describes. It
SHALL panic with `lucidity: no remote nodes are connected` when `remote_nodes()` is empty, even for an empty argument
list.

#### Scenario: Fanout with no peers
- **WHEN** `square_remote_fanout(vec![])` is called by a process whose node has no peers
- **THEN** the calling process panics with `lucidity: no remote nodes are connected`
