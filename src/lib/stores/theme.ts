import { writable } from 'svelte/store';
import { getSystemTheme, onSystemThemeChanged, type SystemTheme } from '$lib/platform/theme';

type Theme = SystemTheme | 'auto';

const defaultTheme = 'auto';
const initialTheme =
	typeof localStorage !== 'undefined'
		? (localStorage.getItem('theme') ?? defaultTheme)
		: defaultTheme;

export const theme = writable<Theme>(initialTheme as Theme);

const editorThemeCallbacks = new Set<(theme: 'light' | 'dark') => void>();
let currentTheme = initialTheme as Theme;

theme.subscribe((value) => {
	currentTheme = value;

	if (typeof localStorage !== 'undefined') {
		localStorage.setItem('theme', value);
	}

	if (value === 'auto') {
		applyResolvedTheme(getSystemTheme());
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

onSystemThemeChanged((systemTheme) => {
	if (currentTheme === 'auto') {
		applyResolvedTheme(systemTheme);
	}
});

export function toggleTheme() {
	theme.update((current) => (current === 'light' ? 'dark' : 'light'));
}

export function registerEditorThemeCallback(callback: (theme: 'light' | 'dark') => void) {
	editorThemeCallbacks.add(callback);

	return () => {
		editorThemeCallbacks.delete(callback);
	};
}
