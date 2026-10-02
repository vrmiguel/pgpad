import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { undo } from '@codemirror/commands';
import { EditorView, keymap } from '@codemirror/view';
import { closeCompletion, currentCompletions, startCompletion } from '@codemirror/autocomplete';
import { get } from 'svelte/store';
import SqlEditor from './SqlEditor.svelte';
import * as editors from '$lib/codemirror';
import { Commands, type ConnectionInfo, type DatabaseSchema } from '$lib/commands.svelte';
import { theme } from '$lib/stores/theme';
import { fontSize } from '$lib/stores/fontSize';

const emptySchema: DatabaseSchema = { tables: [], schemas: [], unique_columns: [] };
const connection: ConnectionInfo = {
	id: 'connection-1',
	name: 'Test',
	connected: true,
	permissions: 'read_write',
	config: { SQLite: { db_path: ':memory:' } }
};

let editorVisible = true;
let components: ReturnType<typeof mount>[] = [];
let standaloneEditors: ReturnType<typeof editors.createEditor>[] = [];
let originalTheme = get(theme);
let originalFontSize: number;

function mountEditor(connected = false) {
	const target = document.createElement('div');
	document.body.append(target);
	const component = mount(SqlEditor, {
		target,
		props: {
			selectedConnection: connected ? connection.id : null,
			connections: connected ? [connection] : [],
			currentScript: null,
			hasUnsavedChanges: false
		}
	});
	components.push(component);
	flushSync();
	return component;
}

async function removeEditor(component: ReturnType<typeof mount>) {
	await unmount(component);
	components = components.filter((item) => item !== component);
	flushSync();
}

beforeEach(() => {
	originalTheme = get(theme);
	originalFontSize = get(fontSize);
	// happy-dom has no layout; expose the editor container as visible.
	editorVisible = true;
	Object.defineProperty(HTMLElement.prototype, 'offsetParent', {
		configurable: true,
		get: () => (editorVisible ? document.body : null)
	});
	vi.spyOn(Commands, 'getDatabaseSchema').mockResolvedValue(emptySchema);
});

afterEach(async () => {
	for (const component of components) await unmount(component);
	components = [];
	for (const editor of standaloneEditors) editor.dispose();
	standaloneEditors = [];
	theme.set(originalTheme);
	fontSize.set(originalFontSize);
	vi.useRealTimers();
	vi.restoreAllMocks();
	Reflect.deleteProperty(HTMLElement.prototype, 'offsetParent');
	document.body.replaceChildren();
});

