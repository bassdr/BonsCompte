# Rate Limit 429 Error - Root Cause & Fix

## Root Cause

Your nginx config **defines** rate limit zones but **never applies them**:

```nginx
# These zones are defined...
limit_req_zone $binary_remote_addr zone=auth_limit:10m rate=5r/m;
limit_req_zone $binary_remote_addr zone=api_limit:10m rate=100r/s;

# But this location block has no limit_req directive!
location /api/ {
    proxy_pass http://127.0.0.1:8000/;
    # ... headers only, no rate limiting applied
}
```

This meant the backend's `tower_governor` was doing ALL the rate limiting, but with a critical flaw:

**When behind nginx, all requests appear to come from `127.0.0.1`**

- Backend sees: `127.0.0.1` → rate limit shared across ALL users
- Nginx sees: Real client IPs → can rate limit per user

When your frontend makes 6 parallel API calls on page load, they ALL count against the same `127.0.0.1` bucket. With multiple users or page refreshes, this bucket exhausts quickly → 429 errors.

## The Fix

### 1. Update Nginx Config on Server

```bash
# Backup current config
sudo cp /etc/nginx/conf.d/bonscompte.conf /etc/nginx/conf.d/bonscompte.conf.backup

# Deploy production config that APPLIES the rate limits
sudo cp ~/src/BonsCompte/docs/NGINX_PRODUCTION.conf /etc/nginx/conf.d/bonscompte.conf

# Test and reload
sudo nginx -t && sudo systemctl reload nginx
```

### 2. Rebuild & Redeploy Backend

The backend code has been updated to remove the general API rate limiter (it's now commented out in `main.rs:202-203`).

```bash
cd ~/src/BonsCompte/backend
cargo build --release

# Restart your backend service
# (however you're running it - systemd, docker, etc.)
```

### 3. Verify the Fix

1. Open https://bonscompte.duckdns.org
2. Refresh the page multiple times
3. Navigate between projects, payments, debts
4. No more 429 errors!

## What Changed

| Component | Before | After |
|-----------|--------|-------|
| **Nginx** | Rate limit zones defined but unused | `limit_req` directives applied (100 req/s, burst 200) |
| **Backend** | Rate limiting ALL users as `127.0.0.1` | General API rate limit disabled (nginx handles it) |
| **Auth endpoints** | Backend: 5 req/60s | Both nginx AND backend enforce 5 req/min (defense in depth) |

## Why This Works

- **Nginx sees real client IPs** (`$binary_remote_addr`) and can rate limit per user
- **Backend keeps auth rate limiting** for defense in depth (both layers protect login)
- **General API has generous limits** (100 req/s, burst 200) to handle parallel requests
- **Users are isolated** - one user's requests don't affect another's quota

## Security Notes

- Higher nginx limits don't reduce security - they match the backend's intended limits
- Auth endpoints still have strict 5 req/min limits (prevents brute force)
- Backend still blocks scanner probes with 404s (`main.rs:82-112`)
- Two-layer rate limiting on auth provides redundancy
