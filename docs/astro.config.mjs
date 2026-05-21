import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://MarcosAndradeV.github.io',
  base: '/sel',
  integrations: [
    starlight({
      title: 'sel Lisp',
      social: {
        github: 'https://github.com/MarcosAndradeV/sel',
      },
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Introduction', link: '/sel/' },
          ],
        },
        {
          label: 'Guides & References',
          items: [
            { label: 'Language Reference', link: '/sel/reference' },
            { label: 'Interactive REPL', link: '/sel/repl' },
            { label: 'Standard & Core Library', link: '/sel/core' },
          ],
        },
      ],
    }),
  ],
});
