import * as monaco from 'monaco-editor';
import type { DatabaseSchema, TableInfo, ColumnInfo } from './commands.svelte';
import { theme as themeStore, registerMonacoThemeCallback } from './stores/theme';
import { get } from 'svelte/store';

import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';

self.MonacoEnvironment = {
	getWorker: function (_workerId: string, _label: string) {
		return new editorWorker();
	}
};

function generateSchemaCompletions(schema: DatabaseSchema | null, word: any, range: any): monaco.languages.CompletionItem[] {
	if (!schema) {
		return [];
	}

	const suggestions: monaco.languages.CompletionItem[] = [];

	for (const table of schema.tables) {
		const tableName = table.schema === 'public' ? table.name : `${table.schema}.${table.name}`;
		suggestions.push({
			label: tableName,
			kind: monaco.languages.CompletionItemKind.Class,
			insertText: tableName,
			documentation: `Table: ${tableName} (${table.columns.length} columns)`,
			range,
			detail: 'table'
		});

		for (const column of table.columns) {
			suggestions.push({
				label: `${tableName}.${column.name}`,
				kind: monaco.languages.CompletionItemKind.Field,
				insertText: column.name,
				documentation: `Column: ${column.name} (${column.data_type})${column.is_nullable ? ', nullable' : ', not null'}`,
				range,
				detail: `${tableName} column`,
				filterText: `${tableName} ${column.name}`
			});

			suggestions.push({
				label: column.name,
				kind: monaco.languages.CompletionItemKind.Field,
				insertText: column.name,
				documentation: `Column: ${column.name} (${column.data_type}) from ${tableName}`,
				range,
				detail: 'column'
			});
		}
	}

	for (const schemaName of schema.schemas) {
		if (schemaName !== 'public') {
			suggestions.push({
				label: schemaName,
				kind: monaco.languages.CompletionItemKind.Module,
				insertText: schemaName,
				documentation: `Schema: ${schemaName}`,
				range,
				detail: 'schema'
			});
		}
	}

	return suggestions;
}

export interface CreateMonacoEditorOptions {
	container: HTMLElement;
	value: string;
	onChange?: (value: string) => void;
	onExecute?: () => void;
	onExecuteSelection?: (selectedText: string) => void;
	disabled?: boolean;
	schema?: DatabaseSchema | null;
}

