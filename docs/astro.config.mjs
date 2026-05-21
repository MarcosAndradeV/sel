import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
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
            { label: 'Introduction', link: '/' },
          ],
        },
        {
          label: 'Guides & References',
          items: [
            { label: 'Language Reference', link: '/reference' },
            { label: 'Standard & Core Library', link: '/core' },
          ],
        },
      ],
    }),
  ],
});
