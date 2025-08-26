import { describe, it, expect } from 'vitest';
import { formatJsonTruncated, CellFormatter } from './cell-formatter';

describe('formatJsonTruncated', () => {
    it('should format simple values correctly', () => {
        expect(formatJsonTruncated(null)).toBe('null');
        expect(formatJsonTruncated(undefined)).toBe('undefined');
        expect(formatJsonTruncated(true)).toBe('true');
        expect(formatJsonTruncated(false)).toBe('false');
        expect(formatJsonTruncated(42)).toBe('42');
        expect(formatJsonTruncated('hello')).toBe('"hello"');
    });

    it('should format arrays correctly', () => {
        expect(formatJsonTruncated([])).toBe('[]');
        expect(formatJsonTruncated([1, 2, 3])).toBe('[1,2,3]');
        expect(formatJsonTruncated(['a', 'b'])).toBe('["a","b"]');
    });

    it('should format objects correctly', () => {
        expect(formatJsonTruncated({})).toBe('{}');
        expect(formatJsonTruncated({ a: 1 })).toBe('{"a":1}');
        expect(formatJsonTruncated({ name: 'test', value: 42 })).toBe('{"name":"test","value":42}');
    });

    it('should escape special characters in strings', () => {
        expect(formatJsonTruncated('hello "world"')).toBe('"hello \\"world\\""');
        expect(formatJsonTruncated('line1\nline2')).toBe('"line1\\nline2"');
        expect(formatJsonTruncated('tab\there')).toBe('"tab\\there"');
        expect(formatJsonTruncated('back\\slash')).toBe('"back\\\\slash"');
    });

    it('should truncate long content and add ellipsis', () => {
        const longObject = {
            veryLongKeyName: 'veryLongValueThatShouldCauseTruncation',
            another: 'field'
        };
        const result = formatJsonTruncated(longObject, 30);
        expect(result).toHaveLength(33); // 30 chars + "..."
        expect(result.endsWith('...')).toBe(true);
    });

    it('should handle nested structures', () => {
        const nested = {
            user: {
                name: 'John',
                age: 30
            },
            items: [1, 2, 3]
        };
        const result = formatJsonTruncated(nested, 100);
        expect(result).toContain('{"user":{"name":"John","age":30},"items":[1,2,3]}');
    });

    it('should stop processing at maxLength', () => {
        const largeArray = new Array(1000).fill('test');
        const result = formatJsonTruncated(largeArray, 20);
        expect(result).toHaveLength(23); // 20 chars + "..."
        expect(result.endsWith('...')).toBe(true);
    });
});

describe('CellFormatter', () => {
    describe('formatCellDisplay', () => {
        it('should format different types correctly', () => {
            expect(CellFormatter.formatCellDisplay(null)).toBe('NULL');
            expect(CellFormatter.formatCellDisplay(undefined)).toBe('NULL');
            expect(CellFormatter.formatCellDisplay(true)).toBe('true');
            expect(CellFormatter.formatCellDisplay(false)).toBe('false');
            expect(CellFormatter.formatCellDisplay(1234.56)).toMatch(/1,?234\.56/); // Locale-dependent
            expect(CellFormatter.formatCellDisplay('hello')).toBe('hello');
            expect(CellFormatter.formatCellDisplay({ key: 'value' })).toBe('{"key":"value"}');
        });
    });

    describe('formatCellForCopy', () => {
        it('should format for clipboard correctly', () => {
            expect(CellFormatter.formatCellForCopy(null)).toBe('NULL');
            expect(CellFormatter.formatCellForCopy({ key: 'value' })).toBe('{\n  "key": "value"\n}');
            expect(CellFormatter.formatCellForCopy('text')).toBe('text');
        });
    });

    describe('formatCellTitle', () => {
        it('should format titles correctly', () => {
            expect(CellFormatter.formatCellTitle(null)).toBeUndefined();
            expect(CellFormatter.formatCellTitle(undefined)).toBeUndefined();
            expect(CellFormatter.formatCellTitle({ key: 'value' })).toBe('{ .. }');
            expect(CellFormatter.formatCellTitle('short')).toBe('short');

            const longText = 'a'.repeat(250);
            const result = CellFormatter.formatCellTitle(longText);
            expect(result).toHaveLength(203); // 200 chars + "…"
            expect(result?.endsWith('…')).toBe(true);
        });
    });

    describe('getCellType', () => {
        it('should identify types correctly', () => {
            expect(CellFormatter.getCellType(null)).toBe('null');
            expect(CellFormatter.getCellType(undefined)).toBe('null');
            expect(CellFormatter.getCellType(true)).toBe('boolean');
            expect(CellFormatter.getCellType(42)).toBe('number');
            expect(CellFormatter.getCellType('text')).toBe('string');
            expect(CellFormatter.getCellType({})).toBe('object');
            expect(CellFormatter.getCellType([])).toBe('object');
        });
    });
});
