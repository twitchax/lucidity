//! The module for automatically building a `lunatic` cluster on fly.io.

use std::time::Duration;

use lucidity_core::lunatic::{
    protocol::{Protocol, Send, TaskEnd},
    Process,
};
use serde_json::{json, Value};

const ENDPOINT: &str = "https://api.machines.dev/v1";

/// Ensures that the `lunatic` cluster is running on fly.io.
///
/// This function takes the app name, region, and number of machines to run.
/// It will delete any existing machines, create new ones, prepare them,
/// and then run the `lunatic` process on them, connecting the node to the
/// machine id that called this function.
pub fn ensure_machines(key: &str, count: usize) -> Result<(), String> {
    let app_name = std::env::var("FLY_APP_NAME").expect("FLY_APP_NAME not set.");
    let local_machine_id = std::env::var("FLY_MACHINE_ID").expect("FLY_MACHINE_ID not set.");
    let region = std::env::var("FLY_REGION").expect("FLY_REGION not set.");

    // Create all of the processes.
    let mut processes = Vec::new();
    for k in 1..=count {
        let machine_name = format!("lucid-{}", k);
        let p = Process::spawn_link(
            (key.to_owned(), app_name.to_owned(), machine_name, region.to_owned(), local_machine_id.to_owned()),
            |(key, app_name, machine_name, region, local_machine_id), m: Protocol<Send<Result<(), String>, TaskEnd>>| match ensure_machine(&key, &app_name, &machine_name, &region, &local_machine_id) {
                Ok(o) => {
                    let _ = m.send(Ok(o));
                }
                Err(e) => {
                    let _ = m.send(Err(e));
                }
            },
        );

        processes.push(p);
    }

    // Wait for all of the processes to finish.
    let mut results = Vec::new();
    for p in processes {
        results.push(p.result());
    }

    // Check for any errors.
    for r in results {
        match r {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        }
    }

    Ok(())
}

fn ensure_machine(key: &str, app_name: &str, machine_name: &str, region: &str, local_machine_id: &str) -> Result<(), String> {
    // Delete any existing machine.
    delete_machine(key, app_name, machine_name);

    // Wait for the machine to be deleted.
    crate::lunatic::sleep(Duration::from_secs(10));

    // Create a new machine.
    create_machine(key, app_name, machine_name, region, local_machine_id).map_err(|e| format!("[ensure_machine] Failed to create machine.  {}", e))?;

    // Wait for the machine to be ready.
    crate::lunatic::sleep(Duration::from_secs(10));

    Ok(())
}

/// List the currently running machines for the app.
pub fn list_machines(key: &str, app_name: &str) -> Result<Vec<Value>, String> {
    let client = nightfly::Client::new();

    let response = client
        .get(format!("{}/apps/{}/machines", ENDPOINT, app_name))
        .bearer_auth(key)
        .send()
        .map_err(|e| format!("[list_machines] Failed to send request.  {}", e))?;

    let value = response.json::<Vec<Value>>().map_err(|e| format!("[list_machines] Failed to parse response.  {}", e))?;

    Ok(value)
}

fn machine_id_from_name(key: &str, app_name: &str, machine_name: &str) -> Result<String, String> {
    let machines = list_machines(key, app_name).map_err(|e| format!("[machine_id_from_name] Failed to list machines.  {}", e))?;

    for machine in machines {
        if machine["name"].as_str().unwrap() == machine_name {
            return Ok(machine["id"].as_str().unwrap().to_string());
        }
    }

    Err(format!("[machine_id_from_name] Machine with name {} not found.", machine_name))
}

fn create_machine(key: &str, app_name: &str, machine_name: &str, region: &str, local_machine_id: &str) -> Result<(), String> {
    let client = nightfly::Client::new();

    let body = json!({
        "name": machine_name,
        "region": region,
        "config": {
            "init": {
                "exec": [
                    "/app.entrypoint.sh",
                    format!("http://{}.vm.{}.internal:3030/", local_machine_id, app_name)
                ]
            },
            "image": "twitchax/lunatic:2024.01.10",
            "auto_destroy": true,
            "restart": {
                "policy": "always"
            },
            "guest": {
                "cpu_kind": "shared",
                "cpus": 1,
                "memory_mb": 1024
            },
            "services": [
                {
                    "protocol": "udp",
                    "internal_port": 3031
                },
            ],
        }
    });

    let _ = client
        .post(format!("{}/apps/{}/machines", ENDPOINT, app_name))
        .bearer_auth(key)
        .json(body)
        .send()
        .map_err(|e| format!("[create_machine] Failed to send request.  {}", e))?;

    Ok(())
}

fn delete_machine(key: &str, app_name: &str, machine_name: &str) {
    let machine_id = match machine_id_from_name(key, app_name, machine_name) {
        Ok(o) => o,
        Err(_) => return,
    };

    let client = nightfly::Client::new();

    // Swallow errors, as it is possible that the machine doesn't exist.
    let _ = client.post(format!("{}/apps/{}/machines/{}/stop", ENDPOINT, app_name, machine_id)).bearer_auth(key).send();

    // Swallow errors, as it is possible that the machine doesn't exist.
    let _ = client.delete(format!("{}/apps/{}/machines/{}", ENDPOINT, app_name, machine_id)).bearer_auth(key).send();
}
