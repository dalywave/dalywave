import withNuxt from './.nuxt/eslint.config.mjs'

export default withNuxt({
  rules: {
    'vue/multi-word-component-names': 'off',
    // Необязательные пропсы в TypeScript и так undefined по умолчанию
    'vue/require-default-prop': 'off',
    // На http не с localhost (своя установка по адресу в сети) этих API у браузера нет
    'no-restricted-properties': ['error',
      { object: 'crypto', property: 'randomUUID', message: 'uuid() из ~/utils/web-api' },
      { object: 'navigator', property: 'clipboard', message: 'copyText() из ~/utils/web-api' },
    ],
  },
})
