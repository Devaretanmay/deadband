# Deadband

## Stop your AI agents from looping.

Deadband detects and stops AI agents from looping. One command: `deadband enable`.

## Quick Start

```bash
pip install deadband
deadband enable
```

Configure your agent to use `http://localhost:4399/v1` as the API base URL.

## Commands

- `deadband enable` — Start the proxy
- `deadband disable` — Stop the proxy  
- `deadband status` — Show stats

## How It Works

1. Agent makes API call → Deadband proxy
2. Deadband checks for exact repeats (same tool + same args)
3. If repeat detected → Inject a prompt
4. Agent recovers. Task completes.

## License

MIT