describe('SqlEditor lifecycle', () => {
	it('does not accumulate CSS rules when restoring and reusing theme or font settings', () => {
		const editor = editors.createEditor({ container: document.body, value: 'SELECT 1' });
		standaloneEditors.push(editor);
		const savedState = editor.saveState();
		// First use can mount styles, reusing the same settings should not are more
		theme.set('dark');
		fontSize.set(19);
		theme.set('light');
		fontSize.set(13);
		editor.restoreState(savedState);
		const ruleCount = () =>
			[...new Set([...document.styleSheets, ...document.adoptedStyleSheets])].reduce(
				(count, sheet) => count + sheet.cssRules.length,
				0
			);
		const initialCount = ruleCount();
		expect(initialCount).toBeGreaterThan(0);
		for (let index = 0; index < 50; index++) {
			theme.set(index % 2 ? 'light' : 'dark');
			fontSize.set(index % 2 ? 13 : 19);
			editor.restoreState(savedState);
		}
		expect(ruleCount()).toBe(initialCount);
	});

	it('does not traverse the schema again when restoring a saved state', () => {
		const schema: DatabaseSchema = {
			...emptySchema,
			tables: [{ name: 'customers', schema: 'public', columns: [] }]
		};
		const readTables = vi.fn(() => schema.tables);
		const observedSchema: DatabaseSchema = {
			...emptySchema,
			get tables() {
				return readTables();
			}
		};
		const editor = editors.createEditor({
			container: document.body,
			value: '',
			schema: observedSchema
		});
		standaloneEditors.push(editor);
		expect(readTables).toHaveBeenCalled();
		const savedState = editor.saveState();
		readTables.mockClear();
		for (let index = 0; index < 50; index++) editor.restoreState(savedState);
		expect(readTables).not.toHaveBeenCalled();
		editor.updateSchema(observedSchema);
		expect(readTables).toHaveBeenCalled();
		readTables.mockClear();
		editor.restoreState(savedState);
		expect(readTables).not.toHaveBeenCalled();
	});

	it('destroys every unmounted view and stops theme/font updates reaching it', async () => {
		const create = vi.spyOn(editors, 'createEditor');
		for (let index = 0; index < 20; index++) {
			const component = mountEditor();
			const editor = create.mock.results.at(-1)!.value as ReturnType<typeof editors.createEditor>;
			const destroy = vi.spyOn(editor.view, 'destroy');
			const dispatch = vi.spyOn(editor.view, 'dispatch');
			await removeEditor(component);
			expect(destroy).toHaveBeenCalledTimes(1);
			dispatch.mockClear();
			theme.set(index % 2 ? 'light' : 'dark');
			fontSize.set(13 + (index % 2));
			expect(dispatch).not.toHaveBeenCalled();
		}
		expect(create).toHaveBeenCalledTimes(20);
	});

	it('cancels initialization when unmounted before becoming visible', async () => {
		vi.useFakeTimers();
		editorVisible = false;
		const create = vi.spyOn(editors, 'createEditor');
		const component = mountEditor();
		await vi.advanceTimersByTimeAsync(300);
		expect(create).not.toHaveBeenCalled();
		await removeEditor(component);
		editorVisible = true;
		await vi.advanceTimersByTimeAsync(500);
		expect(create).not.toHaveBeenCalled();
		expect(vi.getTimerCount()).toBe(0);
	});

	it.each([false, true])(
		'applies a late schema response only while mounted (unmounted: %s)',
		async (unmounted) => {
			let resolveSchema!: (schema: DatabaseSchema) => void;
			vi.mocked(Commands.getDatabaseSchema).mockReturnValue(
				new Promise((resolve) => {
					resolveSchema = resolve;
				})
			);
			const create = vi.spyOn(editors, 'createEditor');
			const component = mountEditor(true);
			const editor = create.mock.results.at(-1)!.value as ReturnType<typeof editors.createEditor>;
			const updateSchema = vi.spyOn(editor, 'updateSchema');
			expect(Commands.getDatabaseSchema).toHaveBeenCalled();
			if (unmounted) await removeEditor(component);
			resolveSchema(emptySchema);
			// Let all async loader continuations run before checking the negative case
			await new Promise<void>((resolve) => setTimeout(resolve, 0));
			if (unmounted) {
				expect(updateSchema).not.toHaveBeenCalled();
			} else {
				expect(updateSchema).toHaveBeenCalledWith(emptySchema);
			}
		}
	);

	it('rebinds saved-state callbacks to the current editor while preserving undo', () => {
		const oldChange = vi.fn();
		const oldRun = vi.fn();
		const oldEditor = editors.createEditor({
			container: document.body,
			value: 'SELECT 1',
			onChange: oldChange,
			onExecute: oldRun
		});
		oldEditor.view.dispatch({ changes: { from: 8, insert: '0' } });
		const savedState = oldEditor.saveState();
		oldEditor.dispose();
		oldChange.mockClear();
		const oldRunBinding = savedState
			.facet(keymap)
			.flat()
			.find((binding) => binding.key === 'Ctrl-r')!;
		oldRunBinding.run!(oldEditor.view);
		expect(oldRun).not.toHaveBeenCalled();

		const newChange = vi.fn();
		const newRun = vi.fn();
		const editor = editors.createEditor({
			container: document.body,
			value: '',
			onChange: newChange,
			onExecute: newRun
		});
		standaloneEditors.push(editor);
		editor.restoreState(savedState);
		expect(editor.view.state.doc.toString()).toBe('SELECT 10');
		expect(undo(editor.view)).toBe(true);
		expect(editor.view.state.doc.toString()).toBe('SELECT 1');
		expect(newChange).toHaveBeenCalledWith('SELECT 1');
		expect(oldChange).not.toHaveBeenCalled();
		const run = editor.view.state
			.facet(keymap)
			.flat()
			.find((binding) => binding.key === 'Ctrl-r')!;
		run.run!(editor.view);
		expect(newRun).toHaveBeenCalledTimes(1);
		expect(oldRun).not.toHaveBeenCalled();
		editor.updateDisabled(true);
		expect(editor.view.state.readOnly).toBe(true);
	});

	it('restores current settings and keeps theme, font and schema updates working', async () => {
		const schema = (name: string): DatabaseSchema => ({
			...emptySchema,
			tables: [{ name, schema: 'public', columns: [] }]
		});
		theme.set('light');
		fontSize.set(13);
		const oldEditor = editors.createEditor({
			container: document.body,
			value: 'ze',
			schema: schema('zebra_old')
		});
		oldEditor.view.dispatch({ selection: { anchor: 2 } });
		const savedState = oldEditor.saveState();
		oldEditor.dispose();

		const editor = editors.createEditor({ container: document.body, value: '' });
		standaloneEditors.push(editor);
		editor.updateDisabled(true);
		editor.updateSchema(schema('zebra_current'));
		theme.set('dark');
		fontSize.set(19);
		editor.restoreState(savedState);
		expect(editor.view.state.readOnly).toBe(true);
		expect(editor.view.state.facet(EditorView.darkTheme)).toBe(true);
		expect(getComputedStyle(editor.view.contentDOM).fontSize).toBe('19px');
		// Reconfiguring the restored state should not change the saved snapshot
		expect(savedState.readOnly).toBe(false);
		expect(savedState.facet(EditorView.darkTheme)).toBe(false);

		editor.updateDisabled(false);
		expect(editor.view.state.readOnly).toBe(false);
		startCompletion(editor.view);
		await vi.waitFor(() => {
			expect(currentCompletions(editor.view.state).map((item) => item.label)).toContain(
				'zebra_current'
			);
		});
		expect(currentCompletions(editor.view.state).map((item) => item.label)).not.toContain(
			'zebra_old'
		);
		closeCompletion(editor.view);

		theme.set('light');
		fontSize.set(23);
		expect(editor.view.state.facet(EditorView.darkTheme)).toBe(false);
		expect(getComputedStyle(editor.view.contentDOM).fontSize).toBe('23px');
		editor.updateSchema(schema('zebra_next'));
		startCompletion(editor.view);
		await vi.waitFor(() => {
			expect(currentCompletions(editor.view.state).map((item) => item.label)).toContain(
				'zebra_next'
			);
		});
		expect(currentCompletions(editor.view.state).map((item) => item.label)).not.toContain(
			'zebra_current'
		);
	});
});
