//! The module for automatically building a `lunatic` cluster on fly.io.

use std::time::Duration;

use lucidity_core::lunatic::{
    protocol::{Protocol, Send, TaskEnd},
    Process,
};
use serde_json::{json, Value};

mod http;

const ENDPOINT: &str = "https://api.machines.dev/v1";

/// Ensures that the `lunatic` cluster is running on fly.io.
///
/// Starts `count` worker machines, named `lucid-1` and up, in the calling machine's app and region, and runs a
/// `lunatic` node on each, connected to the control node the calling machine runs on port 3030.  Any existing machines
/// with those names are deleted first.  `key` is a fly.io API token; the app, machine, and region come from `FLY_APP_NAME`,
/// `FLY_MACHINE_ID`, and `FLY_REGION`.
///
/// Returns `Err` naming the status and body of the first machine create the API rejects.  Panics if any of those
/// variables is unset.
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
    let response = http::request("GET", &format!("{}/apps/{}/machines", ENDPOINT, app_name), key, None).map_err(|e| format!("[list_machines] Failed to send request.  {}", e))?;

    let value = serde_json::from_str::<Vec<Value>>(&response.body).map_err(|e| format!("[list_machines] Failed to parse response.  {}", e))?;

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
            "image": "twitchax/lunatic:2026.11.02",
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

    let response = http::request("POST", &format!("{}/apps/{}/machines", endpoint, app_name), key, Some(&body)).map_err(|e| format!("[create_machine] Failed to send request.  {}", e))?;

    if !response.is_success() {
        return Err(format!("[create_machine] Request rejected with status {}.  {}", response.status, response.body));
    }

    Ok(())
}

fn delete_machine(key: &str, app_name: &str, machine_name: &str) {
    let machine_id = match machine_id_from_name(key, app_name, machine_name) {
        Ok(o) => o,
        Err(_) => return,
    };

    // Swallow errors, as it is possible that the machine doesn't exist.
    let _ = http::request("POST", &format!("{}/apps/{}/machines/{}/stop", ENDPOINT, app_name, machine_id), key, None);

    // Swallow errors, as it is possible that the machine doesn't exist.
    let _ = http::request("DELETE", &format!("{}/apps/{}/machines/{}", ENDPOINT, app_name, machine_id), key, None);
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use lucidity_core::lunatic::{net::TcpListener, Mailbox, Process, Tag};

    use super::{create_machine, http};

    #[test]
    fn a_created_machine_is_ok() {
        let server = serve_once("200 OK", r#"{"id":"abc"}"#);

        assert_eq!(create_machine(&server.endpoint, "key", "app", "lucid-1", "sjc", "abc"), Ok(()));
    }

    #[test]
    fn a_create_posts_the_machine_with_the_key() {
        let server = serve_once("200 OK", r#"{"id":"abc"}"#);

        create_machine(&server.endpoint, "key", "app", "lucid-1", "sjc", "abc").unwrap();

        let request = server.request();
        let (head, body) = request.split_once("\r\n\r\n").unwrap();
        assert!(head.starts_with("POST /apps/app/machines HTTP/1.1\r\n"), "{head}");
        assert!(head.contains("\r\nAuthorization: Bearer key\r\n"), "{head}");
        assert!(head.contains("\r\nContent-Type: application/json\r\n"), "{head}");
        let body: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["name"], "lucid-1");
        assert_eq!(body["config"]["image"], "twitchax/lunatic:2026.11.02");
    }

    #[test]
    fn a_chunked_response_is_decoded() {
        let server = serve_raw("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\n[{\"a\r\n7;ext=1\r\n\":1},{}\r\n1\r\n]\r\n0\r\n\r\n");

        let response = http::request("GET", &format!("{}/apps/app/machines", server.endpoint), "key", None).unwrap();

        assert_eq!(response.status, "200 OK");
        assert_eq!(response.body, r#"[{"a":1},{}]"#);
    }

    #[test]
    fn a_rejected_create_is_an_error_naming_the_status_and_body() {
        let server = serve_once("422 Unprocessable Entity", r#"{"error":"invalid image"}"#);

        let error = create_machine(&server.endpoint, "key", "app", "lucid-1", "sjc", "abc").unwrap_err();

        assert!(error.starts_with("[create_machine] Request rejected with status 422 Unprocessable Entity."), "{error}");
        assert!(error.contains(r#"{"error":"invalid image"}"#), "{error}");
    }

    /// A server that answers one request.
    struct Server {
        endpoint: String,
        tag: Tag,
    }

    impl Server {
        /// The request the server got.
        fn request(&self) -> String {
            unsafe { Mailbox::<String>::new() }.tag_receive(&[self.tag])
        }
    }

    /// Starts a server that answers one request with `status` and the JSON `body`, as the Machines API would.
    fn serve_once(status: &str, body: &str) -> Server {
        serve_raw(&format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ))
    }

    /// Starts a server that answers one request with `response`.
    fn serve_raw(response: &str) -> Server {
        // The tests share a process, so the server tags what it sends it: a test never reads another test's leftovers.
        let tag = Tag::new();
        let test = unsafe { Process::<String>::this() };
        Process::spawn((test, tag, response.to_owned()), serve);
        let port = unsafe { Mailbox::<u16>::new() }.tag_receive(&[tag]);

        Server {
            endpoint: format!("http://127.0.0.1:{port}"),
            tag,
        }
    }

    fn serve((test, tag, response): (Process<String>, Tag, String), _: Mailbox<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        unsafe { Process::<u16>::new(test.node_id(), test.id()) }.tag_send(tag, port);

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

        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
        test.tag_send(tag, String::from_utf8(request).unwrap());
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
