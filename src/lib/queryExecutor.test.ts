import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { QueryExecutor } from './queryExecutor.svelte';
import type { Page, QueryEvent, QueryId } from './commands.svelte';

vi.mock('$lib/commands.svelte', () => ({
	Commands: {
		submitQuery: vi.fn(),
		fetchPage: vi.fn(),
		listenQueryEvents: vi.fn()
	}
}));

import { Commands } from '$lib/commands.svelte';

const mockCommands = Commands as unknown as {
	submitQuery: ReturnType<typeof vi.fn>;
	fetchPage: ReturnType<typeof vi.fn>;
	listenQueryEvents: ReturnType<typeof vi.fn>;
};

type QueryEventHandler = (event: QueryEvent) => void;

let queryEventHandler: QueryEventHandler | undefined;
let unlistenQueryEvents: ReturnType<typeof vi.fn>;

function createDeferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});

	return { promise, resolve, reject };
}

async function flushPromises() {
	await new Promise((resolve) => setTimeout(resolve, 0));
}

async function emitQueryEvent(event: QueryEvent) {
	expect(queryEventHandler).toBeDefined();
	queryEventHandler?.(event);
	await flushPromises();
}

function page(rows: Page): Page {
	return rows;
}

describe('QueryExecutor', () => {
	let executor: QueryExecutor;

	beforeEach(() => {
		queryEventHandler = undefined;
		unlistenQueryEvents = vi.fn();
		vi.clearAllMocks();

		mockCommands.listenQueryEvents.mockImplementation(async (handler: QueryEventHandler) => {
			queryEventHandler = handler;
			return unlistenQueryEvents;
		});
		mockCommands.fetchPage.mockResolvedValue(page([[1]]));
		executor = new QueryExecutor();
	});

	afterEach(() => {
		executor.dispose();
	});

	it('starts the query event listener when constructed', () => {
		expect(mockCommands.listenQueryEvents).toHaveBeenCalledTimes(1);
		expect(queryEventHandler).toBeDefined();
	});

	it('waits for the query event listener before submitting a query', async () => {
		executor.dispose();
		vi.clearAllMocks();
		queryEventHandler = undefined;
		unlistenQueryEvents = vi.fn();

		const listenerReady = createDeferred<() => void>();
		mockCommands.listenQueryEvents.mockImplementation(async (handler: QueryEventHandler) => {
			queryEventHandler = handler;
			return await listenerReady.promise;
		});
		mockCommands.submitQuery.mockResolvedValue([1]);

		executor = new QueryExecutor();
		const execution = executor.executeQuery('SELECT 1', 'conn-1');
		await flushPromises();

		expect(mockCommands.submitQuery).not.toHaveBeenCalled();

		listenerReady.resolve(unlistenQueryEvents);
		await execution;

		expect(mockCommands.submitQuery).toHaveBeenCalledWith('conn-1', 'SELECT 1');
	});

	it('creates result tabs from the submitted event before submitQuery returns', async () => {
		const submit = createDeferred<QueryId[]>();
		mockCommands.submitQuery.mockReturnValue(submit.promise);

		const execution = executor.executeQuery('SELECT 1; SELECT 2', 'conn-1');
		await flushPromises();

		await emitQueryEvent({ type: 'submitted', query_ids: [10, 11] });

		expect(executor.resultTabs).toHaveLength(2);
		expect(executor.resultTabs.map((tab) => tab.queryId)).toEqual([10, 11]);
		expect(executor.resultTabs.map((tab) => tab.status)).toEqual(['Running', 'Running']);

		submit.resolve([10, 11]);
		await execution;
		await flushPromises();

		expect(executor.resultTabs).toHaveLength(2);
		expect(executor.resultTabs.map((tab) => tab.queryId)).toEqual([10, 11]);
	});

	it('falls back to the submitQuery response when the submitted event arrives later', async () => {
		mockCommands.submitQuery.mockResolvedValue([42]);

		await executor.executeQuery('SELECT 1', 'conn-1');

		expect(executor.resultTabs).toHaveLength(1);
		expect(executor.resultTabs[0]).toMatchObject({
			queryId: 42,
			query: 'SELECT 1',
			status: 'Running'
		});
		expect(executor.activeResultTabId).toBe(executor.resultTabs[0].id);
	});

	it('ignores duplicate submitted events after tabs already exist', async () => {
		mockCommands.submitQuery.mockResolvedValue([1]);

		await executor.executeQuery('SELECT 1', 'conn-1');
		const originalTabId = executor.resultTabs[0].id;

		await emitQueryEvent({ type: 'submitted', query_ids: [1] });

		expect(executor.resultTabs).toHaveLength(1);
		expect(executor.resultTabs[0].id).toBe(originalTabId);
	});

	it('applies columns and loads the first page when a page becomes ready', async () => {
		mockCommands.submitQuery.mockResolvedValue([1]);
		mockCommands.fetchPage.mockResolvedValue(
			page([
				[1, 'Alice'],
				[2, 'Bob']
			])
		);

		await executor.executeQuery('SELECT * FROM users', 'conn-1');
		await emitQueryEvent({ type: 'columns_ready', query_id: 1, columns: ['id', 'name'] });
		await emitQueryEvent({ type: 'page_ready', query_id: 1, page_index: 0, page_count: 3 });

		expect(mockCommands.fetchPage).toHaveBeenCalledWith(1, 0);
		expect(executor.resultTabs[0]).toMatchObject({
			columns: ['id', 'name'],
			queryReturnsResults: true,
			currentPageIndex: 0,
			currentPageData: [
				[1, 'Alice'],
				[2, 'Bob']
			],
			totalPages: 3
		});
	});

	it('marks row-returning queries complete and reports the current page-count estimate', async () => {
		const onComplete = vi.fn();
		mockCommands.submitQuery.mockResolvedValue([1]);

		await executor.executeQuery('SELECT * FROM users', 'conn-1', onComplete);
		await emitQueryEvent({ type: 'columns_ready', query_id: 1, columns: ['id'] });
		await emitQueryEvent({ type: 'page_ready', query_id: 1, page_index: 0, page_count: 2 });
		await emitQueryEvent({
			type: 'finished',
			query_id: 1,
			status: 'Completed',
			affected_rows: null,
			error: null
		});

		expect(executor.resultTabs[0]).toMatchObject({
			status: 'Completed',
			queryReturnsResults: true,
			totalPages: 2
		});
		expect(onComplete).toHaveBeenCalledTimes(1);
		expect(onComplete).toHaveBeenCalledWith(100);
	});

	it('marks mutation queries complete with affected rows', async () => {
		const onComplete = vi.fn();
		mockCommands.submitQuery.mockResolvedValue([5]);

		await executor.executeQuery('UPDATE users SET active = true', 'conn-1', onComplete);
		await emitQueryEvent({
			type: 'finished',
			query_id: 5,
			status: 'Completed',
			affected_rows: 7,
			error: null
		});

		expect(executor.resultTabs[0]).toMatchObject({
			status: 'Completed',
			queryReturnsResults: false,
			affectedRows: 7
		});
		expect(onComplete).toHaveBeenCalledWith(7);
	});

	it('marks statement errors without calling onComplete', async () => {
		const onComplete = vi.fn();
		mockCommands.submitQuery.mockResolvedValue([1]);

		await executor.executeQuery('SELECT * FROM missing_table', 'conn-1', onComplete);
		await emitQueryEvent({
			type: 'finished',
			query_id: 1,
			status: 'Error',
			affected_rows: null,
			error: 'no such table: missing_table'
		});

		expect(executor.resultTabs[0]).toMatchObject({
			status: 'Error',
			error: 'no such table: missing_table'
		});
		expect(onComplete).not.toHaveBeenCalled();
	});

	it('ignores events for stale query ids after a new query starts', async () => {
		mockCommands.submitQuery.mockResolvedValueOnce([1]).mockResolvedValueOnce([2]);

		await executor.executeQuery('SELECT old', 'conn-1');
		await executor.executeQuery('SELECT new', 'conn-1');
		await emitQueryEvent({
			type: 'finished',
			query_id: 1,
			status: 'Error',
			affected_rows: null,
			error: 'old query failed late'
		});

		expect(executor.resultTabs).toHaveLength(1);
		expect(executor.resultTabs[0]).toMatchObject({
			queryId: 2,
			query: 'SELECT new',
			status: 'Running'
		});
		expect(executor.resultTabs[0].error).toBeUndefined();
	});

	it('keeps a single event listener for repeated executions and unregisters it on dispose', async () => {
		mockCommands.submitQuery.mockResolvedValueOnce([1]).mockResolvedValueOnce([2]);

		await executor.executeQuery('SELECT 1', 'conn-1');
		await executor.executeQuery('SELECT 2', 'conn-1');

		expect(mockCommands.listenQueryEvents).toHaveBeenCalledTimes(1);
		expect(unlistenQueryEvents).not.toHaveBeenCalled();

		executor.dispose();

		expect(unlistenQueryEvents).toHaveBeenCalledTimes(1);
	});

	it('keeps the latest requested page when concurrent page loads race', async () => {
		const firstPage = createDeferred<Page | null>();
		const secondPage = createDeferred<Page | null>();
		mockCommands.submitQuery.mockResolvedValue([1]);
		mockCommands.fetchPage
			.mockReturnValueOnce(firstPage.promise)
			.mockReturnValueOnce(secondPage.promise);

		await executor.executeQuery('SELECT * FROM users', 'conn-1');
		const firstLoad = executor.loadPage(1, 1);
		const secondLoad = executor.loadPage(1, 2);

		secondPage.resolve(page([[2, 'second']]));
		await secondLoad;
		firstPage.resolve(page([[1, 'first']]));
		await firstLoad;

		expect(executor.resultTabs[0]).toMatchObject({
			currentPageIndex: 2,
			currentPageData: [[2, 'second']]
		});
	});

	it('supports result tab selection and closing', async () => {
		mockCommands.submitQuery.mockResolvedValue([1, 2]);

		await executor.executeQuery('SELECT 1; SELECT 2', 'conn-1');
		const [firstTab, secondTab] = executor.resultTabs;

		executor.handleResultTabSelect(secondTab.id);
		expect(executor.activeResultTabId).toBe(secondTab.id);

		executor.handleResultTabClose(secondTab.id);
		expect(executor.resultTabs).toHaveLength(1);
		expect(executor.activeResultTabId).toBe(firstTab.id);

		executor.handleResultTabClose(firstTab.id);
		expect(executor.resultTabs).toHaveLength(0);
		expect(executor.activeResultTabId).toBeNull();
	});
});
