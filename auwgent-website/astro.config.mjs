// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	integrations: [
		starlight({
			title: 'My Docs',
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/withastro/starlight' }],
			sidebar: [
				{
					label: 'Introduction',
					items: [
						{ label: 'What is Auwgent?', slug: 'introduction' },
					],
				},
				{
					label: 'Guides',
					items: [
						{ label: 'Getting Started', slug: 'guides/getting-started' },
						{ label: 'Example Guide', slug: 'guides/example' },
					],
				},
				{
					label: 'Core Concepts',
					items: [
						{ label: 'The Agent', slug: 'core-concepts/agent' },
						{ label: "Prompt and Context", slug: 'core-concepts/prompt-context' },
						{ label: "Input and Ouput", slug: "core-concepts/input-output" },
						{ label: "Tools", slug: "core-concepts/tools" },
						{ label: "Workflows", slug: "core-concepts/workflows" },
						{ label: "Helpers", slug: "core-concepts/helpers" },
						{ label: "Organizing Your Codebase", slug: "core-concepts/organize-codebase" },
						{ label: "session", slug: "core-concepts/sessions" },
						{ label: "Middleware", slug: "core-concepts/middleware" }
					],
				},
				{
					label: 'Reference',
					autogenerate: { directory: 'reference' },
				},
			],
		}),
	],
});
