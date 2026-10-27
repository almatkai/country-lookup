# Reproduce review checks

Run from `v2/`. QA uses only clone-local disposable directories. Product runtime never requires Node/Python/npm. No real API key is needed or accepted for tests.

```sh
python3 scripts/prepare-qa.py
node qa/source-oracle.mjs
python3 scripts/host-seams.py
cargo fmt --all
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo +1.85.0 test --locked
cargo +1.85.0 clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
cargo +1.85.0 fmt --all -- --check
cargo build --locked
python3 qa/protocol-smoke.py
node qa/renderer-schema.mjs
python3 scripts/isolated-build.py
python3 scripts/licenses.py
```

`prepare-qa.py` recreates a disposable exact Git-object V1 copy and installs its exact package lock with scripts disabled; it runs original smoke tests/typecheck without baseline fixes. It separately installs the pinned renderer/schema QA dependencies under `qa/`. It never invokes `ray`, Bun or a source producer on real paths. `source-oracle.mjs` captures the full own TSX/helpers, executable locked countries library, actual source-mapped utils cache/promise/cached-state logic, React/API/cache/crypto in-memory stubs before evaluation; uncaptured imports, source fs/os/child_process/http access and source process access are denied. A deliberately executed source-fs import regression must throw before any write. Country requests are synthetic fetch captures with exact URL/query/header assertions and no real fetch fallback.

The 21 source cases compare complete text, raw JSON, names, all keywords and ordered metadata; native action tests compare source action order and exact copy/open payloads. Source index execution tests trim-only filtering/BOM/NEL, Map last-write use, persisted toggle, three speculative pages, >300 records, successful cache hit/expiry, ignored out-of-bounds errors, later-page failure and usePromise's prior-data removal. Native tests add visible finite parsing/pagination/transport/security/storage limits. This is finite evidence, not exhaustive arbitrary-JS or live-data parity.

Rust tests use the real production Adapter and pinned SDK. Local HTTP checks exact bearer shape without outputting a credential, offsets/order/stripping, repeated pages, metadata/record/body bounds, sanitized statuses/redirects, deadlines and deliberate refresh retries. A tracked Python TLS server on loopback and freshly generated temporary mock certificate verify rejection of an untrusted certificate. No real API/OS producer is called. A staged-state regression discards successful Copy/Open/search/fetch results just before SDK commit and verifies reconciliation, visible uncertainty and exact deliberate retry. A separate production Adapter test checks other-key/profile rejection, four-hour expiry/future dates and new-Adapter persisted-cache reuse. Storage tests cover unknown-key preservation, concurrent toggles, typed corruption and byte bounds. A real SDK duplex test cancels an open during three in-flight mock HTTP requests and checks registration cleanup without adapter disposal.

`protocol-smoke.py` launches the actual binary over real OS stdin/stdout pipes. It seeds only clone-private synthetic cache, checks hello/init/open/call/search/cancel/dispose/shutdown/EOF, exact uncertain retries and contradictory/late ACK handling, stale/cross-session IDs, query replacement, persistence across restart, 140 canceled opens, bad LF/JSON/EOF/oversized framing and corruption failures. Its only reverse RPC behavior is capture/respond: clipboard, browser/settings producers are never called. Product endpoint blocks the public synthetic test prefix as an additional no-network guard.

`renderer-schema.mjs` executes actual immutable List/Detail/Metadata/ActionOverlay/Markdown code with controlled hooks and genuine locked host Markdown dependencies in JSDOM (no network/resources/scripts). It verifies split detail, metadata/links, flag URL without Raycast suffix, row click/Enter/action menu, Back, server-search bypass, sanitized/literal markup, schema positives/negatives and Detail primary-click before/after. It is not a native browser rendering/layout/theme acceptance test. Registry/runtime validator tests use actual extracted functions and explicitly documented stubs rather than building the host.

`isolated-build.py` copies only Cargo manifests/config/src to a new ignored clone-local directory, uses a fresh Cargo home, fetches the immutable SDK over HTTPS, builds independently and runs real production pipe smoke on that binary. `reports/isolated-build.json` records input hashes and executable digest. No sibling/host checkout or profile is an input.

Use the binary path recorded in that report to package:

```sh
python3 scripts/package.py /absolute/path/to/the/isolated/binary
```

Packaging never installs. It uses fixed ZIP metadata and ordered uncompressed entries; repeated byte equality, full inventory/hashes, native ARM64 Mach-O header, exact PNG, extracted probes and pipe smoke are mandatory. `reports/package-inventory.json` contains the measured development ZIP/executable digests; ZIPs live under ignored `dist/` and must not be staged.

`reports/windows-gnu.txt` records the missing GNU C compiler blocker. No toolchain/compiler was installed. Windows/MSVC, live GUI, real clipboard/browser/settings, Linux and live authenticated API are not-run. `ci/native-build.yml` is inactive review material, not executed evidence.

After all final source/report edits, run `python3 scripts/index.py`. It verifies V1 bytes (only the short README append permitted), enumerates Git-visible untracked deliverables without staging them, checks ignored generated trees explicitly, and writes INDEX excluding itself. Parent must independently verify reviewed/staged bytes. The fresh-clone initial Git inspection was briefly attempted while clone checkout was still running (index.lock); no files were changed, the clone was allowed to finish, then clean status/root/worktrees/fetch/branch were reverified. An accidental QA npm command initially reached the root package; its generated node_modules were moved intact into ignored QA storage and original package-lock bytes restored from the immutable Git object before any implementation change. Final V1 checks enforce the correction.
