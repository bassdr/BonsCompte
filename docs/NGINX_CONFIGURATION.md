# NGINX Configuration for BonsCompte

This guide explains how to configure NGINX as a reverse proxy for BonsCompte in production deployments.

## Overview

In production, NGINX serves as a reverse proxy that:
1. Terminates SSL/TLS connections
2. Routes frontend requests to the SvelteKit server
3. Routes `/api/*` requests to the Rust backend (stripping the `/api` prefix)

## Architecture

```
                    ┌─────────────────┐
                    │     NGINX       │
                    │   (port 443)    │
                    └────────┬────────┘
                             │
            ┌────────────────┼────────────────┐
            │                │                │
            ▼                ▼                ▼
       /api/*              /*           /static/*
   (strip prefix)      (frontend)     (frontend)
            │                │                │
            ▼                ▼                ▼
    ┌───────────────┐  ┌───────────────┐
    │   Backend     │  │   Frontend    │
    │  (port 8000)  │  │  (port 3000)  │
    └───────────────┘  └───────────────┘
```

## Prerequisites

- NGINX installed
- SSL/TLS certificate (e.g., from Let's Encrypt)
- BonsCompte running via Docker Compose

## Basic Configuration

Create `/etc/nginx/conf.d/bonscompte.conf`:

```nginx
# WebSocket upgrade map (for SvelteKit HMR in dev, optional in production)
map $http_upgrade $connection_upgrade {
    default upgrade;
    '' close;
}

server {
    server_name example.com;
    listen 443 ssl http2;
    listen [::]:443 ssl http2;

    # SSL Configuration
    ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;
    ssl_session_timeout 1d;
    ssl_session_cache shared:SSL:50m;
    ssl_session_tickets off;

    # Modern SSL configuration
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;

    # HSTS (optional, uncomment if you want to enforce HTTPS)
    # add_header Strict-Transport-Security "max-age=63072000" always;

    # Frontend (SvelteKit)
    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
    }

    # Backend API
    # The trailing slash in proxy_pass strips the /api prefix
    location /api/ {
        proxy_pass http://127.0.0.1:8000/;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

# HTTP to HTTPS redirect
server {
    listen 80;
    listen [::]:80;
    server_name example.com;
    return 301 https://$server_name$request_uri;
}
```

## How Path Rewriting Works

The key to the API routing is this line:

```nginx
location /api/ {
    proxy_pass http://127.0.0.1:8000/;
}
```

- `location /api/` matches any request starting with `/api/`
- The trailing slash in `proxy_pass http://127.0.0.1:8000/` causes NGINX to **strip the `/api` prefix**

Example:
- Browser requests: `GET /api/auth/login`
- NGINX proxies to: `GET /auth/login` on backend

This is why the production Docker images are built with `VITE_API_BASE=/api` - the frontend prepends `/api` to all API calls, and NGINX strips it before forwarding to the backend.

## Docker Compose Integration

Update your `docker-compose.yml` to only expose ports locally:

```yaml
services:
  backend:
    ports:
      - "127.0.0.1:8000:8000"  # Only accessible from localhost

  frontend:
    ports:
      - "127.0.0.1:3000:3000"  # Only accessible from localhost
```

This ensures that only NGINX can access the containers, not external traffic.

## Testing the Configuration

1. Test NGINX configuration:
   ```sh
   nginx -t
   ```

2. Reload NGINX:
   ```sh
   systemctl reload nginx
   ```

3. Test the API endpoint:
   ```sh
   # This should return the health check
   curl https://example.com/api/health
   ```

4. Test the frontend:
   ```sh
   # This should return the SvelteKit app
   curl https://example.com/
   ```

## Troubleshooting

### 502 Bad Gateway
- Check if Docker containers are running: `docker compose ps`
- Check container logs: `docker compose logs backend frontend`
- Verify ports are correct: `ss -tlnp | grep -E '(3000|8000)'`

### 404 on API calls
- Verify the `location /api/` block has the trailing slash in `proxy_pass`
- Check backend logs for incoming requests
- Ensure frontend was built with `VITE_API_BASE=/api`

### CORS errors
- The backend already includes CORS headers
- If issues persist, check that `X-Forwarded-*` headers are being set correctly

## Advanced Configuration

### Rate Limiting

⚠️ **IMPORTANT**: The backend has its own rate limiting (100 req/s for API, 5 req/min for auth). NGINX rate limits should be **higher** than backend limits to avoid blocking legitimate traffic before it reaches the backend.

**Recommended configuration** (matches backend limits):

```nginx
# In http block (usually in nginx.conf or at top of conf.d file)
# Auth endpoints: 5 requests per minute (prevents brute-force)
limit_req_zone $binary_remote_addr zone=auth_limit:10m rate=5r/m;

# General API: 100 requests per second (allows normal app usage)
limit_req_zone $binary_remote_addr zone=api_limit:10m rate=100r/s;

# In server block - apply stricter limit to auth endpoints
location ~ ^/api/auth/(login|register) {
    limit_req zone=auth_limit burst=5 nodelay;
    limit_req_status 429;
    # ... proxy config
}

# In server block - permissive limit for general API
location /api/ {
    limit_req zone=api_limit burst=200 nodelay;
    limit_req_status 429;
    # ... proxy config
}
```

**Why these limits?**
- Frontend makes 5-10 parallel requests on page load (payments, participants, debts, etc.)
- Scanner bots probe the site constantly (handled by backend's scan blocking middleware)
- Too low limits (e.g., 10r/s) cause legitimate requests to fail with 429 errors

See `docs/NGINX_PRODUCTION.conf` for a complete production-ready configuration.

### Caching Static Assets

SvelteKit handles caching headers for static assets, but you can add NGINX-level caching:

```nginx
location /_app/ {
    proxy_pass http://127.0.0.1:3000;
    proxy_cache_valid 200 1d;
    add_header Cache-Control "public, max-age=86400";
}
```

### Multiple Domains

To serve BonsCompte on multiple domains:

```nginx
server {
    server_name example.com www.example.com;
    # ... rest of config
}
```
