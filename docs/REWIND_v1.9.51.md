# REWIND v1.9.51 — Sensitive registry admission

Execution-wide redaction patterns remain strong security roots after revert,
begin, checkpoint drop and GC: derived ordinary values and retained observations
can still contain them. This increment charges their retained storage instead of
forgetting patterns to fit a budget.

Selected language 1.9.51 includes each distinct text pattern's retained String
capacity plus 96 bytes of conservative entry/node metadata. A cached monotone
sum avoids scanning all patterns at each admission check. Duplicate contents do
not expand this registry. Secret-input import uses the same insertion path.

Binary pattern buffers are registered with the existing Runtime-scoped weak
byte-owner ledger, with 96 bytes per registry entry. The registry itself remains
a strong owner, so an ordinary root disappearing does not prematurely remove
its charge. An already registered shared Vec is not charged again as a second
payload. The existing ledger's payload capacity and metadata fees still apply.
Previous selected-language memory contracts remain unchanged.

VM secret/reveal registration checks admission before returning or staging later
output. A fatal budget failure leaves its patterns registered so the diagnostic
can still be masked. This is fatal resource admission, not a typed Secret error;
recovering after a fatal failure follows the normal execution/checkpoint contract.
Native embedders enable the mode with `enable_sensitive_accounting` and use
`check_sensitive_accounting` after registration; the existing void registration
API remains available. Enabling the mode counts already registered text patterns
and registers existing binary owners.

The profile's `sensitive_registry` field reports text pattern count, admitted text
bytes, binary count and registry-entry bytes. Binary payload bytes belong to the
shared-owner ledger. It reports no pattern contents and is absent for earlier
selected languages.

Native unique/duplicate/cycle/checkpoint/shared-owner tests, CLI rejection before
publication, old-language compatibility, source-free replay and both extracted
SDKs must pass before publication. This change covers the central registry;
operation-specific worker pattern snapshots and supplied-input/observation
storage have their own lifetime/admission contracts and require separate audit.
This increment does not assert v2.0 completion.
