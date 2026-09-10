//! Exercise the actual executable with synthetic invalid requests only: these
//! tests never connect to a running desktop or access a personal vault.
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn run(input: &[u8], origin: &str) -> Vec<u8> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_keynest-native-host"))
        .args([origin, "--parent-window=0"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native host did not exit after stdin EOF");
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success());
    // Responses here are tiny and cannot fill the OS stdout/stderr pipe buffers.
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    assert!(stderr.is_empty(), "unexpected host diagnostics");
    stdout
}

fn frame(bytes: &[u8]) -> Vec<u8> {
    let mut frame = (bytes.len() as u32).to_ne_bytes().to_vec();
    frame.extend_from_slice(bytes);
    frame
}

fn response(stdout: &mut &[u8]) -> serde_json::Value {
    let mut prefix = [0; 4];
    stdout.read_exact(&mut prefix).unwrap();
    let length = u32::from_ne_bytes(prefix) as usize;
    assert!((1..=65536).contains(&length));
    let mut body = vec![0; length];
    stdout.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[test]
fn executable_preserves_ids_and_emits_only_binary_protocol_frames() {
    let origin = format!("chrome-extension://{}/", "a".repeat(32));
    let mut input = Vec::new();
    for id in ["invalid-version", "unknown-type"] {
        let request = if id == "invalid-version" {
            serde_json::json!({"version":2,"requestId":id,"type":"status"})
        } else {
            serde_json::json!({"version":1,"requestId":id,"type":"unlock"})
        };
        input.extend(frame(&serde_json::to_vec(&request).unwrap()));
    }
    // Include a frame with length 26 (0x1a, the CRT text-mode EOF character).
    // It is valid framing but an invalid object schema. Rust must read it and
    // return a third error, then continue to the following malformed UTF-8 body.
    input.extend(frame(&[b' '; 26]));
    input.extend(frame(b"\xff\n\r\n"));
    let output = run(&input, &origin);
    let mut stdout = output.as_slice();
    for id in ["invalid-version", "unknown-type"] {
        let value = response(&mut stdout);
        assert_eq!(value["requestId"], id);
        assert_eq!(value["error"]["code"], "INVALID_REQUEST");
    }
    for _ in 0..2 {
        let value = response(&mut stdout);
        assert!(value["requestId"].is_null());
        assert_eq!(value["error"]["code"], "INVALID_REQUEST");
    }
    assert!(stdout.is_empty(), "stdout contains unframed bytes");
}

#[test]
fn executable_fails_closed_on_bad_lengths_and_exits_cleanly_on_eof() {
    let origin = format!("chrome-extension://{}/", "a".repeat(32));
    for input in [
        0_u32.to_ne_bytes().to_vec(),
        65537_u32.to_ne_bytes().to_vec(),
        u32::MAX.to_ne_bytes().to_vec(),
        vec![1, 0],
        vec![2, 0, 0, 0, b'{'],
    ] {
        let output = run(&input, &origin);
        let mut stdout = output.as_slice();
        assert_eq!(response(&mut stdout)["error"]["code"], "INVALID_REQUEST");
        assert!(stdout.is_empty());
    }
    assert!(run(&[], &origin).is_empty());
    assert!(run(&[], "https://invalid.test").is_empty());
}
