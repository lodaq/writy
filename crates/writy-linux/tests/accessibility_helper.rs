use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn missing_bus_is_reported_without_aborting_and_eof_stops_helper() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_writy"))
        .arg("accessibility-worker")
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/writy-session",
        )
        .env(
            "AT_SPI_BUS_ADDRESS",
            "unix:path=/nonexistent/writy-accessibility",
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"job\":{\"Inspect\":{\"pid\":1}},\"remaining_ms\":1500}\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let reply: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(reply["Inspect"].to_string().contains("unavailable"));
}
