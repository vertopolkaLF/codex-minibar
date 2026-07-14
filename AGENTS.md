Always run `cargo check` after changes related to the app to ensure the code is correct. No need to run it after changes related to the website.

Never launch the app itself.

All settings must take effect immediately in the running application. The user must never need to relaunch the app for a setting change to be applied. Keep every open UI surface and affected background component synchronized with the updated settings.

https://github.com/steipete/CodexBar - similar app for macos. You can use it as a reference for the features and implementations.
