import { drawSelection, dropCursor, EditorView, highlightSpecialChars, lineNumbers, rectangularSelection } from '@codemirror/view';
import { EditorState, type Extension, Compartment } from '@codemirror/state';
import { PostgreSQL, sql } from '@codemirror/lang-sql';
import { autocompletion, acceptCompletion, moveCompletionSelection, closeCompletion, type CompletionContext, type CompletionResult, closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete';
import { keymap } from '@codemirror/view';
import { indentWithTab, history, historyKeymap, defaultKeymap } from '@codemirror/commands';

import type { DatabaseSchema } from './commands.svelte';
import { theme as themeStore, registerEditorThemeCallback } from './stores/theme';
import { get } from 'svelte/store';
import { bracketMatching, indentOnInput, foldGutter, syntaxHighlighting, defaultHighlightStyle } from '@codemirror/language';
import { highlightActiveLine, highlightActiveLineGutter } from '@codemirror/view';
import { highlightSelectionMatches } from '@codemirror/search';

function createTheme(theme: 'light' | 'dark') {
    return EditorView.theme({
        '&': {
            color: theme === 'dark' ? '#d4d4d4' : '#24292e',
            backgroundColor: theme === 'dark' ? '#1e1e1e' : '#ffffff',
            height: '100%',
            overflow: 'hidden',
            borderRadius: '0.75rem',
            fontSize: '15px',
            lineHeight: '1.5',
            margin: '0',
            padding: '0',
            border: 'none',
            boxShadow: 'none'
        },
        '.cm-content': {
            padding: '0 12px',  // No top/bottom padding, only left/right
            margin: '0',        // Ensure no margins
            caretColor: theme === 'dark' ? '#d4d4d4' : '#24292e',
            minHeight: '100%',
            border: 'none',
            borderRadius: '0.75rem'
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
            backgroundColor: theme === 'dark' ? 'rgba(255, 255, 255, 0.02)' : 'rgba(0, 0, 0, 0.02)',  // Very subtle active line
            position: 'relative',
            zIndex: '1'
        },
        '.cm-line': {
            position: 'relative'
        },
        '.cm-activeLineGutter': {
            backgroundColor: theme === 'dark' ? 'rgba(255, 255, 255, 0.02)' : 'rgba(0, 0, 0, 0.02)'  // Very subtle active line gutter
        },
        '.cm-scroller': {
            outline: 'none',
            height: '100%',
            overflow: 'auto',
            borderRadius: '0.75rem',
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
            fontSize: '15px',
            margin: '0',
            padding: '0',
            borderTopLeftRadius: '0.75rem',
            borderBottomLeftRadius: '0.75rem',
            borderTopRightRadius: '0',
            borderBottomRightRadius: '0'
        },
        '.cm-gutter': {
            minHeight: '100%'
        },
        '.cm-lineNumbers': {
            minHeight: '100%',
            fontSize: '15px',
            margin: '0',
            padding: '0 0px 0 10px'
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
    }, { dark: theme === 'dark' });
}

function generateSchemaCompletions(schema: DatabaseSchema | null): any[] {
    if (!schema) {
        return [];
    }

    const completions: any[] = [];

    for (const table of schema.tables) {
        const tableName = table.schema === 'public' ? table.name : `${table.schema}.${table.name}`;
        completions.push({
            label: tableName,
            type: 'class',
            info: `Table: ${tableName} (${table.columns.length} columns)`,
            detail: 'table'
        });
    }

    for (const columnName of schema.unique_columns) {
        completions.push({
            label: columnName,
            type: 'property',
            info: `Column: ${columnName}`,
            detail: 'column'
        });
    }

    // Add qualified column completions (table.column)
    for (const table of schema.tables) {
        const tableName = table.schema === 'public' ? table.name : `${table.schema}.${table.name}`;
        for (const column of table.columns) {
            completions.push({
                label: `${tableName}.${column.name}`,
                type: 'property',
                info: `Column: ${column.name} (${column.data_type}) from ${tableName}${column.is_nullable ? ', nullable' : ', not null'}`,
                detail: `${tableName} column`
            });
        }
    }

    for (const schemaName of schema.schemas) {
        if (schemaName !== 'public') {
            completions.push({
                label: schemaName,
                type: 'namespace',
                info: `Schema: ${schemaName}`,
                detail: 'schema'
            });
        }
    }

    return completions;
}

// An attempt at cloning Rust's eq_ignore_ascii_case
//
// Might seem silly but this seems 5x more performant than toLowerCase(),
// as tested in https://gist.github.com/vrmiguel/a322ac665da53a40e98e5188d142244e
function startsWith(str: string, prefix: string, startIndex: number = 0): boolean {
    if (prefix.length > str.length - startIndex) return false;

    for (let i = 0; i < prefix.length; i++) {
        const a = str.charCodeAt(startIndex + i);
        const b = prefix.charCodeAt(i);

        if (a === b) continue;

        if (a > 127 || b > 127) {
            // Fallback to slower but correct Unicode comparison
            return str.slice(startIndex, startIndex + prefix.length).toLowerCase().startsWith(prefix.toLowerCase());
        }

        const aUpper = (a >= 97 && a <= 122) ? a - 32 : a;  // a-z -> A-Z
        const bUpper = (b >= 97 && b <= 122) ? b - 32 : b;  // a-z -> A-Z

        if (aUpper !== bUpper) return false;
    }
    return true;
}

function includes(str: string, substring: string): boolean {
    const strLen = str.length;
    const subLen = substring.length;

    if (subLen === 0) return true;
    if (subLen > strLen) return false;

    for (let i = 0; i <= strLen - subLen; i++) {
        if (startsWith(str, substring, i)) {
            return true;
        }
    }
    return false;
}

const sqlKeywords = [
    'SELECT', 'FROM', 'WHERE', 'INSERT INTO', 'UPDATE', 'DELETE FROM',
    'CREATE TABLE', 'DROP TABLE', 'ORDER BY', 'GROUP BY', 'HAVING',
    'JOIN', 'LEFT JOIN', 'RIGHT JOIN', 'INNER JOIN', 'OUTER JOIN',
    'UNION', 'UNION ALL', 'DISTINCT', 'COUNT', 'SUM', 'AVG', 'MAX', 'MIN',
    'AND', 'OR', 'NOT', 'IN', 'LIKE', 'BETWEEN', 'IS NULL', 'IS NOT NULL',
    'AS', 'LIMIT', 'OFFSET', 'CASE', 'WHEN', 'THEN', 'ELSE', 'END'
];


function needsQuoting(identifier: string): boolean {
    if (/[A-Z]/.test(identifier)) return true;
    if (/[^a-z0-9_]/.test(identifier)) return true;
    if (/^[0-9]/.test(identifier)) return true;

    return false;
}

function formatIdentifierForCompletion(identifier: string): string {
    // Handle qualified identifiers (table.column)
    if (identifier.includes('.')) {
        // For qualified identifiers, always format each part independently
        // The opening quote (if any) gets "consumed" and replaced with proper quoting
        const parts = identifier.split('.');
        const formattedParts = parts.map(part => {
            if (needsQuoting(part)) {
                return `"${part}"`;
            }
            return part;
        });

        return formattedParts.join('.');
    }

    // Handle simple identifiers
    if (needsQuoting(identifier)) {
        // Always use both quotes for identifiers that need quoting
        return `"${identifier}"`;
    }

    return identifier;
}

function createSqlAutocompletion(schema: DatabaseSchema | null) {
    const cachedCompletions = generateSchemaCompletions(schema);
    console.log(`Cached ${cachedCompletions.length} schema completions`);

    return autocompletion({
        override: [
            (context: CompletionContext): CompletionResult | null => {
                const word = context.matchBefore(/[\w.]*/);
                if (!word) return null;

                if (word.from === word.to && !context.explicit) return null;

                console.log(`Completion triggered for "${word.text}"`);

                // Check if there's an opening quote before the matched word
                const charBeforeWord = word.from > 0 ? context.state.doc.sliceString(word.from - 1, word.from) : '';
                const hasOpeningQuote = charBeforeWord === '"' || charBeforeWord === "'";

                // Check if there's a closing quote after the cursor (from auto-closing brackets)
                const charAfterCursor = word.to < context.state.doc.length ? context.state.doc.sliceString(word.to, word.to + 1) : '';
                const hasAutoClosingQuote = hasOpeningQuote && (charAfterCursor === '"' || charAfterCursor === "'");

                const searchText = word.text;

                const options = [];

                for (const keyword of sqlKeywords) {
                    if (startsWith(keyword, searchText)) {
                        options.push({
                            label: keyword,
                            type: 'keyword',
                            info: `SQL keyword: ${keyword}`,
                            boost: 0.7
                        });
                    }
                }

                for (const completion of cachedCompletions) {
                    let boost = 0;
                    let matches = false;

                    if (startsWith(completion.label, searchText)) {
                        matches = true;
                        boost = 1.0;
                    } else if (completion.label.includes('.') && includes(completion.label, '.' + searchText)) {
                        matches = true;
                        boost = 0.8;
                    } else if (searchText.length >= 2 && includes(completion.label, searchText)) {
                        matches = true;
                        boost = 0.3;
                    }

                    if (matches) {
                        const formattedCompletion = {
                            ...completion,
                            label: formatIdentifierForCompletion(completion.label),
                            boost
                        };
                        options.push(formattedCompletion);
                    }
                }

                const limitedOptions = options
                    .sort((a, b) => (b.boost || 0) - (a.boost || 0))
                    .slice(0, 50);

                return {
                    from: hasOpeningQuote ? word.from - 1 : word.from,
                    to: hasAutoClosingQuote ? word.to + 1 : word.to,
                    options: limitedOptions,
                    // Be more restrictive for short searches
                    validFor: searchText.length >= 2 ? /^[\w.]*$/ : /^[\w.]{0,3}$/
                };
            }
        ]
    });
}

export interface CreateEditorOptions {
    container: HTMLElement;
    value: string;
    onChange?: (value: string) => void;
    onExecute?: () => void;
    onExecuteSelection?: (selectedText: string) => void;
    disabled?: boolean;
    schema?: DatabaseSchema | null;
}

export function createEditorInstance(options: CreateEditorOptions) {
    const { container, value, onChange, onExecute, onExecuteSelection, disabled = false, schema = null } = options;

    let currentTheme: 'light' | 'dark' = get(themeStore);
    const unsubscribeTheme = themeStore.subscribe(t => currentTheme = t);

    let currentSchema = schema;

    // Create compartments for dynamic reconfiguration
    const themeCompartment = new Compartment();
    const readOnlyCompartment = new Compartment();
    const schemaCompartment = new Compartment();

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
        autocompletion(),
        highlightSpecialChars(),
        rectangularSelection(),
        foldGutter(),
        highlightActiveLine(),
        highlightActiveLineGutter(),
        highlightSelectionMatches(),
        keymap.of([
            ...closeBracketsKeymap,
            ...defaultKeymap,
            ...historyKeymap,
        ]),
        sql({ dialect: PostgreSQL }),
        schemaCompartment.of(createSqlAutocompletion(currentSchema)),
        EditorView.lineWrapping,
        EditorView.updateListener.of((update) => {
            if (update.docChanged) {
                onChange?.(update.state.doc.toString());
            }
        }),
        themeCompartment.of(createTheme(currentTheme)),
        readOnlyCompartment.of(disabled ? EditorState.readOnly.of(true) : [])
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
                changes: { from: 0, to: view.state.doc.length, insert: newValue }
            });
        }
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

    const updateSchema = (newSchema: DatabaseSchema | null) => {
        currentSchema = newSchema;
        view.dispatch({
            effects: schemaCompartment.reconfigure(createSqlAutocompletion(currentSchema))
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
        updateSchema,
        dispose: () => {
            unsubscribeTheme();
            unregisterThemeCallback();
            view.destroy();
        }
    };
}

export const createEditor = createEditorInstance;

export default { EditorView, EditorState }; 