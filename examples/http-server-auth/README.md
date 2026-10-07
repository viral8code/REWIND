# Authenticated HTTPS server

Replace `PORT` with an available port. Place the public PEM certificate chain
in `server.pem`. Supply its matching PEM key through `REWIND_HTTP_SERVER_KEY`
and a high-entropy bearer token through `REWIND_HTTP_SERVER_TOKEN`.

```sh
rewind compile main.rw --allow-effects external,network,tasks,env,fileRead
rewind run main.rwc --allow-effects external,network,tasks,env,fileRead --secret-env REWIND_HTTP_SERVER_KEY --secret-env REWIND_HTTP_SERVER_TOKEN
```

Use a client trusting the certificate. `POST /echo?q=one` with body `ping` and
`Authorization: Bearer <token>` returns `ok`. An authenticated `GET /shutdown`
then closes the listener. Missing or wrong authentication returns 401; it does
not reach REWIND's request handler. The accepted authorization header is removed
from the request's public fields. Known private values in the body, URI or other
headers are still rejected.

Keys and tokens are supplied at runtime and stay out of artifacts and traces.
Do not distribute production credentials with an application. The SDK test
uses the repository's documented public test certificate and a test-only token.
Source-free runs still need the public certificate file and private environment
values. Disconnected replay uses the public file observation and requires the
matching key; a different valid token can be supplied for replay.

Physical responses and sockets are not restored by revert. The response receipt
in this example is restored without resending. Authentication is a shared token,
not a user/role or session system; see the v1.9.30 contract for capacities and
rotation.
