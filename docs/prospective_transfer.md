# Prospective native transfer export

`Wallet::prepare_prospective_transfer` builds an opaque, unsigned candidate from a PSBT clone
using the wallet's existing RGB store and normal runtime lock. It resolves every Bitcoin input
through a caller-supplied independent transaction resolver. The returned transaction must match
its requested txid/vout and the PSBT's amount/script; PSBT input metadata is never authoritative.
The resolver is responsible for authenticating its source and checking the network. Sync the
wallet first; these methods do not perform network sync.

Inspect `candidate.psbt()` and `candidate.fascia()` before obtaining explicit application consent.
Persist `ProspectiveApproval` (serializable) with the exact final PSBT bytes, exact fascia JSON,
and all principal/change/maker-fee/taker-fee allocations. Vouts refer to the FINAL colored
transaction, including a commitment output inserted by coloring. Approval must be generated
from agreed terms and independently established recipient scripts, rather than automatically
accepting the candidate. There is no maker commercial policy in this library. Omit zero fee
legs; zero amount allocations are refused. An approved change output must belong to this wallet;
other legs can additionally require local derivation ownership.

`Wallet::export_prospective_transfer` checks this complete approval, unsigned state and exact
transaction/transition/contract/vout/script/amount bindings. It re-resolves every input and
requires the transition inputs to equal currently locally unspent RGB stock inputs. All input
asset value must be explicitly allocated. It exports every approved recipient together through
the pinned engine's `transfer_from_fascia`, while holding the normal wallet lock. Each contract
must have one revealed fungible transfer transition with an approved principal. Concealed,
non-fungible, rights, multiple-transition, signed, missing/extra/duplicate and stale candidates
are refused. A failed export returns no partial transfer vector.

The candidate is intentionally opaque and not deserializable. Persisted approval alone cannot
reconstruct a candidate after restart; prepare and approve again. This API does not provide a
restart consent loader, receive registry, coordinator, signing gate or broadcast service. Those
consumer behaviours still need integration and validation.

No second concurrent Stock/FsBinStore instance, fascia consumption, stock flush/store, transfer
record, backup update or signing occurs in either method. An existing RGB directory and valid
stash/state/index are required. The read-only loader avoids initialization, disables engine
autosave and runtime persistence, and explicitly releases the lock on both success and refusal.
Unlock failures are returned; emergency unwinding performs best-effort lock removal without
stock writes. Existing mutation callers retain their original persistence/drop policy.

The shared coloring implementation preserves PSBT input/output metadata when inserting an
OP_RETURN, shifts vouts only for a newly prepended host, and rejects vout equal to output count.
The legacy consuming method remains separate. See the evidence map for the precise limits of
executed tests; this source candidate is not release acceptance.
