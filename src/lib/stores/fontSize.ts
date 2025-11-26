import { writable } from 'svelte/store';

const MIN_FONT_SIZE = 8;
const MAX_FONT_SIZE = 36;
const DEFAULT_FONT_SIZE = 13;

const initialFontSize =
	typeof localStorage !== 'undefined'
		? parseInt(localStorage.getItem('fontSize') ?? DEFAULT_FONT_SIZE.toString(), 10)
		: DEFAULT_FONT_SIZE;

const clampedInitialSize = Math.max(MIN_FONT_SIZE, Math.min(MAX_FONT_SIZE, initialFontSize));

export const fontSize = writable(clampedInitialSize);

fontSize.subscribe((value) => {
	if (typeof localStorage !== 'undefined') {
		localStorage.setItem('fontSize', value.toString());
	}
});

export const fontSizeUtils = {
	increase: () => {
		fontSize.update((size) => Math.min(MAX_FONT_SIZE, size + 1));
	},
	decrease: () => {
		fontSize.update((size) => Math.max(MIN_FONT_SIZE, size - 1));
	},
	set: (size: number) => {
		fontSize.set(Math.max(MIN_FONT_SIZE, Math.min(MAX_FONT_SIZE, size)));
	},
	reset: () => {
		fontSize.set(DEFAULT_FONT_SIZE);
	},
	MIN_FONT_SIZE,
	MAX_FONT_SIZE,
	DEFAULT_FONT_SIZE
};
