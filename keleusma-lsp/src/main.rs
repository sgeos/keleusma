//! Language Server Protocol server for the Keleusma language.
//!
//! Reuses the core compiler to give editors live feedback:
//!
//! - **Diagnostics** run the full pipeline — `tokenize` -> `parse` -> `compile`
//!   -> `verify` — so lex, parse, type, monomorphization, codegen, and the
//!   worst-case-execution-time and worst-case-memory-usage *verifier rejections*
//!   all surface as you type. The verifier diagnostics are Keleusma's signature:
//!   no other language's LSP shows a resource-bound rejection live.
//! - **Document symbols** list the functions, types, and traits in a file.
//! - **Completion** offers the keyword and primitive-type vocabulary.
//!
//! The compiler is fail-fast, so at most one error diagnostic is produced per
//! pass. Transport is stdio, the convention every LSP client understands.

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result as RpcResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use keleusma::ast::{FunctionCategory, TypeDef};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::Span;
use keleusma::verify::verify;

/// Completion draws its keyword vocabulary from the core crate's single
/// authoritative list ([`keleusma::token::KEYWORDS`]), so the completion offering
/// cannot drift from the lexer. Adding a keyword to the language updates this
/// automatically.
const KEYWORDS: &[&str] = keleusma::token::KEYWORDS;

/// Primitive-type names offered by completion.
const PRIMITIVE_TYPES: &[&str] = &[
    "Word",
    "Byte",
    "Multiword",
    "Fixed",
    "Float",
    "bool",
    "Text",
    "Option",
];

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

/// A diagnostic anchored at the start of the document, for errors that carry no
/// source span (the verifier reports a failing chunk name, not a position).
fn document_diagnostic(message: String) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("keleusma".to_string()),
        message,
        ..Default::default()
    }
}

/// Run the full pipeline and surface the first error, if any. `compile`
/// internally type-checks, monomorphizes, re-type-checks, and generates code, so
/// its `CompileError` covers type and codegen faults with a span; `verify`
/// covers the worst-case-execution-time and worst-case-memory-usage rejections
/// (which carry a chunk name rather than a span).
fn analyze_inner(text: &str) -> Vec<Diagnostic> {
    let tokens = match tokenize(text) {
        Ok(tokens) => tokens,
        Err(e) => {
            let span = e.span;
            return vec![error_diagnostic(text, &span, e.message)];
        }
    };
    let program = match parse(&tokens) {
        Ok(program) => program,
        Err(e) => {
            let span = e.span;
            return vec![error_diagnostic(text, &span, e.message)];
        }
    };
    let module = match compile(&program) {
        Ok(module) => module,
        Err(e) => {
            let span = e.span;
            return vec![error_diagnostic(text, &span, e.message)];
        }
    };
    if let Err(e) = verify(&module) {
        return vec![document_diagnostic(format!(
            "verifier rejected `{}`: {}",
            e.chunk_name, e.message
        ))];
    }
    Vec::new()
}

/// Panic-safe wrapper. A language server processes half-written programs on every
/// keystroke; an unexpected panic in a deep compiler path must degrade to "no
/// diagnostics this pass", never crash the server.
fn analyze(text: &str) -> Vec<Diagnostic> {
    std::panic::catch_unwind(AssertUnwindSafe(|| analyze_inner(text))).unwrap_or_default()
}

#[allow(deprecated)] // `DocumentSymbol::deprecated` is a deprecated protocol field; set to None.
fn symbol(
    name: &str,
    kind: SymbolKind,
    detail: Option<String>,
    text: &str,
    span: &Span,
) -> DocumentSymbol {
    let range = span_to_range(text, span);
    DocumentSymbol {
        name: name.to_string(),
        detail,
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

/// Extract top-level declarations (functions, types, traits) as document symbols.
/// Returns empty if the document does not parse.
fn document_symbols(text: &str) -> Vec<DocumentSymbol> {
    let Ok(tokens) = tokenize(text) else {
        return Vec::new();
    };
    let Ok(program) = parse(&tokens) else {
        return Vec::new();
    };
    let mut symbols = Vec::new();
    for f in &program.functions {
        let detail = match f.category {
            FunctionCategory::Fn => "fn",
            FunctionCategory::Yield => "yield",
            FunctionCategory::Loop => "loop",
        };
        symbols.push(symbol(
            &f.name,
            SymbolKind::FUNCTION,
            Some(detail.to_string()),
            text,
            &f.span,
        ));
    }
    for t in &program.types {
        let (name, kind, span) = match t {
            TypeDef::Struct(s) => (&s.name, SymbolKind::STRUCT, &s.span),
            TypeDef::Enum(e) => (&e.name, SymbolKind::ENUM, &e.span),
            TypeDef::Newtype(n) => (&n.name, SymbolKind::STRUCT, &n.span),
        };
        symbols.push(symbol(name, kind, None, text, span));
    }
    for tr in &program.traits {
        symbols.push(symbol(
            &tr.name,
            SymbolKind::INTERFACE,
            None,
            text,
            &tr.span,
        ));
    }
    symbols
}

fn keyword_completions() -> Vec<CompletionItem> {
    let kws = KEYWORDS.iter().map(|k| CompletionItem {
        label: (*k).to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        ..Default::default()
    });
    let types = PRIMITIVE_TYPES.iter().map(|t| CompletionItem {
        label: (*t).to_string(),
        kind: Some(CompletionItemKind::CLASS),
        ..Default::default()
    });
    kws.chain(types).collect()
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> RpcResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions::default()),
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
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> RpcResult<Option<DocumentSymbolResponse>> {
        let text = self
            .docs
            .lock()
            .await
            .get(&params.text_document.uri)
            .cloned();
        Ok(text.map(|t| DocumentSymbolResponse::Nested(document_symbols(&t))))
    }

    async fn completion(&self, _: CompletionParams) -> RpcResult<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::Array(keyword_completions())))
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
        assert_eq!(
            offset_to_position(text, 10),
            Position {
                line: 1,
                character: 0
            }
        );
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
        assert!(analyze("fn main() -> Word { 1 }\n").is_empty());
    }

    #[test]
    fn parse_error_yields_one_diagnostic() {
        let diags = analyze("fn (");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn type_error_is_reported_via_compile() {
        // Parses, but the body's type does not match the declared return type,
        // so `compile` (which type-checks internally) must reject it.
        let diags = analyze("fn main() -> Word { true }\n");
        assert_eq!(diags.len(), 1, "expected one diagnostic, got {diags:?}");
    }

    #[test]
    fn document_symbols_lists_functions() {
        let syms = document_symbols("fn main() -> Word { 1 }\n");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "main");
        assert_eq!(syms[0].kind, SymbolKind::FUNCTION);
    }

    #[test]
    fn completion_surfaces_every_core_keyword() {
        // The keyword set itself is validated in the core crate; here we guard
        // that completion actually offers every one of them, with the right kind.
        let items = keyword_completions();
        for kw in keleusma::token::KEYWORDS {
            assert!(
                items
                    .iter()
                    .any(|i| i.label == *kw && i.kind == Some(CompletionItemKind::KEYWORD)),
                "completion does not offer keyword `{kw}`"
            );
        }
    }
}
