// VS Code language client for Keleusma.
//
// Launches the keleusma-lsp server over stdio and connects it to `.kel`
// documents, giving live diagnostics (including the WCET and WCMU verifier
// rejections), document symbols, and completion on top of the static syntax
// highlighting the grammar already provides.
//
// The extension is plain JavaScript so it needs no TypeScript build step. The
// only runtime dependency is `vscode-languageclient` (see package.json); install
// it with `npm install` in this directory before packaging or running.

const { workspace, window } = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

/** @type {import("vscode-languageclient/node").LanguageClient | undefined} */
let client;

function activate() {
  const config = workspace.getConfiguration("keleusma");
  if (!config.get("server.enable", true)) {
    return;
  }

  const command = config.get("server.path", "keleusma-lsp");
  const serverOptions = {
    run: { command, transport: TransportKind.stdio },
    debug: { command, transport: TransportKind.stdio },
  };
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "keleusma" }],
  };

  client = new LanguageClient(
    "keleusma",
    "Keleusma Language Server",
    serverOptions,
    clientOptions,
  );

  client.start().catch((err) => {
    window.showErrorMessage(
      `keleusma-lsp failed to start (path: "${command}"). Build it in the ` +
        `keleusma-lsp/ directory and set "keleusma.server.path", or disable the ` +
        `server with "keleusma.server.enable": false. ${err}`,
    );
  });
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
