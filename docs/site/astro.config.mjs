// Beholder documentation (Starlight). Local only for now: `npm run dev`.
import { defineConfig, passthroughImageService } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  // No image processing, so no native `sharp` binary to download.
  image: { service: passthroughImageService() },
  integrations: [
    starlight({
      title: 'Beholder',
      description: 'Companion app for D&D 5e (2024) campaigns.',
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
        fr: { label: 'Français', lang: 'fr' },
      },
      customCss: ['./src/styles/theme.css'],
      sidebar: [
        {
          label: 'Players',
          translations: { fr: 'Joueurs' },
          items: [{ autogenerate: { directory: 'players' } }],
        },
        {
          label: 'Server admin',
          translations: { fr: 'Administration du serveur' },
          items: [{ autogenerate: { directory: 'admin' } }],
        },
        {
          label: 'Developers',
          translations: { fr: 'Développement' },
          items: [{ autogenerate: { directory: 'dev' } }],
        },
      ],
    }),
  ],
});
