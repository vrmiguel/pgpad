import { EditorState } from '@codemirror/state';
import type { Script } from '$lib/commands.svelte';
import { SvelteSet } from 'svelte/reactivity';

interface BaseTab {
	id: string;
	type: TabType;
	title: string;
	isDirty: boolean;
	canClose: boolean;
	canRename: boolean;
}

export interface ScriptTab extends BaseTab {
	type: 'script';
	// Negative for new, unsaved scripts.
	scriptId: number;
	script: Script;
	content: string;
	editorState?: EditorState;
	isNewScript: boolean;
}

interface TableViewTab extends BaseTab {
	type: 'table-view';
	tableName: string;
	schema: string;
	connectionId: string;
}

type TabType = 'script' | 'table-view';
type AnyTab = ScriptTab | TableViewTab;

export type SidebarTabState = 'connections' | 'items' | 'scripts' | 'history';

interface SessionData {
	nextTempId?: number;
	tempScripts?: Array<{
		id: number;
		name: string;
		content: string;
	}>;
	openScriptIds?: number[];
	activeScriptId?: number | null;
	unsavedChanges?: Record<string, string>;
}

interface TabStore {
	tabs: AnyTab[];
	activeTabId: string | null;
	scripts: Script[];
	newScripts: SvelteSet<number>;
	nextTempId: number;
	currentEditorContent: string;
	sqlEditorRef: {
		saveState(): EditorState | undefined;
		restoreState(state: EditorState): void;
		setContent(content: string): void;
	} | null;
}

const tabStore = $state<TabStore>({
	tabs: [],
	activeTabId: null,
	scripts: [],
	newScripts: new SvelteSet<number>(),
	nextTempId: -1,
	currentEditorContent: '',
	sqlEditorRef: null
});

let shouldSaveSession = false;
const sessionSaveCallbacks: (() => void)[] = [];

function markSessionDirty() {
	shouldSaveSession = true;
	sessionSaveCallbacks.forEach((callback) => callback());
}

function findScriptTab(scriptId: number): ScriptTab | undefined {
	return tabStore.tabs.find(
		(tab) => tab.type === 'script' && (tab as ScriptTab).scriptId === scriptId
	) as ScriptTab | undefined;
}