export function createMonacoEditor(options: CreateMonacoEditorOptions) {
	const { container, value, onChange, onExecute, onExecuteSelection, disabled = false, schema = null } = options;

	let currentTheme: 'light' | 'dark' = get(themeStore);
	const unsubscribeTheme = themeStore.subscribe(t => currentTheme = t);

	let currentSchema = schema;

	const completionProvider = monaco.languages.registerCompletionItemProvider('sql', {
		provideCompletionItems: (model, position, context, token) => {
			const word = model.getWordUntilPosition(position);
			const range = {
				startLineNumber: position.lineNumber,
				endLineNumber: position.lineNumber,
				startColumn: word.startColumn,
				endColumn: word.endColumn
			};

			// Static SQL keyword suggestions
			const staticSuggestions: monaco.languages.CompletionItem[] = [
				{
					label: 'SELECT',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'SELECT ',
					documentation: 'SELECT statement',
					range
				},
				{
					label: 'FROM',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'FROM ',
					documentation: 'FROM clause',
					range
				},
				{
					label: 'WHERE',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'WHERE ',
					documentation: 'WHERE clause',
					range
				},
				{
					label: 'INSERT INTO',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'INSERT INTO ',
					documentation: 'INSERT statement',
					range
				},
				{
					label: 'UPDATE',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'UPDATE ',
					documentation: 'UPDATE statement',
					range
				},
				{
					label: 'DELETE FROM',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'DELETE FROM ',
					documentation: 'DELETE statement',
					range
				},
				{
					label: 'CREATE TABLE',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'CREATE TABLE ',
					documentation: 'CREATE TABLE statement',
					range
				},
				{
					label: 'DROP TABLE',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'DROP TABLE ',
					documentation: 'DROP TABLE statement',
					range
				},
				{
					label: 'ORDER BY',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'ORDER BY ',
					documentation: 'ORDER BY clause',
					range
				},
				{
					label: 'GROUP BY',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'GROUP BY ',
					documentation: 'GROUP BY clause',
					range
				},
				{
					label: 'HAVING',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'HAVING ',
					documentation: 'HAVING clause',
					range
				},
				{
					label: 'JOIN',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'JOIN ',
					documentation: 'JOIN clause',
					range
				},
				{
					label: 'LEFT JOIN',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'LEFT JOIN ',
					documentation: 'LEFT JOIN clause',
					range
				},
				{
					label: 'RIGHT JOIN',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'RIGHT JOIN ',
					documentation: 'RIGHT JOIN clause',
					range
				},
				{
					label: 'INNER JOIN',
					kind: monaco.languages.CompletionItemKind.Keyword,
					insertText: 'INNER JOIN ',
					documentation: 'INNER JOIN clause',
					range
				}
			];

			const schemaSuggestions = generateSchemaCompletions(currentSchema, word, range);

			const allSuggestions = [...staticSuggestions, ...schemaSuggestions];

			return { suggestions: allSuggestions };
		}
	});

	const editor = monaco.editor.create(container, {
		value,
		language: 'sql',
		theme: currentTheme === 'dark' ? 'vs-dark' : 'vs',
		wordWrap: 'on',
		lineNumbers: 'on',
		minimap: {
			enabled: false
		},
		fontSize: 14,
		lineHeight: 20,
		scrollBeyondLastLine: false,
		automaticLayout: true,
		tabSize: 2,
		insertSpaces: true,
		folding: true,
		foldingStrategy: 'indentation',
		showFoldingControls: 'mouseover',
		// SQL specific options
		suggest: {
			showKeywords: true,
			showSnippets: true
		},
		quickSuggestions: {
			other: true,
			comments: false,
			strings: false
		},
		readOnly: disabled
	});

	// Add keyboard shortcuts
	editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
		// If user selected some section, execute just that
		const selection = editor.getSelection();
		if (selection && !selection.isEmpty()) {
			const selectedText = editor.getModel()?.getValueInRange(selection);
			if (selectedText && selectedText.trim()) {
				onExecuteSelection?.(selectedText.trim());
				return;
			}
		}

		onExecute?.();
	});

	// Keep Ctrl+R for full execution
	editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyR, () => {
		onExecute?.();
	});

	// Listen for content changes
	let isUpdatingFromProps = false;
	editor.onDidChangeModelContent(() => {
		if (!isUpdatingFromProps) {
			onChange?.(editor.getValue());
		}
	});

	// Update editor when external value changes
	const updateValue = (newValue: string) => {
		if (editor.getValue() !== newValue) {
			isUpdatingFromProps = true;
			editor.setValue(newValue);
			isUpdatingFromProps = false;
		}
	};

	// Update disabled state
	const updateDisabled = (isDisabled: boolean) => {
		editor.updateOptions({ readOnly: isDisabled });
	};

	// Update theme
	const updateTheme = (newTheme: 'light' | 'dark') => {
		monaco.editor.setTheme(newTheme === 'dark' ? 'vs-dark' : 'vs');
	};

	const unregisterThemeCallback = registerMonacoThemeCallback(updateTheme);

	const getExecutableText = () => {
		const selection = editor.getSelection();
		if (selection && !selection.isEmpty()) {
			// Return selected text
			return editor.getModel()?.getValueInRange(selection)?.trim();
		} else {
			// Return current line if no selection
			const position = editor.getPosition();
			if (position) {
				const lineContent = editor.getModel()?.getLineContent(position.lineNumber);
				return lineContent?.trim();
			}
		}
		return null;
	};

	return {
		editor,
		updateValue,
		updateDisabled,
		updateTheme,
		focus: () => editor.focus(),
		getExecutableText,
		getSelectedText: () => {
			const selection = editor.getSelection();
			if (selection && !selection.isEmpty()) {
				return editor.getModel()?.getValueInRange(selection)?.trim();
			}
			return null;
		},
		updateSchema: (newSchema: DatabaseSchema | null) => {
			currentSchema = newSchema;
		},
		dispose: () => {
			unsubscribeTheme();
			unregisterThemeCallback();
			completionProvider.dispose();
			editor.dispose();
		}
	};
}

export default monaco; 