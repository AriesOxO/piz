# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.2.x   | :white_check_mark: |
| < 0.2   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in piz, please report it responsibly:

1. **Do NOT open a public issue** for security vulnerabilities
2. Email the maintainer at the address listed in the repository profile, or use [GitHub Security Advisories](https://github.com/AriesOxO/piz/security/advisories/new)
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

## Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial assessment**: Within 1 week
- **Fix release**: Within 2 weeks for critical issues

## Security Architecture

piz implements a 3-layer security model:

1. **Prompt-level refusal** — LLM returns `{"refuse": true}` for non-command input
2. **Injection detection** — Local regex scan blocks malicious patterns (env exfiltration, reverse shells, base64 payloads, etc.)
3. **Danger classification** — Regex patterns + LLM-provided level classify commands as Safe/Warning/Dangerous

## API Key Storage

API keys are stored in plaintext in `~/.piz/config.toml`. Users should:

- Set restrictive file permissions: `chmod 600 ~/.piz/config.toml`
- Never commit this file to version control (`.piz/` is in `.gitignore`)
- Consider using environment variables for sensitive keys where supported

## Scope

The following are in scope for security reports:

- Command injection bypasses
- Injection detection evasion
- API key exposure
- Privilege escalation via generated commands
- Eval mode (`--eval`) file security
