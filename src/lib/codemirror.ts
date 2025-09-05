import {
	drawSelection,
	dropCursor,
	EditorView,
	highlightSpecialChars,
	lineNumbers,
	rectangularSelection
} from '@codemirror/view';
import { EditorState, type Extension, Compartment, Transaction } from '@codemirror/state';
import { PostgreSQL, sql } from '@codemirror/lang-sql';
import {
	acceptCompletion,
	closeBrackets,
	closeBracketsKeymap,
	closeCompletion,
	moveCompletionSelection
} from '@codemirror/autocomplete';
import { keymap } from '@codemirror/view';
import { indentWithTab, history, historyKeymap, defaultKeymap } from '@codemirror/commands';

import { registerEditorThemeCallback, theme } from './stores/theme';
import { get } from 'svelte/store';
import { TauriLSPTransport } from './lsp-transport';
import { LSPClient, languageServerExtensions } from '@codemirror/lsp-client';

import {
	bracketMatching,
	indentOnInput,
	foldGutter,
	syntaxHighlighting,
	defaultHighlightStyle
} from '@codemirror/language';
import { highlightActiveLine, highlightActiveLineGutter } from '@codemirror/view';
import { highlightSelectionMatches } from '@codemirror/search';

function createTheme(theme: 'light' | 'dark') {
	return EditorView.theme(
		{
			'&': {
				color: theme === 'dark' ? '#d4d4d4' : '#24292e',
				backgroundColor: theme === 'dark' ? '#2c2c2e' : '#ffffff',
				height: '100%',
				overflow: 'hidden',
				fontSize: '13px',
				lineHeight: '1.5',
				margin: '0',
				padding: '0',
				border: 'none',
				boxShadow: 'none'
			},
			'.cm-content': {
				margin: '0', // Ensure no margins
				caretColor: theme === 'dark' ? '#d4d4d4' : '#24292e',
				minHeight: '100%',
				border: 'none'
			},
			'.cm-focused .cm-cursor': {
				borderLeftColor: theme === 'dark' ? '#d4d4d4' : '#24292e'
			},
			'&::selection': {
				backgroundColor: theme === 'dark' ? '#264f78' : '#add6ff',
				color: 'inherit'
			},
			'.cm-selectionBackground': {
				backgroundColor: theme === 'dark' ? '#264f78 !important' : '#add6ff !important'
			},
			'.cm-focused .cm-selectionBackground': {
				backgroundColor: theme === 'dark' ? '#264f78 !important' : '#add6ff !important'
			},
			'.cm-line::selection': {
				backgroundColor: theme === 'dark' ? '#264f78' : '#add6ff'
			},
			'.cm-selectionLayer .cm-selectionBackground': {
				backgroundColor: theme === 'dark' ? '#264f78 !important' : '#add6ff !important'
			},
			'.cm-activeLine': {
				backgroundColor: theme === 'dark' ? 'rgba(255, 255, 255, 0.02)' : 'rgba(0, 0, 0, 0.02)', // Very subtle active line
				position: 'relative',
				zIndex: '1'
			},
			'.cm-line': {
				position: 'relative'
			},
			'.cm-activeLineGutter': {
				backgroundColor: theme === 'dark' ? 'rgba(255, 255, 255, 0.02)' : 'rgba(0, 0, 0, 0.02)' // Very subtle active line gutter
			},
			'.cm-scroller': {
				outline: 'none',
				height: '100%',
				overflow: 'auto',
				margin: '0',
				padding: '0',
				border: 'none',
				boxShadow: 'none'
			},
			'.cm-gutters': {
				backgroundColor: theme === 'dark' ? '#252526' : '#f8f8f8',
				borderRight: `1px solid ${theme === 'dark' ? '#3e3e3e' : '#e1e4e8'}`,
				borderTop: 'none',
				borderBottom: 'none',
				borderLeft: 'none',
				minHeight: '100%',
				fontSize: '13px',
				margin: '0',
				padding: '0',
				borderTopRightRadius: '0',
				borderBottomRightRadius: '0'
			},
			'.cm-gutter': {
				minHeight: '100%'
			},
			'.cm-lineNumbers': {
				minHeight: '100%',
				fontSize: '12px',
				margin: '0',
				padding: '0 0px 0 6px'
			},
			'.cm-editor': {
				outline: 'none !important',
				border: 'none !important',
				height: '100%',
				backgroundColor: 'transparent'
			},
			'.cm-editor.cm-focused': {
				outline: 'none !important',
				border: 'none !important',
				boxShadow: 'none !important'
			},
			'&.cm-focused': {
				outline: 'none !important',
				border: 'none !important'
			},
			'.cm-content:focus': {
				outline: 'none !important'
			},
			'.cm-content:focus-visible': {
				outline: 'none !important'
			}
		},
		{ dark: theme === 'dark' }
	);
}

export interface CreateEditorOptions {
	container: HTMLElement;
	value: string;
	onChange?: (value: string) => void;
	onExecute?: () => void;
	onExecuteSelection?: (selectedText: string) => void;
	disabled?: boolean;
}

