- **`keleusma native`.** Native output is delegated to the detached `kel-native` command,
  found through `KELEUSMA_NATIVE`, beside `keleusma`, or on `PATH`, with every argument and
  the exit status passed through unchanged. A missing `kel-native` exits with status 69 and
  says how to install it. Pinned by `keleusma-cli/tests/native_delegation.rs`. On the
  operator's ruling of 2026-10-28.
