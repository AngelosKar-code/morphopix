# Screenshots

Drop the three README images here, using exactly these names:

| File | What it should show |
| --- | --- |
| `empty-state.png` | The drop zone, before any images are queued |
| `queue-done.png` | A finished run: the queue with per-file savings and the summary bar |
| `compare.png` | The Compare dialog with the before/after divider part way across |

PNG, captured at the default 1280x820 window size. Crop to the window, not the whole desktop.

## Making the demo GIF

Record an MP4 first (Win+G opens the Xbox Game Bar and records the focused window), then
convert. The two passes matter: a single-pass conversion picks a generic 256-colour palette
and the dark UI turns to mud.

```bash
# 1. build a palette from the actual frames
ffmpeg -i demo.mp4 -vf "fps=12,scale=1000:-1:flags=lanczos,palettegen=stats_mode=diff" -y palette.png

# 2. apply it
ffmpeg -i demo.mp4 -i palette.png \
  -lavfi "fps=12,scale=1000:-1:flags=lanczos[v];[v][1:v]paletteuse=dither=bayer:bayer_scale=3" \
  -y demo.gif
```

Keep it short: one clear action, 5-10 seconds, under about 5MB. Trim the source with
`-ss 00:00:02 -t 8` before converting rather than recording exactly.
