import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { QueryExecutor } from './queryExecutor.svelte';
import type { QueryId, QueryStatus, StatementInfo, Page } from './commands.svelte';

vi.mock('$lib/commands.svelte', () => ({
	Commands: {
		submitQuery: vi.fn(),
		waitUntilRenderable: vi.fn(),
		getColumns: vi.fn(),
		fetchPage: vi.fn(),
		getQueryStatus: vi.fn(),
		getPageCount: vi.fn()
	}
}));

import { Commands } from '$lib/commands.svelte';

const mockCommands = Commands as unknown as {
	submitQuery: ReturnType<typeof vi.fn>;
	waitUntilRenderable: ReturnType<typeof vi.fn>;
	getColumns: ReturnType<typeof vi.fn>;
	fetchPage: ReturnType<typeof vi.fn>;
	getQueryStatus: ReturnType<typeof vi.fn>;
	getPageCount: ReturnType<typeof vi.fn>;
};

function createMockStatementInfo(
	overrides: Partial<StatementInfo> = {}
): StatementInfo {
	return {
		returns_values: true,
		status: 'Completed',
		first_page: [[1, 'test']],
		affected_rows: null,
		error: null,
		...overrides
	};
}

function createMockPage(rows: number = 2): Page {
	return Array.from({ length: rows }, (_, i) => [i, `row${i}`]);
}

async function flushPromises() {
	return new Promise((resolve) => setTimeout(resolve, 0));
}

