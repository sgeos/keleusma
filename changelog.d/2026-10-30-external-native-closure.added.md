- **`Vm::register_external_native_with_ctx_closure`.** An external native can be a closure that
  receives the native context, as a verified native already can, for a host whose natives are
  values of its own language. Pinned by `tests/external_native_closure.rs`. A pykeleusma
  request.
