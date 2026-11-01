//! The language server over its real wire protocol.
//!
//! # Why this file exists
//!
//! The crate's other tests call `analyze`, `document_symbols` and the
//! completion helper directly. Those are the pure functions. **Nothing covered
//! the layer an editor actually talks to** — the framing, the dispatch, the
//! initialize handshake, the diagnostics notification.
//!
//! That is the same shape as the task runner, which was wholly non-functional
//! while its unit-level pieces passed. A correct `analyze` says nothing about
//! whether a reply is framed in a way an editor can read.
//!
//! # The harness trap, paid for once
//!
//! **The server's input must stay open for as long as replies are expected.**
//! Driving it by piping a finite file makes standard input hit end-of-file
//! immediately; the server then cancels work in flight and answers
//! `initialize` with `-32800 Canceled`, after which every later request is
//! refused as "Server not initialized". That is exactly what a broken server
//! would look like, and it is entirely an artefact of the harness. The first
//! probe written for this work did precisely that and produced a confident
//! false negative.
//!
//! So [`Server`] holds the child's standard input for its whole lifetime, and
//! drops it only when the server is torn down.
//!
//! # Where these run
//!
//! This crate is outside the main workspace, so a root `cargo test` does not
//! reach it. Continuous integration runs `cargo test` for it in its own job,
//! which is why these tests are worth adding here rather than elsewhere.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use serde_json::{Value, json};

/// How long any single reply may take.
///
/// A server that stops replying must FAIL a test rather than hang the suite,
/// which is why every read is bounded and a timeout is an assertion failure
/// rather than a skip.
const REPLY_TIMEOUT: Duration = Duration::from_secs(20);

struct Server {
    child: Child,
    /// Held for the server's whole lifetime. Dropping it closes the server's
    /// input, which cancels its work; see the module note.
    stdin: Option<ChildStdin>,
    rx: Receiver<Value>,
}

impl Server {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_keleusma-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the language server");
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            loop {
                // Read headers until the blank line, taking the content length.
                let mut len: Option<usize> = None;
                loop {
                    let mut line = String::new();
                    match r.read_line(&mut line) {
                        Ok(0) => return,
                        Ok(_) => {}
                        Err(_) => return,
                    }
                    let t = line.trim_end();
                    if t.is_empty() {
                        break;
                    }
                    if let Some(rest) = t.strip_prefix("Content-Length:") {
                        len = rest.trim().parse().ok();
                    }
                }
                let Some(n) = len else { return };
                let mut body = vec![0u8; n];
                if r.read_exact(&mut body).is_err() {
                    return;
                }
                match serde_json::from_slice::<Value>(&body) {
                    Ok(v) => {
                        if tx.send(v).is_err() {
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
        });
        Server {
            child,
            stdin: Some(stdin),
            rx,
        }
    }

    fn send(&mut self, v: Value) {
        let body = serde_json::to_vec(&v).expect("encode");
        let w = self.stdin.as_mut().expect("stdin still held");
        write!(w, "Content-Length: {}\r\n\r\n", body.len()).expect("write header");
        w.write_all(&body).expect("write body");
        w.flush().expect("flush");
    }

    /// Reads messages until `pred` matches, or fails.
    ///
    /// A timeout is a failure, never a skip: a server that said nothing has not
    /// demonstrated the behaviour under test.
    fn wait_for(&mut self, what: &str, mut pred: impl FnMut(&Value) -> bool) -> Value {
        let deadline = std::time::Instant::now() + REPLY_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(v) => {
                    if pred(&v) {
                        return v;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    panic!("the server sent no {what} within {REPLY_TIMEOUT:?}");
                }
                Err(RecvTimeoutError::Disconnected) => {
                    panic!("the server closed its output before sending {what}");
                }
            }
        }
    }