describe('QueryExecutor', () => {
	let executor: QueryExecutor;

	beforeEach(() => {
		executor = new QueryExecutor();
		vi.clearAllMocks();
	});

	afterEach(() => {
		executor.dispose();
		vi.clearAllTimers();
	});

	describe('Query Execution', () => {
		it('should execute a single-statement query successfully', async () => {
			const queryId: QueryId = 1;
			const queryText = 'SELECT * FROM users';
			const connectionId = 'conn-1';

			mockCommands.submitQuery.mockResolvedValue([queryId]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({
					status: 'Completed',
					first_page: [[1, 'Alice'], [2, 'Bob']]
				})
			);
			mockCommands.getColumns.mockResolvedValue(['id', 'name']);
			mockCommands.getPageCount.mockResolvedValue(1);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			expect(executor.resultTabs).toHaveLength(1);
			expect(executor.resultTabs[0]).toMatchObject({
				queryId,
				query: queryText,
				status: 'Completed',
				queryReturnsResults: true,
				columns: ['id', 'name'],
				currentPageIndex: 0,
				totalPages: 1
			});
			expect(executor.activeResultTabId).toBe(executor.resultTabs[0].id);
		});

		it('should execute a multi-statement query and create multiple tabs', async () => {
			const queryIds: QueryId[] = [1, 2];
			const queryText = 'SELECT * FROM users; SELECT * FROM posts';
			const connectionId = 'conn-1';

			mockCommands.submitQuery.mockResolvedValue(queryIds);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({ status: 'Completed' })
			);
			mockCommands.getColumns.mockResolvedValue(['id', 'data']);
			mockCommands.getPageCount.mockResolvedValue(1);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			expect(executor.resultTabs).toHaveLength(2);
			expect(executor.resultTabs[0].queryId).toBe(1);
			expect(executor.resultTabs[1].queryId).toBe(2);
			expect(executor.activeResultTabId).toBe(executor.resultTabs[1].id);
		});

		it('should handle query execution errors gracefully', async () => {
			const queryText = 'SELECT * FROM invalid_table';
			const connectionId = 'conn-1';
			const errorMessage = 'Table does not exist';

			mockCommands.submitQuery.mockRejectedValue(new Error(errorMessage));

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			expect(executor.resultTabs).toHaveLength(1);
			expect(executor.resultTabs[0]).toMatchObject({
				status: 'Error',
				error: errorMessage,
				queryId: -1
			});
		});

		it('should handle query with results vs query without results', async () => {
			const queryId: QueryId = 1;
			const queryText = 'INSERT INTO users VALUES (1, "test")';
			const connectionId = 'conn-1';
			const affectedRows = 1;

			mockCommands.submitQuery.mockResolvedValue([queryId]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({
					returns_values: false,
					status: 'Completed',
					first_page: null,
					affected_rows: affectedRows
				})
			);

			const onComplete = vi.fn();
			await executor.executeQuery(queryText, connectionId, onComplete);
			await flushPromises();

			expect(executor.resultTabs).toHaveLength(1);
			expect(executor.resultTabs[0]).toMatchObject({
				status: 'Completed',
				queryReturnsResults: false,
				affectedRows
			});
			expect(onComplete).toHaveBeenCalledWith(affectedRows);
		});

		it('should handle statement-level errors', async () => {
			const queryId: QueryId = 1;
			const queryText = 'SELECT * FROM users';
			const connectionId = 'conn-1';
			const errorMessage = 'Column does not exist';

			mockCommands.submitQuery.mockResolvedValue([queryId]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({
					status: 'Error',
					error: errorMessage
				})
			);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			expect(executor.resultTabs).toHaveLength(1);
			expect(executor.resultTabs[0]).toMatchObject({
				status: 'Error',
				error: errorMessage
			});
		});

		it('should call onComplete with correct row count', async () => {
			const queryId: QueryId = 1;
			const queryText = 'SELECT * FROM users';
			const connectionId = 'conn-1';
			const pageCount = 5;
			const onComplete = vi.fn();

			mockCommands.submitQuery.mockResolvedValue([queryId]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({ status: 'Completed' })
			);
			mockCommands.getColumns.mockResolvedValue(['id']);
			mockCommands.getPageCount.mockResolvedValue(pageCount);

			await executor.executeQuery(queryText, connectionId, onComplete);
			await flushPromises();

			// 50 rows per page * 5 pages = 250 rows
			expect(onComplete).toHaveBeenCalledWith(250);
		});

		it('should not call onComplete on errors', async () => {
			const queryText = 'SELECT * FROM invalid';
			const connectionId = 'conn-1';
			const onComplete = vi.fn();

			mockCommands.submitQuery.mockRejectedValue(new Error('SQL error'));

			await executor.executeQuery(queryText, connectionId, onComplete);
			await flushPromises();

			expect(onComplete).not.toHaveBeenCalled();
		});
	});

	describe('Tab Management', () => {
		it('should set active tab correctly on initial load', async () => {
			const queryId: QueryId = 1;
			const queryText = 'SELECT * FROM users';
			const connectionId = 'conn-1';

			mockCommands.submitQuery.mockResolvedValue([queryId]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({ status: 'Completed' })
			);
			mockCommands.getColumns.mockResolvedValue(['id']);
			mockCommands.getPageCount.mockResolvedValue(1);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			expect(executor.activeResultTabId).toBe(executor.resultTabs[0].id);
		});

		it('should switch between tabs', async () => {
			const queryIds: QueryId[] = [1, 2];
			const queryText = 'SELECT 1; SELECT 2';
			const connectionId = 'conn-1';

			mockCommands.submitQuery.mockResolvedValue(queryIds);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({ status: 'Completed' })
			);
			mockCommands.getColumns.mockResolvedValue(['col']);
			mockCommands.getPageCount.mockResolvedValue(1);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			const firstTabId = executor.resultTabs[0].id;
			const secondTabId = executor.resultTabs[1].id;

			executor.handleResultTabSelect(firstTabId);
			expect(executor.activeResultTabId).toBe(firstTabId);

			executor.handleResultTabSelect(secondTabId);
			expect(executor.activeResultTabId).toBe(secondTabId);
		});

		it('should close tab and update active tab appropriately', async () => {
			const queryIds: QueryId[] = [1, 2, 3];
			const queryText = 'SELECT 1; SELECT 2; SELECT 3';
			const connectionId = 'conn-1';

			mockCommands.submitQuery.mockResolvedValue(queryIds);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({ status: 'Completed' })
			);
			mockCommands.getColumns.mockResolvedValue(['col']);
			mockCommands.getPageCount.mockResolvedValue(1);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			expect(executor.resultTabs).toHaveLength(3);
			const middleTabId = executor.resultTabs[1].id;

			executor.handleResultTabClose(middleTabId);

			expect(executor.resultTabs).toHaveLength(2);
			expect(executor.resultTabs.find((t) => t.id === middleTabId)).toBeUndefined();
		});

		it('should close active tab and switch to another tab', async () => {
			const queryIds: QueryId[] = [1, 2];
			const queryText = 'SELECT 1; SELECT 2';
			const connectionId = 'conn-1';

			mockCommands.submitQuery.mockResolvedValue(queryIds);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({ status: 'Completed' })
			);
			mockCommands.getColumns.mockResolvedValue(['col']);
			mockCommands.getPageCount.mockResolvedValue(1);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			const firstTabId = executor.resultTabs[0].id;
			const secondTabId = executor.resultTabs[1].id;

			// active tab is the second one (last added)
			expect(executor.activeResultTabId).toBe(secondTabId);

			executor.handleResultTabClose(secondTabId);

			expect(executor.resultTabs).toHaveLength(1);
			expect(executor.activeResultTabId).toBe(firstTabId);
		});

		it('should set activeResultTabId to null when closing last tab', async () => {
			const queryId: QueryId = 1;
			const queryText = 'SELECT 1';
			const connectionId = 'conn-1';

			mockCommands.submitQuery.mockResolvedValue([queryId]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({ status: 'Completed' })
			);
			mockCommands.getColumns.mockResolvedValue(['col']);
			mockCommands.getPageCount.mockResolvedValue(1);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			const tabId = executor.resultTabs[0].id;
			executor.handleResultTabClose(tabId);

			expect(executor.resultTabs).toHaveLength(0);
			expect(executor.activeResultTabId).toBeNull();
		});
	});

	describe('Result States', () => {
		it('should handle query with results status=Completed', async () => {
			const queryId: QueryId = 1;
			const queryText = 'SELECT * FROM users';
			const connectionId = 'conn-1';

			mockCommands.submitQuery.mockResolvedValue([queryId]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({
					status: 'Completed',
					returns_values: true,
					first_page: [[1, 'test']]
				})
			);
			mockCommands.getColumns.mockResolvedValue(['id', 'name']);
			mockCommands.getPageCount.mockResolvedValue(1);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			expect(executor.resultTabs[0]).toMatchObject({
				status: 'Completed',
				queryReturnsResults: true
			});
		});

		it('should handle query without results showing affected_rows', async () => {
			const queryId: QueryId = 1;
			const queryText = 'UPDATE users SET name = "test"';
			const connectionId = 'conn-1';

			mockCommands.submitQuery.mockResolvedValue([queryId]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({
					returns_values: false,
					status: 'Completed',
					first_page: null,
					affected_rows: 42
				})
			);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			expect(executor.resultTabs[0]).toMatchObject({
				status: 'Completed',
				queryReturnsResults: false,
				affectedRows: 42
			});
		});

		it('should show running status initially', async () => {
			const queryId: QueryId = 1;
			const queryText = 'SELECT * FROM users';
			const connectionId = 'conn-1';

			// Delay waitUntilRenderable to simulate running state
			let resolveWait: (value: StatementInfo) => void;
			const waitPromise = new Promise<StatementInfo>((resolve) => {
				resolveWait = resolve;
			});

			mockCommands.submitQuery.mockResolvedValue([queryId]);
			mockCommands.waitUntilRenderable.mockReturnValue(waitPromise);

			const executePromise = executor.executeQuery(queryText, connectionId);
			await flushPromises();

			expect(executor.resultTabs[0]).toMatchObject({
				status: 'Running'
			});

			// Complete the query
			resolveWait!(createMockStatementInfo({ status: 'Completed' }));
			mockCommands.getColumns.mockResolvedValue(['id']);
			mockCommands.getPageCount.mockResolvedValue(1);
			await executePromise;
			await flushPromises();

			expect(executor.resultTabs[0]).toMatchObject({
				status: 'Completed'
			});
		});

		it('should handle failed query with error message', async () => {
			const queryId: QueryId = 1;
			const queryText = 'SELECT * FROM invalid';
			const connectionId = 'conn-1';
			const errorMessage = 'Syntax error';

			mockCommands.submitQuery.mockResolvedValue([queryId]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({
					status: 'Error',
					error: errorMessage
				})
			);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			expect(executor.resultTabs[0]).toMatchObject({
				status: 'Error',
				error: errorMessage
			});
		});

		it('should handle empty result set (0 rows)', async () => {
			const queryId: QueryId = 1;
			const queryText = 'SELECT * FROM users WHERE id = -1';
			const connectionId = 'conn-1';

			mockCommands.submitQuery.mockResolvedValue([queryId]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({
					status: 'Completed',
					returns_values: true,
					first_page: []
				})
			);
			mockCommands.getColumns.mockResolvedValue(['id', 'name']);
			mockCommands.getPageCount.mockResolvedValue(0);

			await executor.executeQuery(queryText, connectionId);
			await flushPromises();

			expect(executor.resultTabs[0]).toMatchObject({
				status: 'Completed',
				queryReturnsResults: true,
				totalPages: 0,
				currentPageData: []
			});
		});
	});

	describe('Pagination', () => {
		beforeEach(() => {
			// Set up a common scenario with paginated results
			mockCommands.submitQuery.mockResolvedValue([1]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({
					status: 'Completed',
					first_page: createMockPage(2)
				})
			);
			mockCommands.getColumns.mockResolvedValue(['id', 'name']);
		});

		it('should load initial page (page 0)', async () => {
			mockCommands.getPageCount.mockResolvedValue(3);

			await executor.executeQuery('SELECT * FROM users', 'conn-1');
			await flushPromises();

			expect(executor.resultTabs[0]).toMatchObject({
				currentPageIndex: 0,
				currentPageData: createMockPage(2)
			});
		});

		it('should load next page', async () => {
			mockCommands.getPageCount.mockResolvedValue(3);
			const page1Data = createMockPage(3);
			mockCommands.fetchPage.mockResolvedValue(page1Data);

			await executor.executeQuery('SELECT * FROM users', 'conn-1');
			await flushPromises();

			const queryId = executor.resultTabs[0].queryId;
			await executor.loadPage(queryId, 1);
			await flushPromises();

			expect(executor.resultTabs[0]).toMatchObject({
				currentPageIndex: 1,
				currentPageData: page1Data
			});
			expect(mockCommands.fetchPage).toHaveBeenCalledWith(queryId, 1);
		});

		it('should load previous page', async () => {
			mockCommands.getPageCount.mockResolvedValue(3);
			const page0Data = createMockPage(2);
			mockCommands.fetchPage.mockResolvedValue(page0Data);

			await executor.executeQuery('SELECT * FROM users', 'conn-1');
			await flushPromises();

			const queryId = executor.resultTabs[0].queryId;

			// Load page 1 first
			await executor.loadPage(queryId, 1);
			await flushPromises();

			// Then go back to page 0
			await executor.loadPage(queryId, 0);
			await flushPromises();

			expect(executor.resultTabs[0]).toMatchObject({
				currentPageIndex: 0
			});
		});

		it('should load specific page number', async () => {
			mockCommands.getPageCount.mockResolvedValue(10);
			const page5Data = createMockPage(4);
			mockCommands.fetchPage.mockResolvedValue(page5Data);

			await executor.executeQuery('SELECT * FROM users', 'conn-1');
			await flushPromises();

			const queryId = executor.resultTabs[0].queryId;
			await executor.loadPage(queryId, 5);
			await flushPromises();

			expect(executor.resultTabs[0]).toMatchObject({
				currentPageIndex: 5,
				currentPageData: page5Data
			});
			expect(mockCommands.fetchPage).toHaveBeenCalledWith(queryId, 5);
		});

		it('should handle page loading for non-existent tab gracefully', async () => {
			mockCommands.getPageCount.mockResolvedValue(2);

			await executor.executeQuery('SELECT * FROM users', 'conn-1');
			await flushPromises();

			// Try to load page for a non-existent query ID
			await executor.loadPage(9999, 1);
			await flushPromises();

			// Should not throw, tab should remain unchanged
			expect(executor.resultTabs[0].currentPageIndex).toBe(0);
		});
	});

	describe('Polling & Cleanup', () => {
		it('should poll for page count while query is running', async () => {
			vi.useFakeTimers();

			try {
				const queryId: QueryId = 1;
				let getStatusCallCount = 0;
				
				mockCommands.submitQuery.mockResolvedValue([queryId]);
				mockCommands.waitUntilRenderable.mockResolvedValue(
					createMockStatementInfo({
						status: 'Running',
						returns_values: true,
						first_page: [[1]]
					})
				);
				mockCommands.getColumns.mockResolvedValue(['id']);
				
				// Make status transition happen after a couple polls
				mockCommands.getQueryStatus.mockImplementation(() => {
					getStatusCallCount++;
					if (getStatusCallCount <= 2) {
						return Promise.resolve('Running' as QueryStatus);
					}
					return Promise.resolve('Completed' as QueryStatus);
				});
				
				mockCommands.getPageCount.mockImplementation(() => {
					if (getStatusCallCount <= 2) {
						return Promise.resolve(null as any);
					}
					return Promise.resolve(5);
				});

				const executePromise = executor.executeQuery('SELECT * FROM users', 'conn-1');
				await vi.runOnlyPendingTimersAsync();

				// Initial poll should have occurred, verify polling is set up
				expect(getStatusCallCount).toBeGreaterThanOrEqual(1);

				// Advance through multiple poll cycles
				await vi.advanceTimersByTimeAsync(400);

				// Should have polled multiple times and eventually completed
				expect(getStatusCallCount).toBeGreaterThanOrEqual(3);
				expect(executor.resultTabs[0]).toMatchObject({
					status: 'Completed',
					totalPages: 5
				});

				await executePromise;
			} finally {
				vi.useRealTimers();
			}
		});

		it('should stop polling when query is completed', async () => {
			vi.useFakeTimers();

			try {
				const queryId: QueryId = 1;
				mockCommands.submitQuery.mockResolvedValue([queryId]);
				mockCommands.waitUntilRenderable.mockResolvedValue(
					createMockStatementInfo({
						status: 'Running',
						returns_values: true,
						first_page: [[1]]
					})
				);
				mockCommands.getColumns.mockResolvedValue(['id']);
				mockCommands.getQueryStatus.mockResolvedValue('Completed' as QueryStatus);
				mockCommands.getPageCount.mockResolvedValue(3);

				const executePromise = executor.executeQuery('SELECT * FROM users', 'conn-1');
				await vi.runOnlyPendingTimersAsync();

				// Advance timers to trigger one poll
				await vi.advanceTimersByTimeAsync(200);

				const pollCallCount = mockCommands.getQueryStatus.mock.calls.length;

				// Advance much further - polling should have stopped
				await vi.advanceTimersByTimeAsync(2000);

				// Should not have called getQueryStatus more times
				expect(mockCommands.getQueryStatus.mock.calls.length).toBe(pollCallCount);

				await executePromise;
			} finally {
				vi.useRealTimers();
			}
		});

		it('should clear all intervals on dispose', async () => {
			vi.useFakeTimers();

			try {
				const queryIds: QueryId[] = [1, 2];
				mockCommands.submitQuery.mockResolvedValue(queryIds);
				mockCommands.waitUntilRenderable.mockResolvedValue(
					createMockStatementInfo({
						status: 'Running',
						returns_values: true,
						first_page: [[1]]
					})
				);
				mockCommands.getColumns.mockResolvedValue(['id']);
				mockCommands.getQueryStatus.mockResolvedValue('Running' as QueryStatus);
				mockCommands.getPageCount.mockResolvedValue(null as any);

				const executePromise = executor.executeQuery('SELECT 1; SELECT 2', 'conn-1');
				await vi.runOnlyPendingTimersAsync();

				// Should have started polling for both queries
				const statusCallsBefore = mockCommands.getQueryStatus.mock.calls.length;

				executor.dispose();

				// Advance timers - polling should not continue
				await vi.advanceTimersByTimeAsync(1000);

				// No new calls should have been made
				expect(mockCommands.getQueryStatus.mock.calls.length).toBe(statusCallsBefore);

				await executePromise.catch(() => {}); // May have been interrupted
			} finally {
				vi.useRealTimers();
			}
		});

		it('should be safe to call dispose multiple times', () => {
			executor.dispose();
			executor.dispose();
			executor.dispose();

			// Should not throw
			expect(true).toBe(true);
		});

		it('should clear old intervals when executing new query', async () => {
			vi.useFakeTimers();

			try {
				// Execute first query with polling
				mockCommands.submitQuery.mockResolvedValue([1]);
				mockCommands.waitUntilRenderable.mockResolvedValue(
					createMockStatementInfo({
						status: 'Running',
						returns_values: true,
						first_page: [[1]]
					})
				);
				mockCommands.getColumns.mockResolvedValue(['id']);
				mockCommands.getQueryStatus.mockResolvedValue('Running' as QueryStatus);
				mockCommands.getPageCount.mockResolvedValue(null as any);

				const execute1Promise = executor.executeQuery('SELECT 1', 'conn-1');
				await vi.runOnlyPendingTimersAsync();

				// Execute second query (should clear first query's intervals)
				mockCommands.submitQuery.mockResolvedValue([2]);
				const execute2Promise = executor.executeQuery('SELECT 2', 'conn-1');
				await vi.runOnlyPendingTimersAsync();

				// Advance timers
				await vi.advanceTimersByTimeAsync(200);

				// Should only be polling for the second query (queryId 2)
				// The mock doesn't distinguish between queries, but we verify no errors occur
				expect(executor.resultTabs).toHaveLength(1);
				expect(executor.resultTabs[0].queryId).toBe(2);

				await execute1Promise.catch(() => {}); // May have been cleared
				await execute2Promise.catch(() => {}); // May still be running
			} finally {
				vi.useRealTimers();
			}
		});
	});

	describe('Edge Cases', () => {
		it('should generate tab title correctly for short queries', async () => {
			const shortQuery = 'SELECT 1';
			mockCommands.submitQuery.mockResolvedValue([1]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({ 
					status: 'Completed', 
					returns_values: false,
					affected_rows: 1
				})
			);

			await executor.executeQuery(shortQuery, 'conn-1');
			await flushPromises();

			expect(executor.resultTabs[0].name).toBe(shortQuery);
		});

		it('should truncate long query titles', async () => {
			const longQuery = 'SELECT * FROM users WHERE name LIKE "%test%" AND age > 18';
			mockCommands.submitQuery.mockResolvedValue([1]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({ 
					status: 'Completed', 
					returns_values: false,
					affected_rows: 0
				})
			);

			await executor.executeQuery(longQuery, 'conn-1');
			await flushPromises();

			expect(executor.resultTabs[0].name.length).toBeLessThanOrEqual(30);
			expect(executor.resultTabs[0].name.endsWith('...')).toBe(true);
		});

		it('should handle query with many columns', async () => {
			const manyColumns = Array.from({ length: 100 }, (_, i) => `col${i}`);
			mockCommands.submitQuery.mockResolvedValue([1]);
			mockCommands.waitUntilRenderable.mockResolvedValue(
				createMockStatementInfo({ 
					status: 'Completed', 
					returns_values: true,
					first_page: [[]] 
				})
			);
			mockCommands.getColumns.mockResolvedValue(manyColumns);
			mockCommands.getPageCount.mockResolvedValue(1);

			await executor.executeQuery('SELECT * FROM wide_table', 'conn-1');
			await flushPromises();

			expect(executor.resultTabs[0].columns).toHaveLength(100);
		});

		it('should handle failure to get column information', async () => {
			vi.useFakeTimers();

			try {
				mockCommands.submitQuery.mockResolvedValue([1]);
				mockCommands.waitUntilRenderable.mockResolvedValue(
					createMockStatementInfo({
						status: 'Completed',
						returns_values: true,
						first_page: [[1]]
					})
				);
				// Always return null to simulate column fetch failure
				mockCommands.getColumns.mockResolvedValue(null);

				const executePromise = executor.executeQuery('SELECT * FROM users', 'conn-1');
				
				// Run through all the polling attempts (50 retries * 100ms = 5000ms)
				await vi.advanceTimersByTimeAsync(5100);
				await executePromise;

				expect(executor.resultTabs[0]).toMatchObject({
					status: 'Error',
					error: 'Failed to get column information'
				});
			} finally {
				vi.useRealTimers();
			}
		});

		it('should return correct tab status for getTabStatus', async () => {
			mockCommands.submitQuery.mockResolvedValue([1, 2, 3]);
			mockCommands.waitUntilRenderable
				.mockResolvedValueOnce(
					createMockStatementInfo({ 
						status: 'Completed', 
						returns_values: false,
						affected_rows: 1
					})
				)
				.mockResolvedValueOnce(
					createMockStatementInfo({ 
						status: 'Running', 
						returns_values: false 
					})
				)
				.mockResolvedValueOnce(
					createMockStatementInfo({ 
						status: 'Error', 
						error: 'Test error' 
					})
				);

			await executor.executeQuery('SELECT 1; SELECT 2; SELECT 3', 'conn-1');
			await flushPromises();

			expect(executor.getTabStatus(executor.resultTabs[0])).toBe('normal');
			expect(executor.getTabStatus(executor.resultTabs[1])).toBe('modified');
			expect(executor.getTabStatus(executor.resultTabs[2])).toBe('error');
		});
	});
});

