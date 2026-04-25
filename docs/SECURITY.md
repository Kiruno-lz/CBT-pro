# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes |

## Reporting a Vulnerability

If you discover a security vulnerability in CBT-Pro, please report it privately
to the maintainers. Do NOT open a public issue for security problems.

Email: security@example.com (placeholder — update with real contact)

## Security Considerations

### 1. No API Keys in Code

Never commit API keys, database passwords, or secrets to version control.
Use environment variables or a secrets manager:

```bash
# .env (never commit this file!)
DATABASE_URL=postgres://cbtpro:${DB_PASSWORD}@postgres/cbtpro
RUST_LOG=info
```

```yaml
# docker-compose.yml
services:
  rust_engine:
    environment:
      DATABASE_URL: ${DATABASE_URL}
```

### 2. Database Connection Security

- Use strong, unique passwords for PostgreSQL
- Run PostgreSQL on a private Docker network (not exposed to host)
- Enable SSL/TLS for production database connections
- Use connection pooling (PgBouncer) to prevent resource exhaustion
- Restrict database user privileges (read-only for strategy queries)

### 3. WebSocket Authentication (Future)

The WebSocket endpoint currently does not require authentication.
This is acceptable for local development only.

Planned authentication mechanisms:
- JWT-based session tokens
- API key validation per connection
- Rate limiting per authenticated user

```
# Future WebSocket auth flow
Client -> Server: { "type": "auth", "token": "jwt_token_here" }
Server -> Client: { "type": "auth_result", "success": true }
```

### 4. Data Validation at All Boundaries

Every input to the engine is validated:

| Boundary | Validation |
|----------|-----------|
| REST API | JSON schema validation, bounds checking |
| WebSocket | Message type whitelist, rate limiting |
| Strategy callbacks | No I/O, deterministic, no future data |
| Database | SQL injection prevention (SQLx compile-time checked) |
| File uploads | Type whitelist, size limits, virus scanning |

### 5. Container Security

- Multi-stage Docker builds minimize image size and attack surface
- Non-root user (`cbtpro`) runs the Rust engine
- Alpine and slim base images reduce CVE exposure
- Read-only filesystem where possible
- No unnecessary capabilities (drop all, add only NET_BIND_SERVICE)

### 6. Dependency Management

- Rust: `cargo audit` for known CVEs in dependencies
- Python: `pip-audit` or `safety` for vulnerability scanning
- Node.js: `npm audit` in CI pipeline
- Automated Dependabot alerts for all ecosystems

### 7. Network Security

```
Public Internet
       |
  [Reverse Proxy / WAF]
       |
  [Nginx Frontend]
       |
  [Rust Engine API]
       |
  [PostgreSQL + Redis]  (private network only)
```

- Do not expose PostgreSQL or Redis ports publicly
- Use Docker networks for inter-service communication
- Consider WireGuard or mTLS for multi-host deployments

### 8. Anti-Data-Leakage as a Security Feature

The anti-data-leakage rules are not just about correctness — they prevent
strategies from exploiting information asymmetry:

- `allow_future_data: false` (default) enforces the lookahead barrier
- Execution delay prevents same-bar arbitrage
- Audit trails enable post-hoc verification of fair play

### 9. CI/CD Security

- GitHub Actions workflows use minimal permissions (`permissions` block)
- No secrets are logged in CI output
- Docker images are scanned with Trivy or Snyk before push
- Only tagged releases push to container registry

## Security Checklist for Production Deployment

- [ ] Change all default passwords
- [ ] Enable PostgreSQL SSL
- [ ] Configure firewall rules
- [ ] Set up log aggregation and alerting
- [ ] Enable automated vulnerability scanning
- [ ] Implement API authentication
- [ ] Configure rate limiting
- [ ] Set up DDoS protection
- [ ] Run containers as non-root
- [ ] Enable audit logging for all trades
