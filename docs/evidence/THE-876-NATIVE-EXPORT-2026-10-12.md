# THE-876 native export source candidate — 12 Oct 2026

Status: PARTIAL / NOT SAFE TO ACCEPT. Original task and downstream acceptance remain OPEN.

Repository: nerdynigel/rgb-lib; branch feat/the-876-transfer-export.
Registered projectWorkspaceId: 18c352ad-d2a6-499d-921c-f5ce06a00dd9.
Base: 95332c41fd715939ac6e078ad859d474b1f6fa9b.
Base tree: 87f030db4819ff5e9b263410885d49f3238c77fa.
Cwd: /home/nigel/.paperclip/instances/default/projects/3be34d74-08c3-4532-b8b0-2d8f5915387e/7d7a8235-c191-4627-a507-19f9ae24a8ed/rgb-lib/.worktrees/the-876-transfer-export.
Checkout was clean at the exact base. No parallel writer or default upstream tracking.

## Implementation and acceptance map

| Requirement | Delivered source / executed evidence | Remaining |
| --- | --- | --- |
| Wallet-owned synchronized non-consuming export | Public preparation/export methods in src/wallet/prospective.rs; reuse locked existing runtime and pinned transfer_from_fascia | Actual initialized wallet construction/export through these public callers unproven |
| Foreign amount/script authentication | Independent full previous-transaction resolver, txid/vout and exact PSBT data comparison; synthetic valid input and amount/script/txid/vout/duplicate refusals PASS | Actual authenticated indexer/Core resolver integration unproven; resolver source trust is caller's responsibility |
| Local ownership/current stock | BDK locally unspent membership and local recipient derivation; stock opout equality/full fungible conservation implemented | Actual local/foreign ownership and stale-stock export refusal through wallet callers unproven |
| Every recipient in same approved transaction/transition | Exact colored PSBT/fascia consent, complete revealed allocation map, principal per contract, unique roles; engine-built synthetic zero/maker-only/both-fee vectors PASS | Funded native consignments and recipient validation unproven |
| Refusal | Wrong recipient/asset/transition/vout/amount, missing/extra/duplicate/role, post-approval PSBT/fascia/signature and prevout negatives PASS | Actual public caller negative coverage and consumer signing gate unproven |
| Initialization/lock/drop/persistence/error effects | Separate isolated empty engine-store test: missing store refuses without RGB directory creation; lock exclusive; load/drop/release preserve all three bytes/mtimes; explicit unlock failure and construction refusal cleanup; corrupted stash preserved | Successful public construction/export byte/mtime measurement with actual contract unproven; interruption/flush/immediate crash accounting unproven |
| Shared coloring metadata | Pure affected boundary regression: witness data and unknown input/output metadata preserved; prepended/existing/appended host vouts checked | Existing funded color/consume regression withheld because it calls initialization/funding/issuance/signing services |
| Persisted consent/served bilateral integration | Serializable explicit approval; opaque candidate prevents caller mutation | Restart loader, actual served bilateral integration, durable recipient registry and browser/native acceptance remain consumer work |
| Probe pin/lock/provenance | No pin changed in this native source checkpoint | Software next owns isolated tools/l1-atomic-probe updates in the registered consumer workspace after immutable publication/Chief routing |

## Executed validation

Commands use /home/nigel/.cargo/bin/cargo (Rust/Cargo 1.98.1); no dependency pins changed.

- cargo check --locked --offline initially exit101 before compilation: confy absent from cache.
- cargo check --locked fetched the exact committed lock closure, then initially exposed one new
  iterator type error, corrected. Subsequent default check PASS.
- cargo check --locked --offline --no-default-features PASS at final candidate source.
- cargo fmt --all -- --check and git diff --check PASS.
- cargo test --locked --offline prospective --lib: 5 PASS, 0 failed. Synthetic engine transitions
  and MPC commitments only; no issued contract, wallet instance, funds or network.
- cargo test --locked --offline readonly_runtime --lib: 2 PASS, 0 failed. Isolated empty Stock
  fixture; not second live stock, funded wallet, actual export, interruption or crash proof.
- Earlier test compilation errors in synthetic fixture constructors were corrected before PASS.
- Final formatting/diff/check results and exact delivered SHA/PR are recorded in the issue receipt.

No financial profiles, browser SDK initialization, wallet issuance, signing, funding, Core,
indexer, runtime recipe release, old-case replay, WASM/default/native RLN/signer/production pin,
candidate freeze, force-push or merge. Existing engine5d9e811/browser SDK/recipe691de811
preparation evidence is reused from Chief/Reliability's published receipt2553569ff with
attribution; not rerun and not promoted into native acceptance.

## Next owner and rollback

Chief reviews this immutable source increment and routes SAME task to Software in the registered
consumer workspace for isolated probe pins and affected validation. Software retains all missing
public-caller ownership/recipient/export proof; independent Verification is an additional gate,
not a substitute. Runtime/funded proof requires separately authorized exact fixtures; no such
release is inferred here. All THE-870/THE-872/THE-873/THE-874 holds remain.

Rollback: disable consumer use of this additive API and normally revert only this branch's source
and documentation commits. No chain/profile/database migration or production pin rollback is
needed. Preserve existing profiles and historical evidence. Cargo target/cache artifacts are
reproducible and ignored; no other task's files were committed.