export function createEditorInstance(options: CreateEditorOptions) {
	const { container, value, onChange, onExecute, onExecuteSelection, disabled = false } = options;

	// TODO(vini): is this right?
	let currentTheme: 'light' | 'dark' = 'light';
	const $theme = get(theme);
	if ($theme !== 'auto') {
		currentTheme = $theme;
	}

	// Compartments for dynamic reconfiguration
	const themeCompartment = new Compartment();
	const readOnlyCompartment = new Compartment();

	const lspTransport = new TauriLSPTransport();
	const lspClient = new LSPClient({ extensions: languageServerExtensions() }).connect(lspTransport);

	const extensions: Extension[] = [
		keymap.of([
			{
				key: 'Ctrl-Enter',
				mac: 'Cmd-Enter',
				run: (view: EditorView) => {
					const selection = view.state.selection.main;
					if (!selection.empty) {
						const selectedText = view.state.doc.sliceString(selection.from, selection.to);
						if (selectedText.trim()) {
							onExecuteSelection?.(selectedText.trim());
							return true;
						}
					}
					onExecute?.();
					return true;
				}
			},
			// Ctrl+R: Execute full script
			{
				key: 'Ctrl-r',
				mac: 'Cmd-r',
				run: () => {
					onExecute?.();
					return true;
				}
			},
			{
				key: 'ArrowDown',
				run: moveCompletionSelection(true)
			},
			{
				key: 'ArrowUp',
				run: moveCompletionSelection(false)
			},
			{
				key: 'Enter',
				run: acceptCompletion
			},
			{
				key: 'Tab',
				run: acceptCompletion
			},
			indentWithTab,
			{
				key: 'Escape',
				run: closeCompletion
			}
		]),
		history(),
		lineNumbers(),
		drawSelection(),
		dropCursor(),
		EditorState.allowMultipleSelections.of(true),
		indentOnInput(),
		syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
		bracketMatching(),
		closeBrackets(),
		highlightSpecialChars(),
		rectangularSelection(),
		foldGutter(),
		highlightActiveLine(),
		highlightActiveLineGutter(),
		highlightSelectionMatches(),
		keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...historyKeymap]),
		sql({ dialect: PostgreSQL }),
		EditorView.lineWrapping,
		EditorView.updateListener.of((update) => {
			if (update.docChanged) {
				onChange?.(update.state.doc.toString());
			}
		}),
		themeCompartment.of(createTheme(currentTheme)),
		readOnlyCompartment.of(disabled ? EditorState.readOnly.of(true) : []),
		...(lspClient ? [lspClient.plugin('file:///sql-editor.sql')] : [])
	];

	const state = EditorState.create({
		doc: value,
		extensions
	});

	container.style.height = '100%';
	container.style.overflow = 'hidden';

	const view = new EditorView({
		state,
		parent: container
	});

	const updateValue = (newValue: string) => {
		if (view.state.doc.toString() !== newValue) {
			view.dispatch({
				changes: { from: 0, to: view.state.doc.length, insert: newValue },
				annotations: [Transaction.addToHistory.of(false)]
			});
		}
	};

	const saveState = () => {
		return view.state;
	};

	const restoreState = (savedState: EditorState) => {
		view.setState(savedState);
	};

	const updateDisabled = (isDisabled: boolean) => {
		view.dispatch({
			effects: readOnlyCompartment.reconfigure(isDisabled ? EditorState.readOnly.of(true) : [])
		});
	};

	const updateTheme = (newTheme: 'light' | 'dark') => {
		currentTheme = newTheme;
		view.dispatch({
			effects: themeCompartment.reconfigure(createTheme(newTheme))
		});
	};

	const unregisterThemeCallback = registerEditorThemeCallback(updateTheme);

	const getExecutableText = () => {
		const selection = view.state.selection.main;
		if (!selection.empty) {
			// Return selected text
			return view.state.doc.sliceString(selection.from, selection.to).trim();
		} else {
			// Return current line if no selection
			const line = view.state.doc.lineAt(selection.from);
			return line.text.trim();
		}
	};

	const getSelectedText = () => {
		const selection = view.state.selection.main;
		if (!selection.empty) {
			return view.state.doc.sliceString(selection.from, selection.to).trim();
		}
		return null;
	};

	const updateSelectedConnection = (newConnectionId: string) => {
		lspTransport.updateSelectedConnection(newConnectionId).catch((error) => {
			console.error('Failed to notify LSP of selected connection:', error);
		});
	};

	return {
		view,
		updateValue,
		updateDisabled,
		updateTheme,
		focus: () => view.focus(),
		getExecutableText,
		getSelectedText,
		updateSelectedConnection,
		saveState,
		restoreState,
		dispose: () => {
			unregisterThemeCallback();
			view.destroy();
		}
	};
}

export const createEditor = createEditorInstance;

export default { EditorView, EditorState };
