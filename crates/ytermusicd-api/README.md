# ytermusicd-api

Shared request/response and SSE DTOs for the daemon API.

The Rust types derive `serde` and `ts-rs`. Running the crate tests exports the
TypeScript bindings into [`bindings/`](bindings/):

```sh
cargo test -p ytermusicd-api
```

CI regenerates them during `cargo test --workspace` and fails if the checked-in
bindings differ. The web crate (#14) will import/copy these files.

Do not hand-edit the generated `.ts` files; change the Rust types and rerun the
tests.
