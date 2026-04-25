# CBT-Pro

**C**rypto **B**ack**t**ester **Pro**fessional — Institutional-grade cryptocurrency quantitative backtesting system.

## Features

- **Hybrid Architecture**: Rust core engine + Python strategy layer + TypeScript frontend
- **High Performance**: Rust handles 1M+ bars/second with sub-microsecond latency
- **Anti-Data-Leakage**: Engine enforces strict lookahead barriers and execution delays
- **Real-Time Visualization**: WebSocket-driven chart playback with signal overlays
- **Institutional Grade**: FIFO/LIFO/WeightedAverage cost basis, margin calculations, liquidation modeling

## Architecture

```
                   +------------------+
                   |   Frontend       |
                   |   React 19 + TS  |
                   |   Port 3000      |
                   +--------+---------+
                            |
                     WebSocket / HTTP
                            |
                   +--------v---------+
                   |   Rust Engine    |
                   |   Axum HTTP/WS   |
                   |   Ports 8080/8081|
                   +--------+---------+
                            |
              +-------------+-------------+
              |                           |
       +------v------+            +-------v------+
       | PostgreSQL  |            |    Redis     |
       |   Port 5432 |            |   Port 6379  |
       +-------------+            +--------------+
```

See [`SPEC.md`](SPEC.md) for full technical specification.

## Quick Start (Docker Compose)

```bash
# Clone the repository
git clone https://github.com/your-org/cbt-pro.git
cd cbt-pro

# Start all services
docker-compose up -d

# Wait for healthchecks (postgres, rust_engine, frontend)
docker-compose ps

# Access the frontend
open http://localhost:3000
```

### Services

| Service | Port | Description |
|---------|------|-------------|
| Frontend | `3000` | React SPA served by nginx |
| REST API | `8080` | Axum HTTP API |
| WebSocket | `8081` | Real-time backtest events |
| PostgreSQL | `5432` | Historical data storage |
| Redis | `6379` | Indicator cache |

## Development Setup

### Prerequisites

- Docker + Docker Compose
- Rust 1.75+ (for engine development)
- Python 3.11+ (for strategy development)
- Node.js 20+ (for frontend development)

### Rust Core

```bash
cd rust_core

# Check all workspace crates compile
cargo check --workspace

# Run all tests (requires PostgreSQL)
cargo test --workspace

# Format and lint
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Generate coverage report
cargo tarpaulin --workspace --out Xml
```

### Python Strategies

```bash
cd python_strategies

# Install in editable mode with dev dependencies
pip install -e ".[dev]"

# Lint and format check
ruff check cbt_pro/ tests/
ruff format --check cbt_pro/ tests/

# Type check
mypy cbt_pro/

# Run tests with coverage
pytest tests/ -v --cov=cbt_pro --cov-report=xml --cov-report=term
```

### Frontend

```bash
cd frontend

# Install dependencies
npm ci

# Development server
npm run dev

# Type check
npm run typecheck

# Lint
npm run lint

# Unit tests
npm run test:unit

# Production build
npm run build
```

## Testing

### Test Structure

```
rust_core/tests/
  integration_test.rs         # End-to-end Rust engine tests

python_strategies/tests/
  test_interfaces.py          # BaseStrategy ABC tests
  test_indicators.py          # Indicator math verification
  test_client.py              # BacktestClient HTTP mock tests
  test_data_leakage.py        # Anti-leakage validation

frontend/src/tests/
  chart.test.tsx              # lightweight-charts wrapper tests
  playback.test.tsx           # Zustand playback store tests
  websocket.test.tsx          # WebSocket message handling tests
```

### Running All Tests

```bash
# Rust
cd rust_core && cargo test --workspace

# Python
cd python_strategies && pytest tests/ -v

# Frontend
cd frontend && npm run test:unit
```

### CI/CD

Three parallel CI jobs run on every PR:

| Job | Trigger | Checks |
|-----|---------|--------|
| `rust-ci` | `rust_core/**` changes | fmt, clippy, test, tarpaulin coverage |
| `python-ci` | `python_strategies/**` changes | ruff, mypy, pytest coverage |
| `frontend-ci` | `frontend/**` changes | lint, typecheck, vitest, build |

See [`.github/workflows/ci.yml`](.github/workflows/ci.yml) for full configuration.

## Performance Benchmarks

| Metric | Target | Status |
|--------|--------|--------|
| Bar processing | 1M+ bars/sec | In progress |
| WebSocket latency | < 1ms | In progress |
| Chart render (10k bars) | 60 FPS | In progress |
| Backtest startup | < 500ms | In progress |

## Contributing

We welcome contributions! Please follow these guidelines:

1. **Fork** the repository
2. Create a **feature branch** (`git checkout -b feature/amazing-feature`)
3. Follow the coding standards enforced by CI:
   - Rust: `cargo fmt`, `cargo clippy`, all tests passing
   - Python: `ruff`, `mypy strict`, `pytest`
   - Frontend: `tsc --noEmit`, `eslint`, `vitest`
4. **Commit** with [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat:` — New feature
   - `fix:` — Bug fix
   - `chore:` — Maintenance / tooling
   - `docs:` — Documentation only
   - `test:` — Test additions or fixes
   - `refactor:` — Code change that neither fixes a bug nor adds a feature
   - `perf:` — Performance improvement
5. **Open a Pull Request** to `develop`

### Commit Message Format

```
feat(engine): add liquidation modeling

- Add maintenance margin checks
- Implement forced position closure
- Update position status tracking

Refs: #42
```

## Security

See [`docs/SECURITY.md`](docs/SECURITY.md) for security considerations and best practices.

## License

MIT License — See [LICENSE](LICENSE) for details.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history.
