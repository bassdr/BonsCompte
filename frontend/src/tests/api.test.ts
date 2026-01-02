import { describe, it, expect, beforeEach, vi } from 'vitest';
import { login, register, ApiRequestError } from '$lib/api';

describe('API - Authentication', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		localStorage.clear();
		sessionStorage.clear();
	});

	describe('login', () => {
		it('should successfully login with valid credentials', async () => {
			const mockResponse = {
				token: 'mock-jwt-token',
				user: {
					id: 1,
					username: 'testuser',
					display_name: 'Test User',
					preferences: {
						date_format: 'mdy',
						decimal_separator: '.',
						currency_symbol: '$',
						currency_symbol_position: 'before'
					}
				}
			};

			global.fetch = vi.fn().mockResolvedValueOnce({
				ok: true,
				status: 200,
				json: async () => mockResponse
			} as Response);

			const result = await login('testuser', 'password123');

			expect(fetch).toHaveBeenCalledWith(
				expect.stringContaining('/auth/login'),
				expect.objectContaining({
					method: 'POST',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify({
						username: 'testuser',
						password: 'password123'
					})
				})
			);

			expect(result).toEqual(mockResponse);
			expect(result.token).toBe('mock-jwt-token');
			expect(result.user.username).toBe('testuser');
		});

		it('should throw ApiRequestError on invalid credentials', async () => {
			const mockError = {
				code: 'INVALID_CREDENTIALS',
				error: 'Invalid credentials'
			};

			global.fetch = vi.fn().mockResolvedValueOnce({
				ok: false,
				status: 401,
				json: vi.fn().mockResolvedValue(mockError)
			} as unknown as Response);

			try {
				await login('testuser', 'wrongpassword');
				expect.fail('Expected login to throw');
			} catch (error) {
				expect(error).toBeInstanceOf(ApiRequestError);
				if (error instanceof ApiRequestError) {
					expect(error.code).toBe('INVALID_CREDENTIALS');
					expect(error.status).toBe(401);
				}
			}
		});

		it('should handle network errors', async () => {
			global.fetch = vi.fn().mockRejectedValueOnce(new Error('Network error'));

			await expect(login('testuser', 'password')).rejects.toThrow('Network error');
		});

		it('should handle malformed JSON responses', async () => {
			global.fetch = vi.fn().mockResolvedValueOnce({
				ok: false,
				status: 500,
				json: async () => {
					throw new Error('Invalid JSON');
				}
			} as Response);

			await expect(login('testuser', 'password')).rejects.toThrow(ApiRequestError);
		});
	});

	describe('register', () => {
		it('should successfully register a new user', async () => {
			const mockResponse = {
				token: 'mock-jwt-token',
				user: {
					id: 1,
					username: 'newuser',
					display_name: 'New User',
					preferences: {
						date_format: 'mdy',
						decimal_separator: '.',
						currency_symbol: '$',
						currency_symbol_position: 'before'
					}
				}
			};

			global.fetch = vi.fn().mockResolvedValueOnce({
				ok: true,
				status: 200,
				json: async () => mockResponse
			} as Response);

			const result = await register('newuser', 'password123', 'New User');

			expect(fetch).toHaveBeenCalledWith(
				expect.stringContaining('/auth/register'),
				expect.objectContaining({
					method: 'POST',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify({
						username: 'newuser',
						password: 'password123',
						display_name: 'New User'
					})
				})
			);

			expect(result).toEqual(mockResponse);
		});

		it('should throw ApiRequestError when username exists', async () => {
			const mockError = {
				code: 'USERNAME_EXISTS',
				error: 'This username is already taken'
			};

			global.fetch = vi.fn().mockResolvedValueOnce({
				ok: false,
				status: 409,
				json: vi.fn().mockResolvedValue(mockError)
			} as unknown as Response);

			try {
				await register('existinguser', 'password123');
				expect.fail('Expected register to throw');
			} catch (error) {
				expect(error).toBeInstanceOf(ApiRequestError);
				if (error instanceof ApiRequestError) {
					expect(error.code).toBe('USERNAME_EXISTS');
					expect(error.status).toBe(409);
				}
			}
		});

		it('should throw ApiRequestError for weak password', async () => {
			const mockError = {
				code: 'BAD_REQUEST',
				error: 'Password must be at least 6 characters'
			};

			global.fetch = vi.fn().mockResolvedValueOnce({
				ok: false,
				status: 400,
				json: vi.fn().mockResolvedValue(mockError)
			} as unknown as Response);

			try {
				await register('newuser', 'weak');
				expect.fail('Expected register to throw');
			} catch (error) {
				expect(error).toBeInstanceOf(ApiRequestError);
				if (error instanceof ApiRequestError) {
					expect(error.code).toBe('BAD_REQUEST');
					expect(error.status).toBe(400);
				}
			}
		});
	});
});
