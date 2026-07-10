//! Language Server Protocol server for the Keleusma language.
//!
//! Milestone 1 reuses the compiler front end (`tokenize` -> `parse` -> `check`)
//! to publish live diagnostics as the user edits. The compiler is fail-fast, so
//! at most one diagnostic is produced per analysis pass; multi-error recovery,
//! the compile/verify (WCET and WCMU) diagnostics, document symbols, completion,
//! and hover are later milestones.
//!
//! Transport is stdio, the convention every LSP client (VS Code, Neovim, Helix,
//! Emacs) understands. The server holds full-text copies of open documents and
//! re-analyses on every change.

use std::collections::HashMap;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result as RpcResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::Span;
use keleusma::typecheck::check;

/// Server state: the client handle and the set of open documents by URI.
struct Backend {
    client: Client,
    docs: Mutex<HashMap<Url, String>>,
}

impl Backend {
    async fn refresh(&self, uri: Url, text: String) {
        let diagnostics = analyze(&text);
        self.docs.lock().await.insert(uri.clone(), text);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

/// Convert a byte offset in `text` to an LSP [`Position`] (zero-based line, and
/// character measured in UTF-16 code units, as the protocol requires by default).
fn offset_to_position(text: &str, offset: usize) -> Position {
    let offset = offset.min(text.len());
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (i, b) in text.bytes().enumerate() {
        if i >= offset {
            break;
        }
        if b == b'\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    let character = text[line_start..offset].encode_utf16().count() as u32;
    Position { line, character }
}

fn span_to_range(text: &str, span: &Span) -> Range {
    Range {
        start: offset_to_position(text, span.start),
        end: offset_to_position(text, span.end.max(span.start)),
    }
}

fn error_diagnostic(text: &str, span: &Span, message: String) -> Diagnostic {
    Diagnostic {
        range: span_to_range(text, span),
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("keleusma".to_string()),
        message,
        ..Default::default()
    }
}

/// Run lex -> parse -> typecheck and surface the first error, if any, as a
/// diagnostic. Each stage carries a `Span`, so positions are exact.
fn analyze(text: &str) -> Vec<Diagnostic> {
    let tokens = match tokenize(text) {
        Ok(tokens) => tokens,
        Err(e) => {
            let span = e.span;
            return vec![error_diagnostic(text, &span, e.message)];
        }
    };
    let mut program = match parse(&tokens) {
        Ok(program) => program,
        Err(e) => {
            let span = e.span;
            return vec![error_diagnostic(text, &span, e.message)];
        }
    };
    if let Err(e) = check(&mut program) {
        let span = e.span;
        return vec![error_diagnostic(text, &span, e.message)];
    }
    Vec::new()
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> RpcResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "keleusma-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "keleusma-lsp ready")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.refresh(params.text_document.uri, params.text_document.text)
            .await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        // FULL sync: the final content change carries the whole document.
        if let Some(change) = params.content_changes.pop() {
            self.refresh(params.text_document.uri, change.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.docs.lock().await.remove(&params.text_document.uri);
        // Clear diagnostics for the closed document.
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn shutdown(&self) -> RpcResult<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        docs: Mutex::new(HashMap::new()),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_to_position_handles_multibyte_and_newlines() {
        let text = "fn a() {}\nlet x = 1\n";
        // Start of line 2 (byte offset 10) is line 1, character 0.
        assert_eq!(
            offset_to_position(text, 10),
            Position {
                line: 1,
                character: 0
            }
        );
        // Offset 0 is the origin.
        assert_eq!(
            offset_to_position(text, 0),
            Position {
                line: 0,
                character: 0
            }
        );
    }

    #[test]
    fn clean_source_yields_no_diagnostics() {
        // A trivially well-formed program should lex, parse, and typecheck.
        let diags = analyze("fn main() -> Word { 1 }\n");
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn lex_or_parse_error_yields_one_diagnostic() {
        // An obviously malformed program should surface exactly one diagnostic.
        let diags = analyze("fn (");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    }
}
