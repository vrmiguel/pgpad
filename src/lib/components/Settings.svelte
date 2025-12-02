<script lang="ts">
import { Button } from '$lib/components/ui/button';
import { Commands, type OracleSettings } from '$lib/commands.svelte';

	interface Props {
		selectedConnection: string | null;
		selectedDbType?: unknown;
	}

	let { selectedConnection = $bindable(null), selectedDbType = $bindable() }: Props = $props();

	let scope = $state<'connection' | 'global'>(selectedConnection ? 'connection' : 'global');
	let settings = $state<OracleSettings | null>(null);
	let loading = $state(false);
let saving = $state(false);
let reconnecting = $state(false);
let lastSettingsKey = $state<string | null>(null);
let encryptionEnabled = $state(true);
let encryptionKeyStorage = $state<'keychain' | 'prompt'>('keychain');

	$effect(() => {
		if (!selectedConnection) {
			if (scope !== 'global') {
				scope = 'global';
			}
		}
	});

	$effect(() => {
		const connId = scope === 'connection' ? (selectedConnection ?? null) : null;
		const key = `${scope}:${connId ?? ''}`;
		if (lastSettingsKey === key) return;
		lastSettingsKey = key;
		loadSettings(connId ?? undefined);
	});

async function loadSettings(connId?: string) {
    loading = true;
    try {
        settings = await Commands.getOracleSettings(connId);
        const enc = await Commands.getEncryptionSettings();
        try {
            const obj = JSON.parse(enc) as { enabled: boolean; key_storage: string };
            encryptionEnabled = !!obj.enabled;
            encryptionKeyStorage = (obj.key_storage === 'prompt' ? 'prompt' : 'keychain');
        } catch {}
    } catch (e) {
        console.error('Failed to load settings', e);
        settings = null;
    } finally {
        loading = false;
    }
}

	async function save() {
		if (!settings) return;
		saving = true;
		try {
			const connId = scope === 'connection' ? (selectedConnection ?? undefined) : undefined;
			await Commands.setOracleSettings(settings, connId);
		} catch (e) {
			console.error('Failed to save settings', e);
		} finally {
			saving = false;
		}
	}

	async function reconnect() {
		if (!selectedConnection) return;
		reconnecting = true;
		try {
			await Commands.disconnectFromDatabase(selectedConnection);
			await Commands.connectToDatabase(selectedConnection);
		} catch (e) {
			console.error('Failed to reconnect', e);
		} finally {
			reconnecting = false;
		}
	}
</script>

<div class="flex h-full flex-col gap-4 p-4">
	<div class="flex items-center justify-between">
		<h2 class="text-lg font-semibold">Settings</h2>
		<div class="sticky top-0 z-10 flex flex-wrap items-center gap-3">
			<label class="flex items-center gap-2 text-sm">
				<span>Scope</span>
				<select
					class="w-28 rounded border px-2 py-1"
					bind:value={scope}
					disabled={!selectedConnection}
				>
					<option value="connection">Connection</option>
					<option value="global">Global</option>
				</select>
			</label>
			<Button onclick={save} disabled={saving || loading} variant="outline">
				{saving ? 'Saving…' : 'Save'}
			</Button>
			<Button onclick={reconnect} disabled={!selectedConnection || reconnecting}>
				{reconnecting ? 'Reconnecting…' : 'Reconnect'}
			</Button>
		</div>
	</div>

	{#if loading}
		<div class="text-muted-foreground text-sm">Loading settings…</div>
	{:else}
<div class="rounded-lg border p-5">
    <h3 class="mb-3 text-base font-semibold">Global Settings</h3>
    <p class="text-muted-foreground text-sm">
        Oracle‑specific settings have been moved to the connection dialog (Edit → Oracle Settings).
        Reconnect and formatting options are now configured per connection.
    </p>
    <div class="mt-4 grid grid-cols-2 gap-4">
        <div>
            <label class="mb-2 block text-sm font-medium">Database Encryption</label>
            <label class="flex items-center gap-2 text-sm">
                <input type="checkbox" bind:checked={encryptionEnabled} />
                <span>Enable app storage encryption</span>
            </label>
        </div>
        <div>
            <label class="mb-2 block text-sm font-medium">Key Storage Mode</label>
            <select class="w-40 rounded border px-2 py-1" bind:value={encryptionKeyStorage}>
                <option value="keychain">Keychain</option>
                <option value="prompt">Prompt at startup</option>
            </select>
        </div>
        <div class="col-span-2">
            <Button onclick={() => Commands.setEncryptionSettings(encryptionEnabled, encryptionKeyStorage)} disabled={saving || loading} variant="outline">
                Save Encryption Settings
            </Button>
        </div>
    </div>
</div>
	{/if}
</div>

<style>
	.text-muted-foreground {
		color: rgb(100 116 139);
	}
</style>
