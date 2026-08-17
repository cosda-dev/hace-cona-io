# hace-cona-io

**Version**: 0.1.0  
**Edition**: 2021  
**License**: Proprietary  
**Description**: CONA IO FEM module

---

## Canonical Layering

`hace-cona-io` is the **CONA IO FEM consumer/assembler**. It provides cognitive routing, protocol selection, ingress/egress orchestration, flow control, and adapters around the canonical IO transport layer.

The transport/interface DNA is owned by **`hace-io-rac`**. **RACE (`hace-me-race`)** is the execution/router layer above transport.

```text
Actor / Caller
      |
      | RAC URI + SIO
      v
RACE (resolve / route / execute / borrow)
      |
      v
CONA IO / FEM (consumer / assembler)
      |
      v
hace-io-rac (canonical transport DNA)
      |
      +-- CRI
      +-- LTI
      +-- RLI
      +-- A2A / BOX / NET / ON / CLI
```

### Contract separation

- **RAC URI** — routing identity and capability boundary (`WHO / WHERE / WHICH capability`). It must not expose implementation paths.
- **SIO** — invocation/data contract carrying URI, method, payload, headers, and context.
- **Transport** — byte movement and channel/framing/codec concerns; canonical owner is `hace-io-rac`.
- **RACE** — resolves and executes RAC operations and may borrow capacity from QIDE, RIPE, CLIE, or other MEs through RBP.

### Interface semantics

`CRI` is an interface family rather than one transport. `FDI`, `FPI`, and `FFI` have distinct semantics from local/remote transport boundaries such as `LTI` and `RLI`.

- `FPI` = architectural execution instance.
- `FFI` = low-level ABI mechanism.
- An FPI may use FFI, WASM ABI, HostCall, shared memory, or transport adapters.

### CONA responsibilities

- RAC cognitive routing
- protocol selection
- adapter/bridge assembly
- ingress/egress orchestration
- backpressure and idempotency handling
- SIO integration

### Explicit non-responsibilities

- canonical transport implementation
- authority binding
- direct bypass of `hace-io-rac`

---

## Existing Components

- **RAC** — Remote Actor Communication integration
- **Ingress/Egress** — data flow management
- **Backpressure** — flow control
- **Trading strategy** — strategy execution framework
- **Resolver** — IO resolution and routing

## Dependencies

- `hace-cona-fem-shell` (path dependency)
- `hace-io-rac` (canonical transport dependency)
- `hace-cona-fim` (optional)
- `tokio` 1.0
- `blake3` 1.5.1
- `spin` 0.9

## Build

```bash
cd engine/hace/cona/io
cargo build --release
```

**END OF README**
