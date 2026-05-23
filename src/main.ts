import { mount } from 'svelte';
import './app.css';
import { isTauri } from '@tauri-apps/api/core';
import App from './App.svelte';
import MissingWebToken from '$lib/components/MissingWebToken.svelte';
import { initializeWebToken } from '$lib/webToken';

const target = document.getElementById('app')!;
let app: ReturnType<typeof mount>;

if (isTauri() || initializeWebToken()) {
	app = mount(App, { target });
} else {
	app = mount(MissingWebToken, { target });
}

export default app;
