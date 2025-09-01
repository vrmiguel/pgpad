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

const editorThemeCallbacks = new Set<(theme: Theme) => void>();

theme.subscribe(async (value) => {
	if (typeof localStorage !== 'undefined') {
		localStorage.setItem('theme', value);
	}
	if (value == 'auto') value = await getSystemTheme();
	if (typeof document !== 'undefined') {
		if (value === 'dark') {
			document.documentElement.classList.add('dark');
		} else {
			document.documentElement.classList.remove('dark');
		}
	}

	editorThemeCallbacks.forEach((callback) => callback(value));
});

if (typeof document !== 'undefined' && initialTheme === 'dark') {
	document.documentElement.classList.add('dark');
}

export function toggleTheme() {
	theme.update((current) => (current === 'light' ? 'dark' : 'light'));
}

await getCurrentWindow().onThemeChanged(({ payload: currentTheme }) => {
	theme.set(currentTheme);
});

export function registerEditorThemeCallback(callback: (theme: Theme) => void) {
	editorThemeCallbacks.add(callback);

	return () => {
		editorThemeCallbacks.delete(callback);
	};
}
