# M4 design note — HTTP admission, refusal and session lifetime (ruling O-15)

Status: decided before implementation (M4 C4.1 round 2, ruling O-15), committed before `crates/mcp/src/http.rs`
changes (commit `5b9ccf8`). Two mechanisms were corrected while implementing it, without changing an invariant: the
eviction signal (§4: a flag, not a second socket handle) and the client check of a quiet stream (§6). Owner: builder. Findings answered: C41-R2-01 (idle unauthenticated sockets shut out authenticated callers),
C41-R2-02 (refusals escape the connection cap), C41-R2-03 (a session expires while its own request body arrives).
Spec and decisions: 11 §4 (Streamable HTTP), ADR-0022 decision 2 (resource bounds, O-6, O-7).

This is the third pass over the same mechanism. Each earlier pass bounded one state and left another outside every
bound. This note therefore lists every state a socket can be in and names the bound that owns it, so that no state is
outside one; each numbered invariant (I-1 to I-8) has a builder test of its own (§9).

## 1. Limits

| Limit | Default | Owns |
|---|---|---|
| `max_connections` (C) | 64 | connection workers: one thread and one socket each, from admission to hand-off |
| `max_unauthenticated` (U) | 16 | the pending pool: workers whose request head is not complete yet (U is used as `min(U, C)`) |
| `max_lingering` (L) | 128 | sockets being closed gracefully by the closer thread after their answer |
| `MAX_SESSIONS` (S) | 32 | sessions, each with one protocol-loop thread |
| `head_deadline` | 10 s | a pending worker's whole life, from the moment its socket was accepted |
| `request_deadline` | 60 s | the head and the body, from the moment the socket was accepted |
| linger | 256 KiB, 1 s | one lingering socket |
| `session_idle` | 30 min | a session with no request in flight |

## 2. The life cycle of a socket

A socket is in exactly one of these states. The table names who holds it and which bound counts it.

| State | Held by | Counted by | Leaves by |
|---|---|---|---|
| **accepted** | the accept thread (one socket at a time) | none needed: one at a time, decided without reading or waiting | admission (→ pending), refusal (→ lingering) |
| **pending** (head being read) | its worker thread | C and U | head complete (→ authenticated or refused), head deadline (→ refused, `408`), eviction (→ closed), client close (→ closed) |
| **authenticated** (head parsed, token valid) | its worker | C | method dispatch below; a refusal (`404`, `405`, `413`, ...) → lingering |
| **body being read** (`POST`) | its worker | C; its session's in-flight guard (§6) | body complete (→ dispatched), request deadline (→ refused, `408`) |
| **dispatched** (message sent to the session) | its worker | C; in-flight guard | `202` written (→ lingering), or the SSE answer begins (→ streaming) |
| **streaming** (SSE answer or `GET` stream) | its worker | C; in-flight guard | the response written, the subscriber gone or not reading, the session ended (→ lingering) |
| **refused** (an answer written before the request was complete) | the thread that wrote it: the accept thread (`503`) or the worker | as the state it came from, for the time of one non-blocking write | → lingering, at once |
| **lingering** | the closer thread | L | client close, 256 KiB discarded, 1 s, or L exceeded (the oldest is closed) (→ closed) |
| **closed** | nobody | nothing | — |

Every transition into **lingering** is a hand-off: the worker (or the accept thread) gives the socket to the closer
through a channel and the worker thread ends (or the accept thread goes back to accepting). Waiting for a client to
read an answer and close is the closer's job only.

## 3. Bounds on threads and sockets (I-1)

At any moment the server holds at most:

- **threads:** `3 + C + S` (the accept thread, the session sweeper, the closer; at most C connection workers; at most
  S session threads). No other thread is ever created by the server. A worker is created only after its slot in C (and
  in U) was taken, by the accept thread, so the count of workers never exceeds C even for an instant; a worker whose
  slot was taken but whose thread could not be created gives the slot back and its socket is refused.
- **sockets:** `1 + 1 + C + L` (the listener, the one socket the accept thread is deciding about, the workers'
  sockets, the lingering sockets). An evicted pending worker still owns its slot in C until its thread has ended, so a
  socket is never outside C while a thread still touches it.

