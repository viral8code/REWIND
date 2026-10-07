# HTTPS server

Replace `PORT` in `main.rw` with an available port. Place the PEM certificate
chain in `server.pem`. Supply the matching PEM private key as the secret
environment value `REWIND_HTTP_SERVER_KEY`.

```sh
rewind compile main.rw --allow-effects external,network,tasks,env,fileRead
rewind run main.rwc --allow-effects external,network,tasks,env,fileRead --secret-env REWIND_HTTP_SERVER_KEY
```

Connect a client that trusts your certificate to `POST /echo?q=one` with body
`ping`. The server replies `ok`; `GET /shutdown` then closes the listener.
Never distribute a production private key with an SDK or application artifact.
The SDK smoke test uses the repository's documented test-only certificate.

The listener serves TLS 1.2/1.3 and HTTP/1.1. Its certificate is immutable for
each alias. TLS does not provide application authentication. Physical sockets
and already queued responses cannot be rolled back. Reverting an observed
reply receipt does not resend the response.

Recorded runs replay without opening a listener. Replay still requires a valid
private key matching the recorded certificate. The public
certificate file is supplied by the file observation journal. Private key
contents stay outside operation records, traces and compiled artifacts.
