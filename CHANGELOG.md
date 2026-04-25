# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Docker Compose orchestration with PostgreSQL, Redis, Rust engine, and frontend
- Multi-stage Dockerfiles for rust_engine and frontend
- GitHub Actions CI with Rust, Python, and frontend parallel jobs
- GitHub Actions release workflow with GHCR image publishing
- Codecov integration for coverage reporting
- Rust integration tests: full backtest pipeline, data leakage prevention, execution delay
- Python test suite: interfaces, indicators, client mocks, data leakage
- Frontend test suite: chart rendering, playback store, WebSocket messages
- Comprehensive API specification with OpenAPI 3.0 schemas
- Strategy development guide with quick start and debugging tips
- Security documentation

### Changed
- Enhanced docker-compose.yml with healthchecks, networks, and Redis caching
- Enhanced Dockerfile with non-root user, healthchecks, and security hardening
- Enhanced CI workflow with path-based triggers and caching

### Security
- Added `.dockerignore` files for both rust_core and frontend
- Multi-stage builds to minimize attack surface
- Non-root container execution for rust_engine

## [0.1.0] - 2024-01-15

### Added
- Initial project scaffolding with Rust workspace, Python package, and React frontend
- Core data structures: StandardBar, Position, Order, EngineConfig
- Rust crates: data_pipeline, indicators, orderbook, engine, api
- Python package: BaseStrategy ABC, BacktestClient SDK
- Frontend: React 19 + TypeScript + Vite + Zustand setup
- Protocol Buffer schema for cross-language serialization
- PostgreSQL database migrations
- Basic docker-compose.yml with postgres and rust_engine

### Technical Details
- Rust Edition 2021 with tokio, axum, rust_decimal, sqlx
- Python 3.11+ with pydantic, aiohttp, pandas
- Node.js 20+ with React 19, TypeScript 5.5, Vite 6