There is no state outside these: a refusal by the accept thread creates no thread (§5), a refusal by a worker hands
its socket to the closer and the worker ends, and the closer's set is capped at L.

## 4. A full pending pool (I-2, I-3)

A new connection cannot be told apart from an idle one before its head is read, and its head cannot be read without
admitting it. So when the pending pool is full, the accept thread:

1. answers the new connection `503 Service Unavailable` with `Retry-After: 0` (a non-blocking write, nothing read) and
   hands it to the closer; and
2. **evicts the pending connection that has waited longest for its head**: it is removed from the pool under the pool
   lock and its eviction flag is set. Its worker reads the head in slices of 50 ms and checks the flag between them,
   so it ends within one slice and closes the socket without an answer. The pool has room again immediately (the
   evicted worker's slot in C is freed when its thread ends, at most 50 ms later). No second handle on a pending socket
   is kept for this: on Windows, closing a duplicated socket handle resets the connection, which broke ordinary
   answers in the first implementation of this note.

So no connection can hold a pending slot past the next refusal: a pending connection lives until its head is complete,
its head deadline, or the moment a newcomer needs room and it is the oldest. Idle sockets, however many and however
replenished at the head-deadline rate (C41-R2-01), cannot keep a slot: a token holder that meets a full pool is told to
retry at once, and its retry finds room, because its own refusal made it.

Why the newcomer is refused rather than admitted into the slot it frees: the newcomer is itself unclassified (it may
be one more idle socket), so it is not given precedence over a connection that may be about to complete its head; the
refusal is free (no thread, no read, no wait) and is visible to the client that met the full pool, which retries.
Evicting the oldest bounds how long any connection without a complete head can hold a slot to the time between two
newcomers.

**What this does not promise.** A local process without the token that opens a new connection between every refusal
and the retry it causes (an active connection flood, not idle holding) can keep making a retry meet a full pool; no
loopback server can guarantee service against a local flood, which can equally exhaust the CPU or the ephemeral ports.
The bound in that case is the one of §3: the flood costs the server no thread beyond C, no socket beyond C + L and no
memory beyond those sockets' fixed buffers. ADR-0022's threat model (one user, one installation; the token protects
the session's powers) is unchanged.

**Authenticated connections are never evicted.** Only pending connections (head incomplete) are eviction candidates.
When C is full of authenticated connections (streams of token holders), a newcomer is refused `503` without eviction;
that is the token holder's own load.

## 5. Refusals without a thread or a wait (I-4)

- **By the accept thread (`503`, pool or C full):** the socket is switched to non-blocking, the short answer is written
  once (it fits the socket's send buffer; if it does not, it is dropped), and the socket is handed to the closer. The
  accept thread never reads, never waits and creates no thread for a refusal.
- **By a worker (`400`, `401`, `403`, `404`, `405`, `408`, `413`, `431`, `503` for the session cap, ...):** the answer
  is written within the worker's write timeout; when the whole request was read, the worker sends its FIN
  (`shutdown(Write)`); then it hands the socket to the closer and ends, freeing its slot in C.
- **The closer** holds at most L sockets, non-blocking, and discards their input through one fixed buffer until the
  client closes, 256 KiB were discarded or 1 s passed; then it closes the socket. When a hand-off would exceed L, the
  oldest lingering socket is closed first. With no lingering socket it waits on its channel and uses no CPU. Discarding
  the remaining input is what keeps an early refusal from being lost to a reset (C41-R1-05); a client that shuts down
  its socket after reading the refusal finds it still connected, because the FIN is sent only when the whole request
  was read.

## 6. The in-flight guard of a session (I-5, I-6, I-7)

- A session records `in_flight` (the requests and streams it has now) and `last_seen`.
- **Taken** under the session-map lock at the moment the head of a request naming the session is authenticated, before
  a single body byte is read: the session must exist (else `404`), `in_flight` goes up and `last_seen` is set. For
  `initialize`, the guard is taken in the same critical section that checks the session cap and inserts the new
  session (O-7), so a new session is never visible without its first request in flight.
- **Released** by the guard's `Drop`, under the map lock: `in_flight` goes down and `last_seen` is set to now, so the
  idle time starts when the last request ended. `Drop` runs on every return path: an answer written, a refusal, a body
  deadline, a client that went away mid-body or mid-stream (the read or write fails and the handler returns), and a
  panic in the handler (unwinding drops the guard). The guard is held until the answer, or the SSE stream, is fully
  written. A quiet stream checks every second that its client is still there (a non-blocking peek sees the end or an
  error), so a client that went away releases the guard within a second, not at the next 15-second keep-alive.
- **Expiry** removes a session only when `in_flight == 0` and `last_seen` is older than `session_idle`, under the map
  lock. A session with an admitted request is therefore never expired, from the moment its head is accepted until its
  answer is written (C41-R2-03).
- **`DELETE`** ends a session whatever its requests: the client asked. The session leaves the map; a guard released
  later finds no session and does nothing. The session's protocol loop then ends (its input closes), and its routes are
  cleared, so every stream of that session ends at once instead of waiting for a response that will not come.
- **Session cap:** a session counts against S from insertion until it leaves the map (`DELETE`, expiry, the end of its
  protocol loop, server stop); in-flight requests do not change that.
- **Server stop:** the accept loop ends; every session leaves the map (their loops end and their streams end as for
  `DELETE`); every pending connection is evicted; the closer closes every lingering socket. Workers still writing an
  answer end within their write timeout; no new worker is created.

## 7. Lock order (I-8)

There are three locks: the **pending pool** (`pending`), the **session map** (`sessions`) and each session's
**routes** (`routes`).

- `sessions` may be held while taking a session's `routes` (expiry reads a session's open routes; `GET` installs its
  stream). Never the reverse: code that holds `routes` takes no other lock.
- `pending` is never held together with `sessions` or `routes`; it is a leaf lock taken only by the accept thread (to
  admit and evict) and by a worker leaving the pending state.
- The closer's channel and the slot counters are lock-free.

## 8. What changes in the code

- Admission moves into the accept thread, before any thread is created; a worker is created only for an admitted
  socket (C41-R2-02).
- The pending pool becomes a registry of the pending connections (an eviction flag and the accept time of each) so
  that the oldest can be evicted (C41-R2-01).
- Lingering moves from the worker threads to one closer thread with its cap L; refusals from the accept thread are
  written there without a thread (C41-R2-02).
- The session gains `in_flight` and an RAII guard; expiry checks `in_flight`; `DELETE`, the end of a session and stop
  clear its routes (C41-R2-03). A quiet SSE stream checks its client every second.
- `HttpServer::stats()` reports the workers, pending connections, lingering sockets and sessions, so the tests check
  the bounds of §3 directly.

## 9. Tests

The three reviewer tests (`crates/mcp/tests/c41_r2_review.rs`) pass with their bodies unchanged. Builder tests, one
per invariant, in `crates/mcp/tests/c41/transport.rs`:

| Invariant | Builder test |
|---|---|
| I-1 threads and sockets within `3 + C + S` and `2 + C + L` | `admission_bounds_hold_under_idle_and_refused_bursts` |
| I-2 a full pending pool evicts its oldest connection, not a younger one | `a_full_pending_pool_evicts_the_connection_waiting_longest` |
| I-3 a token holder that meets a full pool is served on its retry, idle sockets replenished or not | `a_token_holder_is_served_on_retry_while_idle_sockets_are_replenished` |
| I-4 refusals create no worker; lingering sockets are capped at L, oldest closed first | `refusals_create_no_worker_and_lingering_is_capped` |
| I-5 a session with a request in flight (body or stream) is not expired; its idle time starts when it ends | `a_streaming_request_keeps_its_session_and_idle_time_starts_at_its_end` |
| I-6 the guard is released when the client goes away mid-body | `a_client_that_leaves_mid_body_releases_the_guard` |
| I-7 `DELETE` and stop end a session's open streams | `delete_and_stop_end_open_streams` |
| I-8 lock order | by construction (§7): `pending` is a leaf; `routes` takes no other lock; reviewed in the code, not testable by a run |
