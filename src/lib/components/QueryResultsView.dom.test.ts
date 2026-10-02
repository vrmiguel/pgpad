import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import QueryResultsView from './QueryResultsView.svelte';
import { Commands, type Page, type QueryEvent } from '$lib/commands.svelte';

let eventHandler: (event: QueryEvent) => void;
let component: ReturnType<typeof mount> | undefined;

async function settle() {
	await new Promise<void>((resolve) => setTimeout(resolve, 0));
	flushSync();
}

async function emit(event: QueryEvent) {
	eventHandler(event);
	await settle();
}

function firstColumn() {
	return [...document.querySelectorAll('tbody tr')].map((row) =>
		row.querySelector('td')?.textContent?.trim()
	);
}

beforeEach(() => {
	vi.spyOn(Commands, 'listenQueryEvents').mockImplementation(async (handler) => {
		eventHandler = handler;
		return vi.fn();
	});
	vi.spyOn(Commands, 'submitQuery').mockResolvedValue([1]);
	vi.spyOn(Commands, 'fetchPage').mockResolvedValue([[1]]);
	vi.spyOn(console, 'log').mockImplementation(() => {});
});

afterEach(async () => {
	if (component) await unmount(component);
	component = undefined;
	vi.restoreAllMocks();
	document.body.replaceChildren();
});

it('updates sorted rows and pagination, and opens JSON from the replacement page', async () => {
	const first: Page = [
		[2, { message: 'old-two' }],
		[1, { message: 'old-one' }]
	];
	const second: Page = [
		[4, { message: 'new-four' }],
		[3, { message: 'new-three' }]
	];
	vi.mocked(Commands.fetchPage).mockImplementation(async (_queryId, index) =>
		index === 0 ? first : second
	);
	component = mount(QueryResultsView, {
		target: document.body,
		props: { query: 'SELECT results', connectionId: 'connection-1' }
	});
	flushSync();
	await settle();
	await emit({ type: 'columns_ready', query_id: 1, columns: ['id', 'payload'] });
	await emit({ type: 'page_ready', query_id: 1, page_index: 0, page_count: 2 });
	expect(firstColumn()).toEqual(['2', '1']);
	document.querySelector<HTMLButtonElement>('th button')!.click();
	flushSync();
	expect(firstColumn()).toEqual(['1', '2']);
	[...document.querySelectorAll<HTMLButtonElement>('button')]
		.find((element) => !element.closest('table') && element.textContent?.trim() === '2')!
		.click();
	await settle();
	expect(firstColumn()).toEqual(['3', '4']);
	expect(document.body.textContent).toContain('Page 2 of 2');

	document.querySelector<HTMLButtonElement>('[title="Inspect JSON"]')!.click();
	flushSync();
	const inspector = document.querySelector('[role="dialog"]')!;
	expect(inspector.textContent).toContain('new-three');
	expect(inspector.textContent).not.toContain('old-one');
});
