//! Keep libatspi's process-global GLib/DBus state out of the GTK process.
use super::{FieldSnapshot, Observation, Request};
#[cfg(not(test))]
use anyhow::Context;
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, Write},
    sync::mpsc,
    time::Instant,
};
use writy_core::types::Suggestion;

#[cfg(not(test))]
use std::{
    io::BufReader,
    process::{Child, ChildStdin, ChildStdout, Stdio},
};

const MAX_FRAME: u64 = 1024 * 1024;
#[derive(Serialize, Deserialize)]
enum Job {
    Inspect {
        pid: u32,
    },
    Replace {
        pid: u32,
        suggestion: Box<Suggestion>,
        expected: FieldSnapshot,
    },
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    job: Job,
    remaining_ms: u64,
}
#[derive(Serialize, Deserialize)]
enum Reply {
    Inspect(Observation),
    Replace(std::result::Result<(), String>),
}
#[cfg(not(test))]
struct Connection {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
#[cfg(not(test))]
impl Connection {
    fn start() -> Result<Self> {
        let mut child = std::process::Command::new(std::env::current_exe()?)
            .arg("accessibility-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child.stdin.take().context("Missing helper input")?;
        let output = BufReader::new(child.stdout.take().context("Missing helper output")?);
        Ok(Self {
            child,
            input,
            output,
        })
    }
    fn exchange(&mut self, job: Job, deadline: Instant) -> Result<Reply> {
        let mut frame = serde_json::to_vec(&Envelope {
            job,
            remaining_ms: deadline
                .saturating_duration_since(Instant::now())
                .as_millis() as u64,
        })?;
        ensure!(
            frame.len() < MAX_FRAME as usize,
            "Accessibility request exceeds limit"
        );
        frame.push(b'\n');
        self.input.write_all(&frame)?;
        self.input.flush()?;
        read_frame(&mut self.output)
    }
}
#[cfg(not(test))]
impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn read_frame<T: serde::de::DeserializeOwned>(input: &mut impl BufRead) -> Result<T> {
    let mut frame = String::new();
    std::io::Read::take(input, MAX_FRAME + 1).read_line(&mut frame)?;
    ensure!(
        !frame.is_empty() && frame.len() <= MAX_FRAME as usize && frame.ends_with('\n'),
        "Accessibility helper frame is missing or too large"
    );
    Ok(serde_json::from_str(&frame)?)
}
#[cfg(not(test))]
pub(super) fn broker(receiver: mpsc::Receiver<Request>) {
    let mut connection = None;
    for request in receiver {
        let (job, deadline, closed) = match &request {
            Request::Inspect {
                pid,
                deadline,
                reply,
            } => (Job::Inspect { pid: *pid }, *deadline, reply.is_closed()),
            Request::Replace {
                pid,
                suggestion,
                expected,
                deadline,
                reply,
            } => (
                Job::Replace {
                    pid: *pid,
                    suggestion: suggestion.clone(),
                    expected: expected.clone(),
                },
                *deadline,
                reply.is_closed(),
            ),
        };
        if closed || Instant::now() >= deadline {
            continue;
        }
        let response = (|| {
            if connection.is_none() {
                connection = Some(Connection::start()?);
            }
            connection.as_mut().unwrap().exchange(job, deadline)
        })();
        if response.is_err() {
            connection = None;
        }
        match (request, response) {
            (Request::Inspect { reply, .. }, Ok(Reply::Inspect(value))) => {
                let _ = reply.send(value);
            }
            (Request::Replace { reply, .. }, Ok(Reply::Replace(value))) => {
                let _ = reply.send(value.map_err(anyhow::Error::msg));
            }
            (Request::Inspect { reply, .. }, _) => {
                let _ = reply.send(Observation::unknown("Accessibility helper unavailable"));
            }
            (Request::Replace { reply, .. }, _) => {
                let _ = reply.send(Err(anyhow::anyhow!(
                    "Accessibility helper unavailable; use Copy"
                )));
            }
        }
    }
}

pub(crate) fn run() -> Result<()> {
    // The helper has one GLib thread and no GTK application/main loop.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
    }
    let (sender, receiver) = mpsc::sync_channel(2);
    let reader = std::thread::spawn(move || -> Result<()> {
        let mut input = std::io::stdin().lock();
        let mut output = std::io::stdout().lock();
        loop {
            let envelope = match read_frame::<Envelope>(&mut input) {
                Ok(job) => job,
                Err(_) => break,
            };
            let deadline = Instant::now()
                + std::time::Duration::from_millis(envelope.remaining_ms)
                    .min(super::OPERATION_LIMIT);
            let response = match envelope.job {
                Job::Inspect { pid } => {
                    let (reply, result) = tokio::sync::oneshot::channel();
                    sender.send(Request::Inspect {
                        pid,
                        deadline,
                        reply,
                    })?;
                    Reply::Inspect(result.blocking_recv()?)
                }
                Job::Replace {
                    pid,
                    suggestion,
                    expected,
                } => {
                    let (reply, result) = tokio::sync::oneshot::channel();
                    sender.send(Request::Replace {
                        pid,
                        suggestion,
                        expected,
                        deadline,
                        reply,
                    })?;
                    Reply::Replace(result.blocking_recv()?.map_err(|error| error.to_string()))
                }
            };
            serde_json::to_writer(&mut output, &response)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
        Ok(())
    });
    super::worker(receiver);
    match reader.join() {
        Ok(result) => result,
        Err(_) => bail!("Accessibility helper input failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_frames_require_a_bounded_complete_json_line() {
        for data in [
            b"".to_vec(),
            b"{}".to_vec(),
            b"bad\n".to_vec(),
            vec![b'x'; MAX_FRAME as usize + 2],
        ] {
            assert!(read_frame::<Envelope>(&mut &data[..]).is_err());
        }
        let frame = b"{\"job\":{\"Inspect\":{\"pid\":1}},\"remaining_ms\":20}\n";
        let envelope = read_frame::<Envelope>(&mut &frame[..]).unwrap();
        assert_eq!(envelope.remaining_ms, 20);
    }
}
