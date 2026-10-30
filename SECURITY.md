# Security

Please report security problems privately to the maintainers (see the contact page on theslopvibe.com) instead of opening a public issue.

What the app does with secrets: API keys are stored in the Windows Credential Manager, never in files or the database, and are hidden in logs and reports. The local API for Claude Desktop listens on 127.0.0.1 only and requires a token. Downloads of models and ComfyUI come from the official GitHub and Hugging Face hosts.
