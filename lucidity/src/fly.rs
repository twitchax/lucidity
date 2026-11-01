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
    // TODO: Use events instead of sleeping.
    crate::lunatic::sleep(Duration::from_secs(30));

    // Create a new machine.
    create_machine(ENDPOINT, key, app_name, machine_name, region, local_machine_id).map_err(|e| format!("[ensure_machine] Failed to create machine.  {}", e))?;

    // Wait for the machine to be ready.
    // TODO: Use events instead of sleeping.
    crate::lunatic::sleep(Duration::from_secs(30));

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

fn create_machine(endpoint: &str, key: &str, app_name: &str, machine_name: &str, region: &str, local_machine_id: &str) -> Result<(), String> {
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

    let response = client
        .post(format!("{}/apps/{}/machines", endpoint, app_name))
        .bearer_auth(key)
        .json(body)
        .send()
        .map_err(|e| format!("[create_machine] Failed to send request.  {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(format!("[create_machine] Request rejected with status {}.  {}", status, body));
    }

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

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use lucidity_core::lunatic::{net::TcpListener, Mailbox, Process};

    use super::create_machine;

    #[test]
    fn a_created_machine_is_ok() {
        let endpoint = serve_once("200 OK", r#"{"id":"abc"}"#);

        assert_eq!(create_machine(&endpoint, "key", "app", "lucid-1", "sjc", "abc"), Ok(()));
    }

    #[test]
    fn a_rejected_create_is_an_error_naming_the_status_and_body() {
        let endpoint = serve_once("422 Unprocessable Entity", r#"{"error":"invalid image"}"#);

        let error = create_machine(&endpoint, "key", "app", "lucid-1", "sjc", "abc").unwrap_err();

        assert!(error.starts_with("[create_machine] Request rejected with status 422 Unprocessable Entity."), "{error}");
        assert!(error.contains(r#"{"error":"invalid image"}"#), "{error}");
    }

    /// Starts a server that answers one request with `status` and the JSON `body`, as the Machines API would, and
    /// returns its endpoint.
    fn serve_once(status: &str, body: &str) -> String {
        let this = unsafe { Process::<u16>::this() };
        Process::spawn((this, status.to_owned(), body.to_owned()), serve);
        let port = unsafe { Mailbox::<u16>::new() }.receive();

        format!("http://127.0.0.1:{port}")
    }

    fn serve((test, status, body): (Process<u16>, String, String), _: Mailbox<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        test.send(listener.local_addr().unwrap().port());

        let (mut stream, _) = listener.accept().unwrap();
        // Read the whole request first: closing a connection with unread data resets it, possibly before the client
        // reads the response.
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !is_complete(&request) {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "the client closed the connection mid-request");
            request.extend_from_slice(&buffer[..read]);
        }

        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        stream.flush().unwrap();
    }

    // Whether `request` holds its headers and as much body as their `Content-Length` announces.
    fn is_complete(request: &[u8]) -> bool {
        let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            return false;
        };
        let headers = String::from_utf8_lossy(&request[..end]);
        let length = headers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .map_or(0, |(_, value)| value.trim().parse::<usize>().unwrap());

        request.len() >= end + 4 + length
    }
}
