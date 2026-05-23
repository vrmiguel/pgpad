const WEB_TOKEN_STORAGE_KEY = 'pgpad.web.token';

let currentWebToken: string | null = null;

export function initializeWebToken(): boolean {
	if (typeof window === 'undefined') return false;

	const url = new URL(window.location.href);
	const token = url.searchParams.get('token');

	if (token !== null) {
		if (token) {
			currentWebToken = token;
			getSessionStorage()?.setItem(WEB_TOKEN_STORAGE_KEY, token);
		}

		url.searchParams.delete('token');
		window.history.replaceState(
			window.history.state,
			'',
			`${url.pathname}${url.search}${url.hash}`
		);
	}

	return getWebToken() !== null;
}

export function getWebToken(): string | null {
	const storedToken = getSessionStorage()?.getItem(WEB_TOKEN_STORAGE_KEY);
	if (storedToken) {
		currentWebToken = storedToken;
	}

	return currentWebToken;
}

function getSessionStorage(): Storage | null {
	try {
		return typeof sessionStorage === 'undefined' ? null : sessionStorage;
	} catch {
		return null;
	}
}
