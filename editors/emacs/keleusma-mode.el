;;; keleusma-mode.el --- Major mode for the Keleusma scripting language -*- lexical-binding: t; -*-

;; Author: Brendan Sechter <sgeos@hotmail.com>
;; Maintainer: Brendan Sechter <sgeos@hotmail.com>
;; Version: 0.2.0
;; Keywords: languages
;; URL: https://github.com/sgeos/keleusma
;; SPDX-License-Identifier: 0BSD

;;; Commentary:

;; Major mode for editing Keleusma source files (`.kel`).  Provides
;; syntax-highlighting categorisation aligned with the vim and VSCode
;; integrations: the three function categories (`fn', `yield', `loop')
;; share one face; storage-discipline modifiers (`signed', `ephemeral',
;; `shared', `private', `const') share another; the checked-arithmetic
;; arm keywords (`ok', `overflow', `underflow', `saturate_max',
;; `saturate_min') share a third; information-flow labels (`@Label',
;; `@!Negative', `@{A, B}') and the operators `classify' / `declassify'
;; share a fourth; and the pipeline operator `|>' shares the fourth as
;; well so the IFC machinery and the stream-style composition syntax
;; stand out together.
;;
;; To enable manually: M-x keleusma-mode.  To enable automatically on
;; `.kel' files: add (require 'keleusma-mode) to your init file.
;;
;; Highlighting categories audited against `docs/spec/GRAMMAR.md'
;; Section 2 (keywords, operators, comments) and Section 3 (primitive
;; types).

;;; Code:

(defvar keleusma-mode-syntax-table
  (let ((table (make-syntax-table)))
    ;; Line comments: //
    (modify-syntax-entry ?/ ". 124b" table)
    (modify-syntax-entry ?\n "> b" table)
    ;; Block comments: /* */
    (modify-syntax-entry ?* ". 23" table)
    ;; String quotes
    (modify-syntax-entry ?\" "\"" table)
    ;; Word constituents include digits and underscore
    (modify-syntax-entry ?_ "w" table)
    table)
  "Syntax table for `keleusma-mode'.")

(defconst keleusma-function-category-keywords
  '("fn" "yield" "loop")
  "Function category declarations.")

(defconst keleusma-conditional-keywords
  '("if" "else" "match" "when")
  "Conditional and pattern-match keywords.")

(defconst keleusma-repeat-keywords
  '("for" "break")
  "Iteration keywords.")

(defconst keleusma-storage-class-keywords
  '("signed" "ephemeral" "shared" "private" "const")
  "V0.2.0 storage-discipline modifiers.")

(defconst keleusma-other-keywords
  '("let" "in" "use" "external" "struct" "enum" "newtype"
    "trait" "impl" "data" "pure" "where" "as")
  "Reserved keywords that are neither function categories, storage
modifiers, conditionals, checked-arithmetic arms, nor word operators.")

(defconst keleusma-word-operator-keywords
  '("and" "or" "xor" "not" "andalso" "orelse"
    "lsl" "asl" "lsr" "asr" "band" "bor" "bxor" "bnot")
  "B19 word-form operators: eager boolean (and, or, xor, not),
short-circuit boolean (andalso, orelse), assembly-mnemonic shifts
(lsl, asl, lsr, asr), and per-limb bitwise (band, bor, bxor, bnot).")

(defconst keleusma-checked-arm-keywords
  '("ok" "overflow" "underflow" "saturate_max" "saturate_min")
  "Arm keywords of the numeric overflow construct.")

(defconst keleusma-ifc-operator-keywords
  '("classify" "declassify")
  "Information-flow operators (context-sensitive in the parser but
highlighted unconditionally because the convention is not to name
user-defined functions `classify' or `declassify').")

(defconst keleusma-boolean-keywords
  '("true" "false")
  "Boolean literals.")

(defconst keleusma-primitive-types
  '("Multiword" "Byte" "Word" "Fixed" "Float" "bool" "Text" "Option")
  "Primitive surface types per docs/spec/GRAMMAR.md Section 3.  Note
that `bool' is lowercase per the grammar.")

(defun keleusma--word-regexp (words)
  "Return a regexp matching any of WORDS as a whole identifier."
  (concat "\\<" (regexp-opt words) "\\>"))

(defvar keleusma-font-lock-keywords
  `(
    ;; Shebang: only at beginning of buffer
    ("\\`#!.*$" . font-lock-comment-face)
    ;; Information-flow label annotations: @Label, @!Negative, @{A, B}
    ("@!?\\(?:[A-Za-z_][A-Za-z0-9_]*\\|{[^}]*}\\)" . font-lock-builtin-face)
    ;; IFC operators: classify, declassify
    (,(keleusma--word-regexp keleusma-ifc-operator-keywords) . font-lock-builtin-face)
    ;; Pipeline operator
    ("|>" . font-lock-builtin-face)
    ;; Function category declarations
    (,(keleusma--word-regexp keleusma-function-category-keywords) . font-lock-keyword-face)
    ;; Storage class modifiers
    (,(keleusma--word-regexp keleusma-storage-class-keywords) . font-lock-type-face)
    ;; Checked-arithmetic arm keywords
    (,(keleusma--word-regexp keleusma-checked-arm-keywords) . font-lock-builtin-face)
    ;; Conditionals and pattern matching
    (,(keleusma--word-regexp keleusma-conditional-keywords) . font-lock-keyword-face)
    ;; Repeat
    (,(keleusma--word-regexp keleusma-repeat-keywords) . font-lock-keyword-face)
    ;; Other reserved keywords
    (,(keleusma--word-regexp keleusma-other-keywords) . font-lock-keyword-face)
    ;; Word-form operators (boolean, shift, per-limb bitwise)
    (,(keleusma--word-regexp keleusma-word-operator-keywords) . font-lock-keyword-face)
    ;; Boolean literals
    (,(keleusma--word-regexp keleusma-boolean-keywords) . font-lock-constant-face)
    ;; Primitive types
    (,(keleusma--word-regexp keleusma-primitive-types) . font-lock-type-face)
    ;; Numeric literals: float, hex, binary, integer. Type suffixes:
    ;; fractional literals take `Float` or `Fixed<N>`; integer
    ;; literals take `Word`, `Byte`, `Float`, or `Fixed<N>`.
    ("\\b[0-9][0-9_]*\\.[0-9][0-9_]*\\(Float\\|Fixed<[0-9]+>\\)?" . font-lock-constant-face)
    ("\\b0x[0-9a-fA-F_]+\\b" . font-lock-constant-face)
    ("\\b0b[01_]+\\b" . font-lock-constant-face)
    ("\\b[0-9][0-9_]*\\(Word\\|Byte\\|Float\\|Fixed<[0-9]+>\\)?" . font-lock-constant-face)
    ;; User-defined type identifiers (uppercase initial). Matched after
    ;; the specific keyword and primitive-type rules above so those win.
    ("\\<[A-Z][A-Za-z0-9_]*\\>" . font-lock-type-face)
    ;; Function calls: lowercase identifier immediately followed by `(`
    ("\\<\\([a-z_][a-zA-Z0-9_]*\\)\\>(?=\\s-*(" 1 font-lock-function-name-face)
    )
  "Font-lock specification for `keleusma-mode'.")

;;;###autoload
(define-derived-mode keleusma-mode prog-mode "Keleusma"
  "Major mode for editing Keleusma source code.

\\{keleusma-mode-map}"
  :syntax-table keleusma-mode-syntax-table
  (setq-local font-lock-defaults '(keleusma-font-lock-keywords))
  (setq-local comment-start "// ")
  (setq-local comment-end "")
  (setq-local comment-start-skip "//+ *")
  (setq-local indent-tabs-mode nil)
  (setq-local tab-width 4))

;;;###autoload
(add-to-list 'auto-mode-alist '("\\.kel\\'" . keleusma-mode))

(provide 'keleusma-mode)

;;; keleusma-mode.el ends here
