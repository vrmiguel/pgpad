import { writable } from 'svelte/store';

const defaultTheme = 'light';
const initialTheme = typeof localStorage !== 'undefined'
    ? localStorage.getItem('theme') ?? defaultTheme
    : defaultTheme;

export const theme = writable<'light' | 'dark'>(initialTheme as 'light' | 'dark');

const monacoThemeCallbacks = new Set<(theme: 'light' | 'dark') => void>();

theme.subscribe((value) => {
    if (typeof localStorage !== 'undefined') {
        localStorage.setItem('theme', value);
    }

    if (typeof document !== 'undefined') {
        if (value === 'dark') {
            document.documentElement.classList.add('dark');
        } else {
            document.documentElement.classList.remove('dark');
        }
    }

    monacoThemeCallbacks.forEach(callback => callback(value));
});

if (typeof document !== 'undefined' && initialTheme === 'dark') {
    document.documentElement.classList.add('dark');
}

export function toggleTheme() {
    theme.update(current => current === 'light' ? 'dark' : 'light');
}

export function registerMonacoThemeCallback(callback: (theme: 'light' | 'dark') => void) {
    monacoThemeCallbacks.add(callback);

    return () => {
        monacoThemeCallbacks.delete(callback);
    };
} 