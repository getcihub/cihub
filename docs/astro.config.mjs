import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
	site: 'https://docs.cihub.io',
	base: '/',
	image: {
		service: { entrypoint: 'astro/assets/services/noop' },
	},
	integrations: [
		starlight({
			title: 'CIHub',
			description: 'Supercharged GitHub Actions runners',
			social: {
				github: 'https://github.com/getcihub/cihub',
			},
			customCss: ['./src/styles/custom.css'],
			head: [
				{
					tag: 'script',
					content: `
            localStorage.setItem('starlight-theme', 'dark');
            document.documentElement.dataset.theme = 'dark';
          `,
				},
			],
			components: {
				ThemeSelect: './src/components/ThemeSelect.astro',
			},
			sidebar: [
				{ label: 'Welcome', link: '/' },
				{
					label: 'Getting Started',
					autogenerate: { directory: 'getting-started' },
				},
			],
			editLink: {
				baseUrl: 'https://github.com/getcihub/cihub/edit/main/docs/',
			},
			lastUpdated: true,
		}),
	],
});
