import { afterEach, describe, expect, it, vi } from 'vitest';

function stubBrowser(href: string, initialValues: Record<string, string> = {}) {
	const values = new Map<string, string>(Object.entries(initialValues));
	const replaceState = vi.fn();

	vi.stubGlobal('window', {
		location: { href },
		history: {
			state: { current: true },
			replaceState
		}
	});
	vi.stubGlobal('sessionStorage', {
		getItem: vi.fn((key: string) => values.get(key) ?? null),
		setItem: vi.fn((key: string, value: string) => {
			values.set(key, value);
		})
	});

	return { replaceState, values };
}

describe('webToken', () => {
	afterEach(() => {
		vi.unstubAllGlobals();
		vi.resetModules();
	});

	it('stores launch token and removes it from the URL', async () => {
		const { replaceState } = stubBrowser('http://127.0.0.1:3000/?token=abc123&view=main#tabs');
		const { getWebToken, initializeWebToken } = await import('./webToken');

		const hasToken = initializeWebToken();

		expect(hasToken).toBe(true);
		expect(getWebToken()).toBe('abc123');
		expect(replaceState).toHaveBeenCalledWith({ current: true }, '', '/?view=main#tabs');
	});

	it('uses an existing stored token when the launch URL has no token', async () => {
		const { replaceState } = stubBrowser('http://127.0.0.1:3000/?view=main', {
			'pgpad.web.token': 'stored-token'
		});
		const { getWebToken, initializeWebToken } = await import('./webToken');

		const hasToken = initializeWebToken();

		expect(hasToken).toBe(true);
		expect(getWebToken()).toBe('stored-token');
		expect(replaceState).not.toHaveBeenCalled();
	});

	it('reports when no launch or stored token is available', async () => {
		const { replaceState } = stubBrowser('http://127.0.0.1:3000/?view=main');
		const { getWebToken, initializeWebToken } = await import('./webToken');

		const hasToken = initializeWebToken();

		expect(hasToken).toBe(false);
		expect(getWebToken()).toBeNull();
		expect(replaceState).not.toHaveBeenCalled();
	});

	it('removes an empty launch token from the URL without accepting it', async () => {
		const { replaceState } = stubBrowser('http://127.0.0.1:3000/?token=&view=main');
		const { getWebToken, initializeWebToken } = await import('./webToken');

		const hasToken = initializeWebToken();

		expect(hasToken).toBe(false);
		expect(getWebToken()).toBeNull();
		expect(replaceState).toHaveBeenCalledWith({ current: true }, '', '/?view=main');
	});
});
