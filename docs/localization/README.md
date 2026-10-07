# Localization

English is the source language. Translations will use embedded Fluent catalogs,
with per-message English fallback and a live Auto / English / Russian setting.

Implementation in progress:
- Inventory application strings, distinguishing UI copy from protocol and storage identifiers.
- Add an English source catalog, translator template and Russian translation.
- Connect all UI surfaces and notifications to the current language.
- Validate catalog coverage, fallback and Rust compilation without launching the app.
