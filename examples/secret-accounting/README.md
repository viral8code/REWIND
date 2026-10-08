# Sensitive registry retention

`rewind profile main.rw --history-memory 256KiB --native-work 10000000` prints two
redacted values. Duplicate registrations use one retained text pattern. `revert`
and `revert begin` reset VM values while execution-wide redaction roots remain.
The synthetic string in this example is not a credential.

The v1.9.51 profile reports counts and admitted text/entry bytes without printing
patterns. Retained binary payloads share the normal byte-owner ledger. These
security roots are kept because ordinary derived values, observations and host
receipts may still contain the data. They are not automatically forgotten by GC.
