# 倉頡日課  Cangjie Daily

Ten Cangjie 5 characters a day. Type the codes, Check or Hint each row, or draw **10 more**.

```bash
cargo run --release   # pack rime/ → web/cangjie.json
cd web && trunk serve # http://127.0.0.1:8080
```

Pushes to `master` build with Trunk and deploy to GitHub Pages. In the repo: **Settings → Pages → Source: GitHub Actions**.
