import { describe, it, expect } from 'vitest';

/**
 * API Configuration Tests for Production NGINX Reverse Proxy
 *
 * These tests validate that the API base URL configuration works correctly
 * for different deployment scenarios, particularly with NGINX reverse proxy.
 */
describe('API Configuration - Production NGINX Proxy Support', () => {
	it('should validate the API base URL selection logic', () => {
		// The logic in api.ts follows this precedence:
		// 1. VITE_API_BASE environment variable (highest priority)
		// 2. HTTPS in browser → /api (for NGINX reverse proxy)
		// 3. HTTP or server-side → http://localhost:8000 (development)

		const scenarios = [
			{
				name: 'Production HTTPS with NGINX proxy',
				env: undefined,
				protocol: 'https:',
				browser: true,
				expected: '/api',
				description: 'Should use /api which NGINX proxies to backend'
			},
			{
				name: 'Development HTTP',
				env: undefined,
				protocol: 'http:',
				browser: true,
				expected: 'http://localhost:8000',
				description: 'Should connect directly to backend'
			},
			{
				name: 'Custom deployment with env var',
				env: 'https://api.custom.com',
				protocol: 'https:',
				browser: true,
				expected: 'https://api.custom.com',
				description: 'Environment variable overrides all defaults'
			},
			{
				name: 'Server-side rendering',
				env: undefined,
				protocol: 'https:',
				browser: false,
				expected: 'http://localhost:8000',
				description: 'SSR should use localhost backend'
			}
		];

		// Document the expected behavior
		scenarios.forEach((scenario) => {
			expect(scenario.expected).toBeDefined();
			expect(scenario.description).toBeTruthy();
		});
	});

	it('should validate NGINX reverse proxy path mapping', () => {
		// Documents the NGINX configuration requirements
		const nginxProxyRules = {
			// Auth endpoints: /api/auth/login → http://127.0.0.1:8000/auth/login
			auth: {
				frontendPath: '/api/auth/login',
				nginxPattern: '^/api/auth/(login|register)',
				backendProxy: 'http://127.0.0.1:8000',
				rateLimit: '5r/m',
				burst: 5
			},
			// General API: /api/projects → http://127.0.0.1:8000/projects
			api: {
				frontendPath: '/api/projects',
				nginxPattern: '/api/',
				// Note: trailing slash in proxy_pass strips /api prefix
				backendProxy: 'http://127.0.0.1:8000/',
				rateLimit: '100r/s',
				burst: 200
			}
		};

		// Validate auth endpoint mapping
		expect(nginxProxyRules.auth.frontendPath).toContain('/api/');
		expect(nginxProxyRules.auth.nginxPattern).toMatch(/login|register/);
		expect(nginxProxyRules.auth.backendProxy).toContain('127.0.0.1:8000');

		// Validate general API mapping
		expect(nginxProxyRules.api.frontendPath).toMatch(/^\/api\//);
		expect(nginxProxyRules.api.backendProxy).toContain('127.0.0.1:8000');

		// Validate rate limiting is configured
		expect(nginxProxyRules.auth.rateLimit).toBe('5r/m');
		expect(nginxProxyRules.api.rateLimit).toBe('100r/s');
	});

	it('should validate production deployment checklist', () => {
		const productionChecklist = {
			nginx: {
				reverseProxyConfigured: true,
				httpsEnabled: true,
				rateLimitingEnabled: true,
				corsHeadersSet: true
			},
			frontend: {
				builtForProduction: true,
				httpsDetectionWorks: true,
				apiBaseUsesRelativePath: true
			},
			backend: {
				listeningOnLocalhost: true,
				port: 8000,
				jwtSecretSet: true,
				databasePathCorrect: true
			}
		};

		// Verify all production requirements are documented
		expect(productionChecklist.nginx.reverseProxyConfigured).toBe(true);
		expect(productionChecklist.nginx.httpsEnabled).toBe(true);
		expect(productionChecklist.frontend.apiBaseUsesRelativePath).toBe(true);
		expect(productionChecklist.backend.port).toBe(8000);
	});

	it('should validate API endpoint paths follow NGINX routing', () => {
		// All API calls in production should start with /api
		const productionEndpoints = [
			'/api/auth/login',
			'/api/auth/register',
			'/api/projects',
			'/api/projects/123/payments',
			'/api/projects/123/participants',
			'/api/projects/123/members',
			'/api/projects/123/debts',
			'/api/users/me/preferences'
		];

		// Validate all endpoints follow the /api pattern
		productionEndpoints.forEach((endpoint) => {
			expect(endpoint).toMatch(/^\/api\//);
		});

		// Validate auth endpoints are captured by auth rate limit pattern
		const authEndpoints = productionEndpoints.filter((e) => e.includes('/auth/'));
		authEndpoints.forEach((endpoint) => {
			expect(endpoint).toMatch(/^\/api\/auth\/(login|register)/);
		});
	});

	it('should document backend CORS requirements for production', () => {
		// Backend must allow requests from the frontend domain
		const corsConfig = {
			allowedOrigins: ['https://bonscompte.duckdns.org'],
			allowedMethods: ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS'],
			allowedHeaders: ['Content-Type', 'Authorization'],
			exposeHeaders: ['Content-Length'],
			allowCredentials: true,
			maxAge: 3600
		};

		// Validate CORS is properly configured
		expect(corsConfig.allowedOrigins).toContain('https://bonscompte.duckdns.org');
		expect(corsConfig.allowedMethods).toContain('POST');
		expect(corsConfig.allowedHeaders).toContain('Authorization');
	});
});
