<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Database, X } from '@lucide/svelte';

	interface Props {
		onSubmit: (connection: any) => void;
		onCancel: () => void;
	}

	let { onSubmit, onCancel }: Props = $props();

	let formData = $state({
		name: '',
		host: 'localhost',
		port: 5432,
		database: '',
		username: '',
		password: ''
	});

	let errors = $state<Record<string, string>>({});

	function validateForm() {
		errors = {};
		
		if (!formData.name.trim()) {
			errors.name = 'Connection name is required';
		}
		
		if (!formData.host.trim()) {
			errors.host = 'Host is required';
		}
		
		if (!formData.database.trim()) {
			errors.database = 'Database name is required';
		}
		
		if (!formData.username.trim()) {
			errors.username = 'Username is required';
		}

		if (formData.port < 1 || formData.port > 65535) {
			errors.port = 'Port must be between 1 and 65535';
		}

		return Object.keys(errors).length === 0;
	}

	function handleSubmit(e: Event) {
		e.preventDefault();
		
		if (validateForm()) {
			onSubmit(formData);
		}
	}
</script>

<form onsubmit={handleSubmit} class="space-y-4">
	<div class="flex items-center justify-between mb-6">
		<div class="flex items-center gap-2">
			<Database class="w-5 h-5 text-blue-600" />
			<h2 class="text-lg font-semibold">Add Connection</h2>
		</div>
		<Button
			type="button"
			variant="ghost"
			size="sm"
			onclick={onCancel}
			class="p-1"
		>
			<X class="w-4 h-4" />
		</Button>
	</div>

	<div class="space-y-4">
		<div>
			<label for="name" class="block text-sm font-medium text-gray-700 mb-1">
				Connection Name *
			</label>
			<Input
				id="name"
				type="text"
				bind:value={formData.name}
				placeholder="e.g., Local Development"
				class={errors.name ? 'border-red-500' : ''}
			/>
			{#if errors.name}
				<p class="text-sm text-red-600 mt-1">{errors.name}</p>
			{/if}
		</div>

		<div class="grid grid-cols-3 gap-3">
			<div class="col-span-2">
				<label for="host" class="block text-sm font-medium text-gray-700 mb-1">
					Host *
				</label>
				<Input
					id="host"
					type="text"
					bind:value={formData.host}
					placeholder="localhost"
					class={errors.host ? 'border-red-500' : ''}
				/>
				{#if errors.host}
					<p class="text-sm text-red-600 mt-1">{errors.host}</p>
				{/if}
			</div>

			<div>
				<label for="port" class="block text-sm font-medium text-gray-700 mb-1">
					Port *
				</label>
				<Input
					id="port"
					type="number"
					bind:value={formData.port}
					min="1"
					max="65535"
					class={errors.port ? 'border-red-500' : ''}
				/>
				{#if errors.port}
					<p class="text-sm text-red-600 mt-1">{errors.port}</p>
				{/if}
			</div>
		</div>

		<div>
			<label for="database" class="block text-sm font-medium text-gray-700 mb-1">
				Database *
			</label>
			<Input
				id="database"
				type="text"
				bind:value={formData.database}
				placeholder="my_database"
				class={errors.database ? 'border-red-500' : ''}
			/>
			{#if errors.database}
				<p class="text-sm text-red-600 mt-1">{errors.database}</p>
			{/if}
		</div>

		<div>
			<label for="username" class="block text-sm font-medium text-gray-700 mb-1">
				Username *
			</label>
			<Input
				id="username"
				type="text"
				bind:value={formData.username}
				placeholder="postgres"
				class={errors.username ? 'border-red-500' : ''}
			/>
			{#if errors.username}
				<p class="text-sm text-red-600 mt-1">{errors.username}</p>
			{/if}
		</div>

		<div>
			<label for="password" class="block text-sm font-medium text-gray-700 mb-1">
				Password
			</label>
			<Input
				id="password"
				type="password"
				bind:value={formData.password}
				placeholder="••••••••"
			/>
		</div>
	</div>

	<div class="flex gap-2 pt-4">
		<Button type="submit" class="flex-1">
			Add Connection
		</Button>
		<Button type="button" variant="outline" onclick={onCancel}>
			Cancel
		</Button>
	</div>
</form> 