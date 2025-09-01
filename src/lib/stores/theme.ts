import { writable } from 'svelte/store';
import { getCurrentWindow } from '@tauri-apps/api/window';

type Theme = 'light' | 'dark' | 'auto';

async function getSystemTheme() {
	try {
		const theme = await getCurrentWindow().theme();
		return theme;
	} catch (error) {
		console.error('Error getting system theme:', error);
	}
}

const defaultTheme = 'auto';
const initialTheme =
	typeof localStorage !== 'undefined'
		? (localStorage.getItem('theme') ?? defaultTheme)
		: defaultTheme;

export const theme = writable<Theme>(initialTheme as Theme);

const editorThemeCallbacks = new Set<(theme: 'light' | 'dark') => void>();

theme.subscribe((value) => {
	if (typeof localStorage !== 'undefined') {
		localStorage.setItem('theme', value);
	}

	if (value === 'auto') {
		getSystemTheme()
			.then((systemTheme) => {
				const resolvedTheme = systemTheme || 'light';
				applyResolvedTheme(resolvedTheme);
			})
			.catch((error) => {
				console.error('Error resolving system theme:', error);
				applyResolvedTheme('light');
			});
	} else {
		applyResolvedTheme(value);
	}
});

function applyResolvedTheme(resolvedTheme: 'light' | 'dark') {
	if (typeof document !== 'undefined') {
		if (resolvedTheme === 'dark') {
			document.documentElement.classList.add('dark');
		} else {
			document.documentElement.classList.remove('dark');
		}
	}

	editorThemeCallbacks.forEach((callback) => callback(resolvedTheme));
}

if (typeof document !== 'undefined') {
	if (initialTheme === 'auto') {
		getSystemTheme()
			.then((systemTheme) => {
				const resolvedTheme = systemTheme || 'light';
				applyResolvedTheme(resolvedTheme);
			})
			.catch((error) => {
				console.error('Error resolving initial system theme:', error);
				applyResolvedTheme('light');
			});
	} else if (initialTheme === 'dark') {
		document.documentElement.classList.add('dark');
	}
}

export function toggleTheme() {
	theme.update((current) => (current === 'light' ? 'dark' : 'light'));
}

getCurrentWindow().onThemeChanged(({ payload: currentTheme }) => {
	theme.set(currentTheme);
});

export function registerEditorThemeCallback(callback: (theme: 'light' | 'dark') => void) {
	editorThemeCallbacks.add(callback);

	return () => {
		editorThemeCallbacks.delete(callback);
	};
}
