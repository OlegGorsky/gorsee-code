# Privacy

Gorsee Code stores project state locally under `.gorsee-code/`.
Session events are redacted before being written, exported, or streamed by the
gateway. Raw API keys are never printed by CLI status commands.

By default the API key is read from `NEUROGATE_API_KEY`.
If `gcode auth set` is used, the key is stored in `.gorsee-code/auth.json`.
That path is included in `.gitignore`; users should still treat it as a local
secret file.