export const tabs = {
	get all(): AnyTab[] {
		return tabStore.tabs;
	},
	get active(): AnyTab | null {
		return tabStore.tabs.find((t) => t.id === tabStore.activeTabId) || null;
	},
	get activeId(): string | null {
		return tabStore.activeTabId;
	},
	get scripts(): Script[] {
		return tabStore.scripts;
	},
	get newScripts(): SvelteSet<number> {
		return tabStore.newScripts;
	},
	get currentEditorContent(): string {
		return tabStore.currentEditorContent;
	},
	get nextTempId(): number {
		return tabStore.nextTempId;
	},
	set nextTempId(value: number) {
		tabStore.nextTempId = value;
	},

	openScript(script: Script): void {
		const tabId = `script-${script.id}`;

		let existingTab = tabStore.tabs.find((t) => t.id === tabId) as ScriptTab;
		if (!existingTab) {
			const scriptTab: ScriptTab = {
				id: tabId,
				type: 'script',
				scriptId: script.id,
				title: script.name,
				isDirty: false,
				canClose: true,
				canRename: true,
				script: script,
				content: script.query_text,
				isNewScript: tabStore.newScripts.has(script.id)
			};
			tabStore.tabs.push(scriptTab);
			existingTab = scriptTab;
		}

		tabStore.activeTabId = tabId;
		existingTab.content = script.query_text;
		tabStore.currentEditorContent = script.query_text;

		markSessionDirty();
	},

	switchToTab(tabId: string, skipSaveCurrentContent = false): void {
		const tab = tabStore.tabs.find((t) => t.id === tabId);
		if (!tab) return;

		const currentTab = this.active as ScriptTab;
		if (currentTab?.type === 'script' && tabStore.sqlEditorRef && !skipSaveCurrentContent) {
			currentTab.content = tabStore.currentEditorContent;

			const state = tabStore.sqlEditorRef.saveState();
			if (state) {
				currentTab.editorState = state;
			}
		}

		tabStore.activeTabId = tabId;

		if (tab.type === 'script') {
			const scriptTab = tab as ScriptTab;
			const content = scriptTab.content || scriptTab.script.query_text;

			tabStore.currentEditorContent = content;

			if (tabStore.sqlEditorRef && scriptTab.editorState) {
				tabStore.sqlEditorRef.restoreState(scriptTab.editorState);
			}
		}

		markSessionDirty();
	},

	closeTab(tabId: string): void {
		const tab = tabStore.tabs.find((t) => t.id === tabId);
		if (!tab) return;

		tabStore.tabs = tabStore.tabs.filter((t) => t.id !== tabId);

		if (tab.type === 'script') {
			const scriptTab = tab as ScriptTab;
			tabStore.newScripts.delete(scriptTab.scriptId);
		}

		if (tabStore.activeTabId === tabId) {
			if (tabStore.tabs.length > 0) {
				const lastTab = tabStore.tabs[tabStore.tabs.length - 1];
				this.switchToTab(lastTab.id);
			} else {
				tabStore.activeTabId = null;
				tabStore.currentEditorContent = '';
				if (tabStore.sqlEditorRef) {
					tabStore.sqlEditorRef.setContent('');
				}
			}
		}

		markSessionDirty();
	},

	handleEditorContentChange(newContent: string): void {
		tabStore.currentEditorContent = newContent;
		const activeTab = this.active;
		if (activeTab?.type === 'script') {
			const scriptTab = activeTab as ScriptTab;
			scriptTab.content = newContent;

			const originalContent = scriptTab.script.query_text;
			const shouldShowUnsaved = scriptTab.isNewScript
				? newContent.length > 0
				: newContent !== originalContent;

			scriptTab.isDirty = shouldShowUnsaved;
		}
	},

	markScriptSaved(scriptId: number, newContent: string): void {
		const scriptTab = findScriptTab(scriptId);

		if (scriptTab) {
			scriptTab.script.query_text = newContent;
			scriptTab.content = newContent;

			if (scriptTab.isNewScript && tabStore.newScripts.has(scriptId)) {
				scriptTab.isNewScript = false;
			}

			scriptTab.isDirty = false;
		}
	},

	updateScriptId(oldScriptId: number, newScriptId: number, updatedScript: Script): void {
		const oldTabId = `script-${oldScriptId}`;
		const newTabId = `script-${newScriptId}`;

		const scriptTab = tabStore.tabs.find((tab) => tab.type === 'script' && tab.id === oldTabId) as
			| ScriptTab
			| undefined;

		if (scriptTab) {
			scriptTab.id = newTabId;
			scriptTab.scriptId = newScriptId;
			scriptTab.script = updatedScript;
			scriptTab.isNewScript = false;

			if (tabStore.activeTabId === oldTabId) {
				tabStore.activeTabId = newTabId;
			}
		}
	},

	createNewScript(): void {
		this.createScriptFromHistory('');
	},

	createScriptFromHistory(historyQuery: string): void {
		const tempId = tabStore.nextTempId--;

		const existingUntitled = tabStore.scripts.filter((s) =>
			s.name.startsWith('Untitled Script')
		).length;
		const name =
			existingUntitled === 0 ? 'Untitled Script' : `Untitled Script ${existingUntitled + 1}`;

		const newScript: Script = {
			id: tempId,
			name,
			description: null,
			query_text: historyQuery,
			connection_id: null,
			tags: null,
			created_at: Date.now() / 1000,
			updated_at: Date.now() / 1000,
			favorite: false
		};

		tabStore.scripts.push(newScript);
		tabStore.newScripts.add(tempId);
		this.openScript(newScript);
		markSessionDirty();
	},

	renameScript(tabId: string, newName: string): void {
		const tab = tabStore.tabs.find((t) => t.id === tabId);
		if (tab?.type === 'script') {
			const scriptTab = tab as ScriptTab;
			scriptTab.title = newName;
			scriptTab.script.name = newName;

			const scriptIndex = tabStore.scripts.findIndex((s) => s.id === scriptTab.scriptId);
			if (scriptIndex !== -1) {
				tabStore.scripts[scriptIndex].name = newName;
			}

			markSessionDirty();
		}
	},

	openTableExplorationTab(tableName: string, schema: string, connectionId: string): void {
		const tabId = `table-${connectionId}-${schema}-${tableName}`;

		if (tabStore.tabs.find((t) => t.id === tabId)) {
			this.switchToTab(tabId);
			return;
		}

		const tableTab: TableViewTab = {
			id: tabId,
			type: 'table-view',
			title: `${schema}.${tableName}`,
			isDirty: false,
			canClose: true,
			canRename: false,
			tableName,
			schema,
			connectionId
		};

		tabStore.tabs.push(tableTab);
		tabStore.activeTabId = tabId;
		markSessionDirty();
	},

	async saveSession(saveSessionCallback: (data: SessionData) => Promise<void>): Promise<void> {
		const scriptTabs = tabStore.tabs.filter((t) => t.type === 'script') as ScriptTab[];

		const sessionData = {
			openTabIds: tabStore.tabs.map((t) => t.id),
			activeTabId: tabStore.activeTabId,
			openScriptIds: scriptTabs.map((t) => t.scriptId),
			activeScriptId: scriptTabs.find((t) => t.id === tabStore.activeTabId)?.scriptId || null,
			unsavedChanges: Object.fromEntries(
				scriptTabs
					.filter((t) => !t.isNewScript && t.content !== t.script.query_text)
					.map((t) => [t.scriptId, t.content])
			),
			tempScripts: scriptTabs
				.filter((t) => t.isNewScript)
				.map((t) => ({
					id: t.scriptId,
					name: t.script.name,
					content: t.content
				})),
			nextTempId: tabStore.nextTempId
		};

		await saveSessionCallback(sessionData);
		shouldSaveSession = false;
	},

	async restoreSession(sessionData: SessionData | null): Promise<boolean> {
		if (!sessionData) return false;

		if (sessionData.nextTempId !== undefined) {
			tabStore.nextTempId = Math.min(tabStore.nextTempId, sessionData.nextTempId);
		}

		for (const temp of sessionData.tempScripts ?? []) {
			if (!tabStore.scripts.find((s) => s.id === temp.id)) {
				tabStore.scripts.push({
					id: temp.id,
					name: temp.name,
					description: null,
					query_text: temp.content,
					connection_id: null,
					tags: null,
					created_at: Date.now() / 1000,
					updated_at: Date.now() / 1000,
					favorite: false
				});
			}
			tabStore.newScripts.add(temp.id);
		}

		for (const scriptId of sessionData.openScriptIds ?? []) {
			const script = tabStore.scripts.find((s) => s.id === scriptId);
			if (script) {
				this.openScript(script);

				const tabId = `script-${scriptId}`;
				const tab = tabStore.tabs.find((t) => t.id === tabId) as ScriptTab;
				if (tab && sessionData.unsavedChanges?.[scriptId]) {
					tab.content = sessionData.unsavedChanges[scriptId];
					tab.isDirty = true;
				}
			}
		}

		if (sessionData.activeScriptId != null) {
			const tabId = `script-${sessionData.activeScriptId}`;
			if (tabStore.tabs.find((t) => t.id === tabId)) {
				this.switchToTab(tabId, true);
			}
		}

		return (sessionData.openScriptIds?.length ?? 0) > 0;
	},

	setScripts(scripts: Script[]): void {
		tabStore.scripts = scripts;
	},

	setSqlEditorRef(ref: TabStore['sqlEditorRef']): void {
		tabStore.sqlEditorRef = ref;
	},

	onSessionSave(callback: () => void): void {
		sessionSaveCallbacks.push(callback);
	},

	get shouldSaveSession(): boolean {
		return shouldSaveSession;
	},

	clearSessionDirty(): void {
		shouldSaveSession = false;
	}
};
