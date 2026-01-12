# Rate Limiting Architecture

## How Rate Limiting Works

**Backend rate limiting is disabled by default** (`RATE_LIMIT_ENABLED=false`).

This is intentional because:

1. **Reverse proxies see real client IPs** - nginx/Caddy/Traefik see `$binary_remote_addr` (actual client)
2. **Backend sees proxy IP** - all requests appear to come from one IP (e.g., `127.0.0.1` or Docker network IP)
3. **Per-user rate limiting requires real IPs** - backend can't distinguish between users behind a proxy

## Recommended Setup

### Nginx handles rate limiting (default)

Use `docs/NGINX_PRODUCTION.conf` which includes:

```nginx
# Rate limit zones (per real client IP)
limit_req_zone $binary_remote_addr zone=auth_limit:10m rate=5r/m;
limit_req_zone $binary_remote_addr zone=api_limit:10m rate=100r/s;

# Auth endpoints - strict (5 req/min, burst 5)
location ~ ^/api/auth/(login|register)$ {
    limit_req zone=auth_limit burst=5 nodelay;
    # ...
}

# General API - permissive (100 req/s, burst 200)
location /api/ {
    limit_req zone=api_limit burst=200 nodelay;
    # ...
}
```

### Backend rate limiting disabled

No configuration needed - `RATE_LIMIT_ENABLED` defaults to `false`.

Backend logs will show:
```
Rate limiting: disabled (reverse proxy should handle rate limiting)
```

## When to Enable Backend Rate Limiting

Only set `RATE_LIMIT_ENABLED=true` if the backend is **directly exposed to the internet** without any reverse proxy. This is rare and not recommended.

## Troubleshooting 429 Errors

If you see "429 Too Many Requests":

1. **Check backend logs** - if rate limiting is enabled, the 429 comes from the backend
2. **Check nginx logs** - if rate limiting is in nginx config, the 429 comes from nginx
3. **Ensure backend rate limiting is disabled** when behind a proxy:
   ```bash
   docker logs bonscompte-backend | grep "Rate limiting"
   # Should show: "disabled (reverse proxy should handle rate limiting)"
   ```

If backend shows "enabled", either:
- Update to latest version (default changed to disabled)
- Or set `RATE_LIMIT_ENABLED=false` in docker-compose.yml

## Security Notes

- Nginx rate limiting protects against abuse while seeing real client IPs
- Backend scan-blocking middleware still blocks scanner probes with 404s
- Auth endpoints have strict limits (5 req/min) to prevent brute-force attacks
- General API has generous limits (100 req/s) to handle parallel frontend requests
