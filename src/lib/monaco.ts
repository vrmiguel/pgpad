import * as monaco from 'monaco-editor';

import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import cssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker';
import htmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker';
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker';
import tsWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker';

self.MonacoEnvironment = {
	getWorker: function (_: string, label: string) {
		switch (label) {
			case 'json':
				return new jsonWorker();
			case 'css':
			case 'scss':
			case 'less':
				return new cssWorker();
			case 'html':
			case 'handlebars':
			case 'razor':
				return new htmlWorker();
			case 'typescript':
			case 'javascript':
				return new tsWorker();
			default:
				return new editorWorker();
		}
	}
};

export interface CreateMonacoEditorOptions {
	container: HTMLElement;
	value: string;
	onChange?: (value: string) => void;
	onExecute?: () => void;
	onExecuteSelection?: (selectedText: string) => void;
	disabled?: boolean;
	theme?: 'light' | 'dark';
}

export function createMonacoEditor(options: CreateMonacoEditorOptions) {
	const { container, value, onChange, onExecute, onExecuteSelection, disabled = false, theme = 'light' } = options;

	monaco.languages.registerCompletionItemProvider('sql', {
		provideCompletionItems: (model, position, context, token) => {
			const word = model.getWordUntilPosition(position);
			const range = {
				startLineNumber: position.lineNumber,
				endLineNumber: position.lineNumber,
				startColumn: word.startColumn,
				endColumn: word.endColumn
			};

			const suggestions: monaco.languages.CompletionItem[] = [
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
					label: 'information_schema.columns',
					kind: monaco.languages.CompletionItemKind.Module,
					insertText: 'information_schema.columns',
					documentation: 'PostgreSQL system view for column information',
					range
				},
				{
					label: 'information_schema.tables',
					kind: monaco.languages.CompletionItemKind.Module,
					insertText: 'information_schema.tables',
					documentation: 'PostgreSQL system view for table information',
					range
				}
			];

			return { suggestions };
		}
	});

	const editor = monaco.editor.create(container, {
		value,
		language: 'sql',
		theme: theme === 'dark' ? 'vs-dark' : 'vs',
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
		dispose: () => editor.dispose()
	};
}

export default monaco; 