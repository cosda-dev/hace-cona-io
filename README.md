# hace-cona-io

**Version**: 0.1.0  
**Edition**: 2021  
**License**: Proprietary  
**Description**: CONA IO FEM module

---

## Overview

`hace-cona-io` is the FEM (Fractal Engine Machine) module for IO operations. It provides RAC (Remote Actor Communication) protocol integration, ingress/egress handling, and backpressure management.

### Key Components

- **RAC** — Remote Actor Communication protocol (racin, racex, racid, racbox)
- **Ingress/Egress** — Data flow management
- **Backpressure** — Flow control and backpressure handling
- **Trading strategy** — Strategy execution framework
- **Resolver** — IO resolution and routing

### Features

| Feature | Default | Description |
|---|---|---|
| `std` | ✅ | Standard library mode |
| `fim` | ❌ | FIM (Feature Inference Module) integration |

---

## Dependencies

- `hace-cona-fem-shell` (path dependency)
- `hace-io-rac` (path dependency)
- `hace-cona-fim` (optional)
- `tokio` 1.0
- `blake3` 1.5.1
- `spin` 0.9

---

## Build

```bash
cd engine/hace/cona/io
cargo build --release
```

**END OF README**
