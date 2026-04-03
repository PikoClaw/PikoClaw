# Docs Deploy Setup

This file documents the one-time setup needed to make the GitHub Actions workflow work.

---

## How it works

```
PikoClaw repo (main branch)
  docs/** changed
       │
       ▼
  GitHub Actions: docs.yml
       │  1. npm install + vitepress build
       │  2. checkout pikoclaw-website repo
       │  3. copy .vitepress/dist → website-repo/docs/
       │  4. git commit + push
       ▼
  pikoclaw-website repo (main branch)
       │
       ▼
  GitHub Pages → pikoclaw.com/docs/
```

---

## One-time setup

### 1. Create a Personal Access Token (PAT)

The workflow needs permission to push to `pikoclaw/pikoclaw-website`.

1. Go to GitHub → Settings → Developer settings → Personal access tokens → Fine-grained tokens
2. Click **Generate new token**
3. Set:
   - **Token name**: `PIKOCLAW_DOCS_DEPLOY`
   - **Expiration**: No expiration (or 1 year)
   - **Repository access**: Only select repositories → `pikoclaw/pikoclaw-website`
   - **Permissions**:
     - Contents: **Read and write**
     - Metadata: Read (required)
4. Copy the token

### 2. Add the token as a secret in PikoClaw repo

1. Go to `github.com/PikoClaw/PikoClaw` → Settings → Secrets and variables → Actions
2. Click **New repository secret**
3. Name: `WEBSITE_DEPLOY_TOKEN`
4. Value: paste the token from step 1
5. Save

### 3. Enable GitHub Pages on pikoclaw-website

1. Go to `github.com/pikoclaw/pikoclaw-website` → Settings → Pages
2. Source: **Deploy from a branch**
3. Branch: `main`, folder: `/ (root)`
4. Save

The CNAME file (`pikoclaw.com`) is already in the repo root — Pages will use it automatically.

---

## Local development

```bash
cd docs
npm install
npm run dev     # → http://localhost:5173/docs/
```

## Manual trigger

Go to `github.com/PikoClaw/PikoClaw` → Actions → Deploy Docs → Run workflow.

---

## URL structure after deploy

```
pikoclaw.com/                  ← pikoclaw-website/index.html (existing landing page)
pikoclaw.com/docs/             ← VitePress home (docs/index.md)
pikoclaw.com/docs/spec/        ← Feature specs index
pikoclaw.com/docs/design-spec/ ← Design specs index
```
