# Parts Catalog Resolution Engine

A high-performance Rust engine for resolving component assemblies, part fitment graphs, and interchange equivalences with confidence scoring.

## Features

- **Assembly Hierarchy Traversal**: Traverses vehicle assembly directed acyclic graphs (DAGs) to locate target subsystems.
- **Interchange Resolution**: Maps OEM part numbers to certified alternatives with fitment confidence scoring.
- **Deterministic Scoring**: Produces traceable confidence scores and resolution notes for repair bills of materials.

## Build & Test

```bash
cargo check
cargo test
```

## License

MIT
