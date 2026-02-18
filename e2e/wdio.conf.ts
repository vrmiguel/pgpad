import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const repoRoot = path.resolve(__dirname, '..');

const tauriAppName = process.platform === 'win32' ? 'app.exe' : 'app';
const tauriAppPath = path.resolve(repoRoot, 'src-tauri', 'target', 'debug', tauriAppName);
const tauriDriverPath = path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver');

let tauriDriver: ChildProcess | undefined;
let exiting = false;

export const config = {
	host: '127.0.0.1',
	port: 4444,
	specs: ['./specs/**/*.e2e.ts'],
	maxInstances: 1,
	capabilities: [
		{
			maxInstances: 1,
			'tauri:options': {
				application: tauriAppPath
			}
		}
	],
	reporters: ['spec'],
	framework: 'mocha',
	mochaOpts: {
		ui: 'bdd',
		timeout: 60000
	},

	onPrepare: () => {
		spawnSync('npm', ['run', 'tauri', 'build', '--', '--debug', '--no-bundle'], {
			cwd: repoRoot,
			stdio: 'inherit',
			shell: true
		});
	},

	beforeSession: () => {
		tauriDriver = spawn(tauriDriverPath, [], {
			stdio: [null, process.stdout, process.stderr]
		});

		tauriDriver.on('error', (error) => {
			console.error('tauri-driver error:', error);
			process.exit(1);
		});

		tauriDriver.on('exit', (code) => {
			if (!exiting) {
				console.error('tauri-driver exited unexpectedly with code:', code);
				process.exit(1);
			}
		});
	},

	afterSession: () => {
		closeTauriDriver();
	}
};

function closeTauriDriver() {
	exiting = true;
	tauriDriver?.kill();
}

function onShutdown(fn: () => void) {
	const cleanup = () => {
		try {
			fn();
		} finally {
			process.exit();
		}
	};

	process.on('exit', cleanup);
	process.on('SIGINT', cleanup);
	process.on('SIGTERM', cleanup);
	process.on('SIGHUP', cleanup);
	process.on('SIGBREAK', cleanup);
}

onShutdown(() => {
	closeTauriDriver();
});
