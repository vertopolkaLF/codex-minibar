# Codex Minibar promo media

Remotion compositions that rebuild the popup, notifications and Usage tab in
React for patch notes and the launch video. Colors, sizes, timings, icons and
sounds come from the app (`src/ui/popup_window/ui/theme.rs`, `toast.rs`,
`components.rs`, `assets/`); strings come from `locales/*/app.ftl`. All
account names and numbers are placeholders from `src/data.ts`.

```bash
npm install
npm run studio          # preview and scrub every composition
./render.sh             # render everything into out/release
./render.sh Themes      # or just some compositions
```

| Composition | Output |
| --- | --- |
| `HomeStill` | `home.png` |
| `Notifications`, `NotificationsLight` | transparent `.webp`, `.webm` (VP9 alpha), `.mov` (ProRes 4444), `.gif` |
| `Home`, `Layouts`, `Themes`, `Languages`, `Usage` | `.webp`, `.gif`, `.mp4` |
| `Launch` | `launch-1080p.mp4` with notification sounds |

Clips render at 2x and 60 fps; WebP and GIF are reduced to 30 and 25 fps.
`Launch` reuses the clip compositions, so a change to a clip updates the
video too. The Settings beat uses real captures in `public/shots/` (pages
without personal data).

Requires Node 22 and `ffmpeg` on `PATH`.
