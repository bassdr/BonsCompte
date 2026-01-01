# Fixing the 429 Rate Limit Issue

## Problem

Users are getting "429 Too Many Requests" errors when using the app. This happens because:

1. **NGINX rate limit is too restrictive** (10 req/s) compared to backend (100 req/s)
2. **Frontend makes multiple parallel requests** on page load (5-10 requests)
3. **Scanner bots consume quota** with probe requests (though backend blocks them)

## Solution

Update your NGINX configuration to match the backend's rate limits.

### Step 1: Backup Current Config

```bash
sudo cp /etc/nginx/conf.d/bonscompte.conf /etc/nginx/conf.d/bonscompte.conf.backup
```

### Step 2: Update the Configuration

Edit `/etc/nginx/conf.d/bonscompte.conf` and make these changes:

**BEFORE** (old, too restrictive):
```nginx
# In http block
limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;

# In server block
location /api/ {
    limit_req zone=api burst=20 nodelay;
    proxy_pass http://127.0.0.1:8000/;
    # ... other config
}
```

**AFTER** (new, matches backend limits):
```nginx
# In http block (or at top of the file, outside server block)
# Separate zones for auth and general API
limit_req_zone $binary_remote_addr zone=auth_limit:10m rate=5r/m;
limit_req_zone $binary_remote_addr zone=api_limit:10m rate=100r/s;

# In server block - add BEFORE the general /api/ location
location ~ ^/api/auth/(login|register) {
    limit_req zone=auth_limit burst=5 nodelay;
    limit_req_status 429;

    proxy_pass http://127.0.0.1:8000;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}

# In server block - update existing /api/ location
location /api/ {
    limit_req zone=api_limit burst=200 nodelay;
    limit_req_status 429;

    proxy_pass http://127.0.0.1:8000/;
    # ... rest of existing proxy config
}
```

**Or use the complete config:**

You can replace your entire config with the one in `docs/NGINX_PRODUCTION.conf` (already customized for bonscompte.duckdns.org).

### Step 3: Test the Configuration

```bash
# Test for syntax errors
sudo nginx -t

# If test passes, reload NGINX
sudo systemctl reload nginx
```

### Step 4: Verify the Fix

1. Open your browser to https://bonscompte.duckdns.org
2. Navigate through the app (projects, payments, debts)
3. Refresh the page multiple times
4. You should no longer see 429 errors

## What Changed?

| Endpoint Type | Old Limit | New Limit | Why |
|--------------|-----------|-----------|-----|
| Auth (login/register) | 10 req/s | 5 req/min | Prevents brute-force, matches backend |
| General API | 10 req/s, burst 20 | 100 req/s, burst 200 | Allows normal app usage with parallel requests |

## Rollback

If something goes wrong:

```bash
# Restore backup
sudo cp /etc/nginx/conf.d/bonscompte.conf.backup /etc/nginx/conf.d/bonscompte.conf

# Reload NGINX
sudo systemctl reload nginx
```

## Additional Notes

- The backend also has rate limiting, so there are two layers of protection
- Scanner bots get 404s from the backend's scan-blocking middleware (see `backend/src/main.rs:82-112`)
- The higher NGINX limits won't make your site less secure - the backend still enforces its own limits
- `burst=200` allows temporary spikes (like opening a page that makes 10 parallel requests)
