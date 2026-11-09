# OS-backed master-key storage — 2026-11-08

Parent: [Switchboard configuration restoration](switchboard-configuration-restoration-spec-2026-11-08.md)

**Status: implementation-ready security correction; not implemented or accepted.**

## Decision

Remove the plaintext `<config-stem>.master-key.json` companion file entirely.

Reliquary keeps one random 256-bit master encryption key per configuration. That key encrypts all `credential.<id>` objects in `reliquary.cfg` using the existing AES-256-GCM credential codec, with fresh nonces and logical-object AAD exactly as today.

The master key itself is stored only in the platform credential service:

- Windows: Windows Credential Manager through the native keyring backend.
- macOS: Keychain.
- Linux: persistent Secret Service/keyring backend.

There is no file fallback in the normal product path.

## Goals

1. Keep the existing `CVCFG\0\r\n` configuration format and encrypted `credential.*` object schemas unchanged.
2. Eliminate plaintext master-key material from the filesystem.
3. Keep Reliquary cross-platform.
4. Preserve one master key per config rather than one key per credential.
5. Keep config inspection/readiness operations secret-free and non-mutating.
6. Make failure to access the OS credential store explicit and typed rather than silently weakening storage.

## Non-goals

- No new config format.
- No wrapping-key sidecar.
- No DPAPI-only or Windows-only implementation.
- No environment-variable fallback for normal use.
- No automatic cloud synchronization of keys.
- No per-credential encryption keys.
- No change to provider credentials, route selection, OAuth refresh semantics, or the AES-GCM credential payload format.

## Key identity

Switchboard owns the master-key lookup identity.

Use a stable service name such as `reliquary.master-key.v1`.

The account/key identifier must be deterministic for the config and must not expose secrets. Derive it from a canonicalized config identity rather than the filename alone, for example:

`sha256("reliquary/master-key/v1\0" || canonical_config_path_utf8)`

Acceptance requires:

- two distinct config paths do not share a master key;
- reopening the same config resolves the same key;
- moving/copying a config does not silently bind it to an unrelated key;
- the lookup identifier is bounded and safe for all supported platform stores.

If portable moves are later required, they must use an explicit export/import or re-key workflow.

## Lifecycle

### Fresh config with no credentials

Creating, showing, verifying, or editing non-credential settings does not create a master key.

### First credential write

When the first credential is saved:

1. Resolve the OS keyring entry for this config.
2. If absent, generate 32 random bytes with the existing secure RNG.
3. Persist that key to the OS credential store.
4. Encrypt the credential object with AES-256-GCM.
5. Atomically save `reliquary.cfg`.

If OS-key persistence fails, the credential mutation fails and no encrypted credential is committed.

### Existing credential read

When `reliquary.cfg` contains any `credential.*` object:

1. Resolve the config's master key from the OS credential store.
2. If missing/unavailable, return a typed credential/master-key error.
3. Decrypt and authenticate credentials as today.

No fallback key may be created for an existing encrypted config.

### Credential removal

Removing one credential does not rotate or delete the master key.

When the last credential is removed, the master key may remain in the OS credential store. Automatic deletion is not required for this phase.

## Existing installations

The current plaintext companion-key format is retired through a one-way compatibility migration:

1. If `reliquary.cfg` contains encrypted credentials and the OS key is absent, detect the legacy companion file.
2. Read and validate the legacy 256-bit key once.
3. Store that exact key in the OS credential service.
4. Reopen/decrypt the existing credential objects using the OS-backed key.
5. Delete the legacy companion file only after successful OS-key persistence and successful credential authentication.
6. Never rewrite credential ciphertext solely for this migration.

If migration cannot complete, leave the legacy file untouched and fail with a clear migration error. Do not silently continue indefinitely on the legacy backend.

Legacy-reader removal is a later separately accepted cleanup.

## Platform abstraction

Switchboard owns a small master-key-store seam, conceptually:

- `load(config_identity) -> Missing | Key | Error`
- `store_new(config_identity, key) -> Result`
- optional `delete(config_identity) -> Result`

Production uses the platform credential store. Tests use an isolated in-memory/mock backend.

The current `keyring` crate is an acceptable implementation dependency if configured for:

- Windows native,
- macOS native,
- persistent Linux Secret Service/keyring.

Tests must not depend on the developer or CI runner's real credential store.

## Error semantics

Distinguish at least:

- master key missing for encrypted config;
- OS credential store unavailable;
- OS credential store rejected write;
- invalid stored key material;
- credential authentication/decryption failure;
- legacy migration failed.

Errors must not include master-key bytes, credential values/tokens, encrypted payload contents, or provider secrets.

## Security invariants

- Master key never appears in `reliquary.cfg`.
- Master key is never written to argv, stdout/stderr, logs, Debug output, environment variables, JSON sidecars, tests, or documentation examples.
- Secret plaintext buffers remain zeroized where currently supported.
- Each credential encryption keeps a fresh nonce.
- Existing AAD binding to `credential.<id>` remains.
- Copying only `reliquary.cfg` is insufficient to decrypt credentials on another account/machine.
- Failure of the OS key store fails closed.

## Public/CLI behavior

Existing configuration commands remain unchanged:

- `config show`
- `config verify`
- `config credential list`
- `config credential add-api-key`
- `config credential login-codex`
- `config credential refresh-codex`
- `config credential remove`

No master-key management is exposed in the ordinary CLI for this phase.

User-facing documentation should state that credentials are encrypted in `reliquary.cfg` and their master key is stored in the operating system credential service.

## Acceptance

### Switchboard

- encrypted credential object round-trip remains schema-compatible;
- fresh key creation stores only in the mock/OS key store, never the filesystem;
- distinct configs receive distinct keys;
- reopen resolves the same key;
- missing key for an encrypted config fails typed;
- wrong/tampered key fails authenticated decryption;
- unknown config objects remain byte-preserved;
- read-only operations do not create keys;
- failed key-store writes do not commit credential ciphertext.

### Legacy migration

- a real legacy config + companion key migrates the exact key into the store;
- credential ciphertext is unchanged;
- companion file is deleted only after verified migration;
- interrupted/failed migration leaves the legacy file intact;
- reopening after migration no longer reads the companion file.

### Platform

- Windows native backend verified.
- macOS native backend covered by build/test configuration.
- Linux persistent backend covered by build/test configuration.
- CI/unit tests use a mock backend and never modify the runner's real credential store.

### Regression

- configuration CLI tests remain green;
- device login/refresh remains green;
- provider binding/readiness remains green;
- Root/external consumer tests remain green;
- no `*.master-key.json` creation remains in production code;
- documentation no longer describes the plaintext companion file as the canonical security boundary.

## Implementation sequence

1. Introduce the Switchboard master-key-store seam and mock backend.
2. Add OS-backed production implementation.
3. Replace companion-file load/create calls.
4. Add one-way legacy companion-key migration.
5. Update tests to the mock backend and add migration/platform coverage.
6. Update configuration/security documentation.
7. Run full config/credential/runtime regression gates.
8. Remove stale `reliquary.master-key.json` ignore/documentation assumptions once migration behavior is verified.

## Completion gate

This correction is complete only when a fresh credential-bearing config contains no recoverable master-key material anywhere in Reliquary-managed files, the same config can reopen through the OS credential service, legacy configs migrate without re-encryption or loss, and all supported platforms retain a viable secure backend.
