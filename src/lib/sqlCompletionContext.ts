export type CompletionIntent = 'relation' | 'column' | 'keyword' | 'unknown';

type SqlContextToken = { type: 'word'; value: string } | { type: 'symbol'; value: string };

type CompletionLike = {
	type?: string;
};

const relationClauseKeywords = new Set(['from', 'join', 'update']);
const columnClauseKeywords = new Set([
	'by',
	'group',
	'having',
	'on',
	'order',
	'returning',
	'select',
	'set',
	'where'
]);
const relationBoundaryKeywords = new Set([
	'cross',
	'except',
	'full',
	'group',
	'having',
	'inner',
	'intersect',
	'join',
	'left',
	'limit',
	'natural',
	'offset',
	'on',
	'order',
	'right',
	'union',
	'where'
]);
function isWordChar(charCode: number) {
	return (
		(charCode >= 65 && charCode <= 90) ||
		(charCode >= 97 && charCode <= 122) ||
		(charCode >= 48 && charCode <= 57) ||
		charCode === 95
	);
}

function tokenizeSqlContext(text: string): SqlContextToken[] {
	const tokens: SqlContextToken[] = [];
	let statementStart = 0;
	let i = 0;

	while (i < text.length) {
		const charCode = text.charCodeAt(i);
		const nextCode = i + 1 < text.length ? text.charCodeAt(i + 1) : 0;

		if (charCode <= 32) {
			i++;
			continue;
		}

		if (charCode === 45 && nextCode === 45) {
			i += 2;
			while (i < text.length && text.charCodeAt(i) !== 10) i++;
			continue;
		}

		if (charCode === 47 && nextCode === 42) {
			i += 2;
			while (i + 1 < text.length) {
				if (text.charCodeAt(i) === 42 && text.charCodeAt(i + 1) === 47) {
					i += 2;
					break;
				}
				i++;
			}
			continue;
		}

		if (charCode === 39) {
			const quote = charCode;
			i++;
			while (i < text.length) {
				const current = text.charCodeAt(i);
				if (current === quote) {
					if (i + 1 < text.length && text.charCodeAt(i + 1) === quote) {
						i += 2;
						continue;
					}
					i++;
					break;
				}
				i++;
			}
			continue;
		}

		if (charCode === 34 || charCode === 96) {
			const quote = charCode;
			let value = '';
			i++;
			while (i < text.length) {
				const current = text.charCodeAt(i);
				if (current === quote) {
					if (i + 1 < text.length && text.charCodeAt(i + 1) === quote) {
						value += text[i];
						i += 2;
						continue;
					}
					i++;
					break;
				}
				value += text[i];
				i++;
			}
			tokens.push({ type: 'word', value: value.toLowerCase() });
			continue;
		}

		if (charCode === 59) {
			statementStart = tokens.length + 1;
			tokens.push({ type: 'symbol', value: ';' });
			i++;
			continue;
		}

		if (isWordChar(charCode)) {
			const start = i;
			i++;
			while (i < text.length && isWordChar(text.charCodeAt(i))) i++;
			tokens.push({ type: 'word', value: text.slice(start, i).toLowerCase() });
			continue;
		}

		tokens.push({ type: 'symbol', value: text[i] });
		i++;
	}

	return tokens.slice(statementStart);
}

function previousWord(tokens: SqlContextToken[], beforeIndex: number) {
	for (let i = beforeIndex - 1; i >= 0; i--) {
		const token = tokens[i];
		if (token.type === 'word') return token.value;
	}

	return null;
}

function hasRelationAfterClause(tokens: SqlContextToken[], clauseIndex: number) {
	for (let i = clauseIndex + 1; i < tokens.length; i++) {
		const token = tokens[i];
		if (token.type === 'symbol') {
			if (token.value === ',' || token.value === '(' || token.value === ')') continue;
			return true;
		}

		if (relationBoundaryKeywords.has(token.value)) return false;
		if (token.value === 'as') {
			i++;
			continue;
		}

		return true;
	}

	return false;
}

function isRelationSlotOpenAfterClause(tokens: SqlContextToken[], clauseIndex: number) {
	let expectingRelation = true;

	for (let i = clauseIndex + 1; i < tokens.length; i++) {
		const token = tokens[i];

		if (token.type === 'symbol') {
			if (token.value === ',') {
				expectingRelation = true;
			}
			continue;
		}

		if (relationBoundaryKeywords.has(token.value)) return false;
		if (token.value === 'as') {
			i++;
			expectingRelation = false;
			continue;
		}

		if (expectingRelation) {
			expectingRelation = false;
		}
	}

	return expectingRelation;
}

export function inferSqlCompletionIntent(textBeforeCompletion: string): CompletionIntent {
	const tokens = tokenizeSqlContext(textBeforeCompletion);

	for (let i = tokens.length - 1; i >= 0; i--) {
		const token = tokens[i];
		if (token.type !== 'word') continue;

		if (token.value === 'into') {
			const previous = previousWord(tokens, i);
			if (previous === 'insert') {
				return hasRelationAfterClause(tokens, i) ? 'column' : 'relation';
			}
			return 'unknown';
		}

		if (relationClauseKeywords.has(token.value)) {
			return isRelationSlotOpenAfterClause(tokens, i) ? 'relation' : 'unknown';
		}

		if (columnClauseKeywords.has(token.value)) {
			return 'column';
		}

		if (token.value === 'values' || token.value === 'limit' || token.value === 'offset') {
			return 'keyword';
		}
	}

	return 'unknown';
}

export function completionIntentBoost(completion: CompletionLike, intent: CompletionIntent) {
	if (intent === 'relation') {
		switch (completion.type) {
			case 'class':
				return 3;
			case 'namespace':
				return 1.25;
			case 'property':
				return -2;
			case 'keyword':
				return -0.75;
			default:
				return 0;
		}
	}

	if (intent === 'column') {
		switch (completion.type) {
			case 'property':
				return 2;
			case 'keyword':
				return 0.35;
			case 'class':
				return -0.75;
			case 'namespace':
				return -1;
			default:
				return 0;
		}
	}

	return 0;
}
