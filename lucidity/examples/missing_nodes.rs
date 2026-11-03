//! A job spawned on any of several nodes skips the nodes that aren't in the cluster, and runs on one that is.
//!
//! Run it inside a `lunatic node` with a peer (see the `pythagorean` example): it should print "missing nodes: ok", and
//! then panic with "none of the nodes ... is in the cluster".

fn main() {
    let peer = *lucidity::remote_nodes().first().expect("missing_nodes needs a peer node");

    // No node has these ids, so nearly every job picks some of them before the peer.
    let mut nodes = (1..=15).map(|k| u64::MAX - k).collect::<Vec<_>>();
    nodes.push(peer);

    for _ in 0..8 {
        assert_eq!(lucidity::Job::<LocateJob>::spawn_any(&nodes, ()).await_get(), peer);
    }
    println!("missing nodes: ok");

    lucidity::Job::<LocateJob>::spawn_any(&nodes[..15], ());
    println!("unreachable: spawned on a missing node");
}

#[lucidity::job]
fn locate() -> u64 {
    lucidity::local_node()
}
