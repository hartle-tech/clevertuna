# Safety, and what "verified" means

## "Verified" means verified

A command that returned zero is not proof that anything happened. Every write
walks the same ladder, and each rung is reportable:

```
validated → sent → acknowledged → read back → compared → verified
```

**Acknowledged** only means the keyboard liked the request. **Verified** means
Clevertuna asked again afterwards and the answer matched what you asked for. If
it does not match, that is a `mismatch` — a first-class result with expected and
actual values, not a success with a shrug.

| Exit | Meaning |
|---:|---|
| 0 | read completed, or write verified |
| 2 | usage or validation error |
| 3 | no device found |
| 4 | transport or protocol failure |
| 5 | write accepted but readback differs |
| 6 | backup file rejected |

Comparison is deliberately limited to what your scheme actually named. The
keyboard fills in fields you left out, and reporting those as a mismatch would
be noise.

## Safety

- `set-backlight` rewrites **only** the backlight. Gestures, touch zones, key
  mappings and any field this tool does not model are carried through
  byte-for-byte, so a colour change cannot quietly rewrite something else.
- `import` is a full restore and is broader. It validates the file, then asks.
- Schemes are validated **before** the device is opened: ranges, marker counts,
  one-effect-per-zone, and schema version.
- Serial numbers and similar identifiers are hidden unless you pass
  `--show-identifiers`, so a pasted terminal is safe by default.
- Nothing phones home. There is no update check, no analytics, no network code.

## Device behaviours that will cost you an afternoon

Three things the firmware does that look like bugs in your client:

- It **rejects requests sent back-to-back** with `UNSUPPORTED_REQUEST` even when
  the bytes are perfectly valid — it is timing, not size, and a one-byte edit
  fails just as readily as a large one.
- It **accepts its own settings back verbatim**, which makes a no-op round trip
  a safe way to test a client.
- Over Bluetooth **the acknowledgement is the fragile half** — it sometimes does
  not arrive, or arrives as the answer to the previous request. A lost
  acknowledgement is not a failed write, so Clevertuna settles the question by
  reading the keyboard back rather than by believing the reply.

The full wire format is in [PROTOCOL.md](PROTOCOL.md).
