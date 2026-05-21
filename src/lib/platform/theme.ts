export type SystemTheme = 'light' | 'dark';

function prefersDark() {
	return (
		typeof window !== 'undefined' &&
		window.matchMedia?.('(prefers-color-scheme: dark)').matches === true
	);
}

// matchMedia works in browsers and Tauri webviews. This intentionally ignores Rust-side setTheme.
export function getSystemTheme(): SystemTheme {
	return prefersDark() ? 'dark' : 'light';
}

export function onSystemThemeChanged(callback: (theme: SystemTheme) => void) {
	if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
		return () => {};
	}

	const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
	const listener = (event: MediaQueryListEvent) => {
		callback(event.matches ? 'dark' : 'light');
	};

	mediaQuery.addEventListener('change', listener);

	return () => {
		mediaQuery.removeEventListener('change', listener);
	};
}
