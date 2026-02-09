# Rate Limiting with NGINX

## Why NGINX Handles Rate Limiting

Rate limiting is configured in the reverse proxy (NGINX/Caddy) because:

1. **Reverse proxies see real client IPs** - NGINX sees `$binary_remote_addr` (actual client)
2. **Backend sees proxy IP** - all requests appear to come from one IP (e.g., `127.0.0.1` or Docker network IP)
3. **Per-user rate limiting requires real IPs** - the backend can't distinguish between users behind a proxy

## Recommended Setup

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

## Troubleshooting 429 Errors

If you see "429 Too Many Requests":

1. **Check nginx error logs** - rate limit rejections are logged there
2. **Adjust burst values** if legitimate users are being blocked
3. **Check for misconfigured clients** making too many requests

## Security Notes

- NGINX rate limiting protects against brute-force attacks using real client IPs
- Backend scan-blocking middleware still blocks scanner probes with 404s
- Auth endpoints have strict limits (5 req/min) to prevent password guessing
- General API has generous limits (100 req/s) to handle parallel frontend requests
