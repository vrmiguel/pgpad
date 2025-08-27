export function formatJsonTruncated(value: any, maxLength: number = 60): string {
	let result = '';
	let length = 0;

	function addChar(char: string): boolean {
		if (length >= maxLength) return false;
		result += char;
		length++;
		return true;
	}

	function addString(str: string): boolean {
		if (length + str.length <= maxLength) {
			result += str;
			length += str.length;
			return true;
		} else if (length < maxLength) {
			const remaining = maxLength - length;
			result += str.slice(0, remaining);
			length = maxLength;
		}
		return false;
	}

	function formatValue(val: any): boolean {
		if (val === null) return addString('null');
		if (val === undefined) return addString('undefined');
		if (typeof val === 'boolean') return addString(val ? 'true' : 'false');
		if (typeof val === 'number') return addString(String(val));
		if (typeof val === 'string') {
			if (!addChar('"')) return false;
			for (let i = 0; i < val.length && length < maxLength - 1; i++) {
				const char = val[i];
				if (char === '"' || char === '\\') {
					if (!addChar('\\') || !addChar(char)) return false;
				} else if (char === '\n') {
					if (!addString('\\n')) return false;
				} else if (char === '\r') {
					if (!addString('\\r')) return false;
				} else if (char === '\t') {
					if (!addString('\\t')) return false;
				} else {
					if (!addChar(char)) return false;
				}
			}
			return addChar('"');
		}
		if (Array.isArray(val)) {
			if (!addChar('[')) return false;
			for (let i = 0; i < val.length && length < maxLength; i++) {
				if (i > 0 && !addChar(',')) return false;
				if (!formatValue(val[i])) return false;
			}
			return addChar(']');
		}
		if (typeof val === 'object') {
			if (!addChar('{')) return false;
			const keys = Object.keys(val);
			for (let i = 0; i < keys.length && length < maxLength; i++) {
				if (i > 0 && !addChar(',')) return false;
				if (!addChar('"') || !addString(keys[i]) || !addChar('"') || !addChar(':')) return false;
				if (!formatValue(val[keys[i]])) return false;
			}
			return addChar('}');
		}
		return addString(String(val));
	}

	const completed = formatValue(value);
	return completed ? result : result + '...';
}

export class CellFormatter {
	private static numberFormatter = new Intl.NumberFormat();

	static formatCellDisplay(value: unknown): string {
		if (value === null || value === undefined) return 'NULL';
		if (typeof value === 'number') return this.numberFormatter.format(value);
		if (typeof value === 'boolean') return value ? 'true' : 'false';
		if (typeof value === 'object') {
			return formatJsonTruncated(value, 60);
		}
		return String(value);
	}

	static formatCellForCopy(value: unknown): string {
		if (value === null || value === undefined) return 'NULL';
		if (typeof value === 'object') return JSON.stringify(value, null, 2);
		if (typeof value === 'number') return this.numberFormatter.format(value);
		return String(value);
	}

	static formatCellTitle(value: unknown): string | undefined {
		if (value === null || value === undefined) return undefined;
		if (typeof value === 'object') return '{ .. }';
		const text = typeof value === 'number' ? this.numberFormatter.format(value) : String(value);
		return text.length > 200 ? text.slice(0, 200) + '…' : text;
	}

	static getCellType(value: unknown): 'null' | 'boolean' | 'number' | 'object' | 'string' {
		if (value === null || value === undefined) return 'null';
		if (typeof value === 'boolean') return 'boolean';
		if (typeof value === 'number') return 'number';
		if (typeof value === 'object') return 'object';
		return 'string';
	}
}
