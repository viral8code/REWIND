# HTTP server example

Replace `PORT` in `main.rw` with an unused TCP port, then run:

```sh
rewind run main.rw --allow-effects external,network,tasks
```

Send `POST /echo?q=one` with the body `ping` to `http://127.0.0.1:PORT`.
The server returns `ok`. After reading that response, send `GET /shutdown`.
That connection closes without a response and the program prints `http server done`.
The second request keeps the listener alive until the first response has been read.

The example restores an already completed response future. The cached receipt is
returned; the physical reply is sent once. For ordinary applications, run a request
loop and explicitly choose when to close or drain connections.
