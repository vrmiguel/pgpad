import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Script } from '$lib/commands.svelte';

interface DisplayTab {
	id: string;
	name: string;
	isDirty: boolean;
}

interface TabBarProps {
	tabs: DisplayTab[];
	activeTabId: string | null;
	onTabSelect: (id: string) => void;
	onTabClose: (id: string) => void;
	onTabRename: (id: string, name: string) => void;
	getTabStatus: (tab: DisplayTab) => 'normal' | 'modified' | 'error';
}

// Capture the rendered child boundary so these tests exercise ScriptTabs' real
// projection and callbacks without requiring a DOM or native desktop runtime.
const { captureTabBar } = vi.hoisted(() => ({
	captureTabBar: vi.fn<(_renderer: unknown, props: TabBarProps) => void>()
}));

vi.mock('./ui/TabBar.svelte', () => ({ default: captureTabBar }));
vi.mock('$lib/backend', () => ({ backend: { listen: vi.fn() } }));

describe('ScriptTabs identity', () => {
	let tabs: typeof import('$lib/stores/tabs.svelte').tabs;
	let render: typeof import('svelte/server').render;
	let ScriptTabs: typeof import('./ScriptTabs.svelte').default;

	beforeEach(async () => {
		vi.resetModules();
		captureTabBar.mockClear();
		({ tabs } = await import('$lib/stores/tabs.svelte'));
		({ render } = await import('svelte/server'));
		({ default: ScriptTabs } = await import('./ScriptTabs.svelte'));

		const script: Script = {
			id: 1,
			name: 'Saved script',
			query_text: 'SELECT 1',
			description: null,
			connection_id: null,
			tags: null,
			created_at: 0,
			updated_at: 0,
			favorite: false
		};
		tabs.setScripts([script]);
		tabs.openScript(script);
		tabs.handleEditorContentChange('SELECT 2');
		tabs.openTableExplorationTab('users', 'public', 'connection-1');
	});

	function renderTabs(): TabBarProps {
		// Read the lazy render output to evaluate the child component.
		void render(ScriptTabs).body;
		return captureTabBar.mock.lastCall![1];
	}

	it('preserves distinct IDs and statuses for a script and table with the same numeric suffix', () => {
		const view = renderTabs();

		expect(view.tabs.map((tab) => tab.id)).toEqual(['script-1', 'table-1']);
		expect(view.activeTabId).toBe('table-1');
		expect(view.tabs.map(view.getTabStatus)).toEqual(['modified', 'normal']);
	});

	it('selects and closes each tab independently through the tab-bar callbacks', () => {
		const view = renderTabs();

		view.onTabSelect('script-1');
		expect(tabs.activeId).toBe('script-1');
		expect(renderTabs().activeTabId).toBe('script-1');

		view.onTabSelect('table-1');
		expect(tabs.activeId).toBe('table-1');

		view.onTabClose('table-1');
		expect(tabs.all.map((tab) => tab.id)).toEqual(['script-1']);
		expect(tabs.activeId).toBe('script-1');

		view.onTabClose('script-1');
		expect(tabs.all).toEqual([]);
		expect(tabs.activeId).toBeNull();
	});

	it('renames by full script ID without affecting the table with the same suffix', () => {
		const view = renderTabs();

		view.onTabRename('script-1', 'Renamed script');
		view.onTabRename('table-1', 'Not a script');

		expect(renderTabs().tabs.map((tab) => tab.name)).toEqual(['Renamed script', '📋 public.users']);
		expect(tabs.scripts[0].id).toBe(1);
		expect(tabs.scripts[0].name).toBe('Renamed script');
	});

	it('keeps the shared TabBar compatible with numeric query-result IDs', async () => {
		const { default: TabBar } =
			await vi.importActual<typeof import('./ui/TabBar.svelte')>('./ui/TabBar.svelte');
		const getTabStatus = vi.fn(() => 'normal' as const);
		const { body } = render(TabBar, {
			props: {
				tabs: [
					{ id: 1, name: 'First result' },
					{ id: 2, name: 'Second result' }
				],
				activeTabId: 2,
				onTabSelect: vi.fn(),
				getTabStatus
			}
		});

		expect(body).toContain('First result');
		expect(body).toContain('Second result');
		expect(getTabStatus.mock.calls).toEqual([
			[{ id: 1, name: 'First result' }],
			[{ id: 2, name: 'Second result' }]
		]);
	});
});
