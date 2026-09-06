# Argon2id wrapper policy (roadmap Phase 2)

## New wrappers: documented fixed configuration

KeyNest retains Argon2id v1.3 (`Version::V0x13`) with:

- Memory: 65,536 KiB (64 MiB total, not per lane)
- Passes: 3
- Parallelism: 4 lanes (not a guarantee of four execution threads)
- Salt: 16 fresh OS-random bytes per wrapper
- Derived wrapping key: 32 bytes

These are the memory-constrained, second recommended settings in
[RFC 9106 section 4](https://www.rfc-editor.org/rfc/rfc9106.html#section-4).
The implementation specifies every parameter; it does not use library defaults.

Phase 2 deliberately uses the roadmap's fixed-configuration alternative. Adding
hardware calibration would introduce timing-dependent policy, startup work, and
additional persistence/failure paths without being needed to replace weak
defaults: the existing settings already match this published configuration.
The 250–500 ms calibration target is **not** a latency promise. Actual derivation
latency depends on hardware, build mode, and load. Setup and recovery involve
multiple sequential derivations; their total time is longer than a single KDF.

## Stored wrappers and compatibility

Every password/recovery wrapper already stores `memory_kib`, `iterations`, and
`parallelism` alongside its salt, nonce, and ciphertext. Argon2id v1.3 is the
existing interpretation of the supported v1/v2 profile formats. This phase does
not change the formats, KDF algorithm/version, salts, or ciphertext of any profile.

`ProfileStore` no longer accepts a current-default KDF argument. Loading must not
require stored parameters to equal the defaults used for newly created wrappers.
Unwrap uses the wrapper's own validated parameters. A deliberate password change
uses the current creation settings only for the new master wrapper; an unchanged
recovery wrapper keeps its original metadata. Defaults changing within the
supported envelope therefore do not invalidate existing wrappers.

Future changes outside this envelope or to Argon2 version/algorithm require an
explicit compatibility decision and tests; do not reinterpret existing data.

## Bounded read and derivation policy

Production accepts only the following envelope, independently of creation defaults:

- 65,536–131,072 KiB memory (maximum 128 MiB Argon2 working memory per derivation)
- 3–6 passes
- 1–4 lanes
- `memory_kib * iterations <= 393,216` (KiB-passes)

The last bound caps approximate memory-pass work at twice the current default;
it is a resource bound, not a cryptographic-strength or wall-clock formula.
Memory and pass floors retain at least the existing memory/pass costs. Small
bounded variation supports stronger future settings without coupling reads to
the newest defaults. This does not impose a hard process-RAM or execution-time
limit: allocator/library overhead and hardware behavior still apply.

Checks run on profile load, create, and replace, and again in the common key
derivation function **before** constructing/invoking Argon2. The arithmetic uses
`u64` to avoid overflow from `u32` metadata. Invalid types, missing parameters,
unknown KDF/wrapper fields, zero values, excessive values, and over-budget
combinations fail closed. Password and recovery wrappers use the same validation.
Unauthenticated UI errors remain the existing safe damaged-profile errors.

Unit tests alone accept the exact existing 32 KiB / 1 pass / 1 lane fixture.
That exception is behind `cfg(test)`, not a runtime flag or persisted setting.
Tests separately prove the production policy rejects that fixture.

## Verification

- A real production-cost setup persists both wrappers' explicit parameters.
- The profile remains readable/unlockable when creation defaults change from
  64 MiB / 3 passes / 4 lanes to 64 MiB / 4 passes / 4 lanes.
- Unlock leaves profile bytes unchanged; explicit password change retains the
  Vault Key and the recovery wrapper, and the new master wrapper uses new defaults.
- An unchanged recovery wrapper still unwraps using its own stored KDF values.
- Invalid metadata in either wrapper is rejected without modifying disk bytes.
- Direct wrap/unwrap/derive calls also reject out-of-policy costs.
- OS-random salts have the required size and differ across separate wrappers.

No frontend, calibration UI, KDF benchmark, or later-roadmap phase was implemented.
