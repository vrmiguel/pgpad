import assert from 'node:assert/strict';
import { describe, it } from 'mocha';

describe('pgpad smoke test', () => {
	it('loads the desktop app window', async () => {
		const appRoot = $('#app');
		await appRoot.waitForExist({ timeout: 15000 });

		const title = await browser.getTitle();
		assert.equal(title, 'pgpad');
	});
});
