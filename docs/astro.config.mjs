import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://MarcosAndradeV.github.io',
  base: '/sel',
  integrations: [
    starlight({
      title: 'sel Lisp',
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/MarcosAndradeV/sel' },
      ],
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Introduction', link: '/' },
          ],
        },
        {
          label: 'Guides & References',
          items: [
            { label: 'Language Reference', link: '/reference' },
            { label: 'Interactive REPL', link: '/repl' },
            { label: 'Standard & Core Library', link: '/core' },
          ],
        },
      ],
    }),
  ],
});