    /// Completes the handshake and returns the advertised capabilities.
    fn initialize(&mut self) -> Value {
        self.send(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"processId": null, "rootUri": null, "capabilities": {}}
        }));
        let reply = self.wait_for("initialize reply", |v| v.get("id") == Some(&json!(1)));
        assert!(
            reply.get("error").is_none(),
            "initialize failed: {reply}. If this is `Canceled`, the harness closed the \
             server's input; see the module note."
        );
        self.send(json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
        reply["result"]["capabilities"].clone()
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.send(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": {"textDocument": {
                "uri": uri, "languageId": "keleusma", "version": 1, "text": text
            }}
        }));
    }

    /// The diagnostics published for `uri`.
    fn diagnostics_for(&mut self, uri: &str) -> Vec<Value> {
        let m = self.wait_for("publishDiagnostics", |v| {
            v.get("method") == Some(&json!("textDocument/publishDiagnostics"))
                && v["params"]["uri"] == json!(uri)
        });
        m["params"]["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Close the input first, then kill: the ordering mirrors how an editor
        // shuts a server down.
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The handshake succeeds and advertises the capabilities the server provides.
///
/// Asserted as presence of named capabilities rather than by comparing the
/// whole object, so adding a capability does not fail this while removing one
/// does.
#[test]
fn initialize_succeeds_and_advertises_what_the_server_implements() {
    let mut s = Server::start();
    let caps = s.initialize();
    for name in [
        "textDocumentSync",
        "documentSymbolProvider",
        "completionProvider",
    ] {
        assert!(
            caps.get(name).is_some(),
            "the server no longer advertises {name}; its capabilities were {caps}"
        );
    }
}

/// Opening a malformed document publishes a diagnostic through the protocol.
#[test]
fn a_malformed_document_publishes_a_diagnostic() {
    let mut s = Server::start();
    s.initialize();
    let uri = "file:///malformed.kel";
    s.open(uri, "fn (");
    let d = s.diagnostics_for(uri);
    assert!(
        !d.is_empty(),
        "a document that does not parse produced no diagnostics"
    );
    // Structure, not wording: a message is free to be reworded, a range is not
    // free to be absent.
    assert!(
        d[0].get("range").is_some() && d[0].get("message").is_some(),
        "the diagnostic is missing a range or a message: {}",
        d[0]
    );
}

/// Opening a valid document publishes none.
///
/// The control. Without it, a server that reported an error for every document
/// would satisfy the test above.
#[test]
fn a_valid_document_publishes_no_diagnostics() {
    let mut s = Server::start();
    s.initialize();
    let uri = "file:///valid.kel";
    s.open(uri, "fn main() -> Word { 1 }\n");
    let d = s.diagnostics_for(uri);
    assert!(
        d.is_empty(),
        "a valid program produced {} diagnostic(s): {:?}",
        d.len(),
        d
    );
}

/// Document symbols come back over the protocol for an opened document.
#[test]
fn document_symbols_are_returned_over_the_protocol() {
    let mut s = Server::start();
    s.initialize();
    let uri = "file:///syms.kel";
    s.open(
        uri,
        "fn helper(a: Word) -> Word { a }\nfn main() -> Word { 1 }\n",
    );
    let _ = s.diagnostics_for(uri);
    s.send(json!({
        "jsonrpc": "2.0", "id": 7, "method": "textDocument/documentSymbol",
        "params": {"textDocument": {"uri": uri}}
    }));
    let reply = s.wait_for("documentSymbol reply", |v| v.get("id") == Some(&json!(7)));
    assert!(
        reply.get("error").is_none(),
        "documentSymbol failed: {reply}"
    );
    let text = reply["result"].to_string();
    assert!(
        text.contains("main") && text.contains("helper"),
        "the symbol reply names neither function: {text}"
    );
}

/// Completion comes back over the protocol and is not empty.
#[test]
fn completion_is_returned_over_the_protocol() {
    let mut s = Server::start();
    s.initialize();
    let uri = "file:///comp.kel";
    s.open(uri, "fn main() -> Word { 1 }\n");
    let _ = s.diagnostics_for(uri);
    s.send(json!({
        "jsonrpc": "2.0", "id": 9, "method": "textDocument/completion",
        "params": {
            "textDocument": {"uri": uri},
            "position": {"line": 0, "character": 0}
        }
    }));
    let reply = s.wait_for("completion reply", |v| v.get("id") == Some(&json!(9)));
    assert!(reply.get("error").is_none(), "completion failed: {reply}");
    let text = reply["result"].to_string();
    assert!(text.len() > 2, "the completion reply is empty: {text}");
}
