# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.3.x   | :white_check_mark: |
| 0.2.x   | :x:                |
| < 0.2   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in caxe, please report it responsibly:

1. **DO NOT** open a public GitHub issue for security vulnerabilities
2. Email the maintainer directly at: <dhimasardinatapp@gmail.com>
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

## Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial Assessment**: Within 7 days
- **Fix Release**: Depends on severity (critical: ASAP, high: 14 days, medium: 30 days)

## Security Considerations

### Dependency Downloads
- caxe downloads dependencies from Git URLs specified in `cx.toml`
- **Always verify** the source of dependencies before adding them
- Use pinned versions (`tag`, `rev`) for production projects

### Prebuilt Binaries
- Prebuilt binaries are downloaded from official GitHub releases
- SHA256 verification is available for integrity checks

### Build Scripts
- Pre/post build scripts in `cx.toml` are executed during builds
- **Review all scripts** before running builds in untrusted projects

## Best Practices

1. **Pin dependencies** to specific tags or commits
2. **Use lockfiles** (`cx.lock`) for reproducible builds
3. **Audit dependencies** before first build
4. **Run in sandboxed environments** for untrusted code
