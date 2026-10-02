import { configDefaults, defineConfig } from 'vitest/config';

export default defineConfig({
	test: {
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'unit',
					exclude: [...configDefaults.exclude, '**/*.dom.test.ts']
				}
			},
			{
				extends: './vite.config.ts',
				resolve: { conditions: ['browser'] },
				test: {
					name: 'dom',
					environment: 'happy-dom',
					include: ['src/**/*.dom.test.ts']
				}
			}
		]
	}
});
