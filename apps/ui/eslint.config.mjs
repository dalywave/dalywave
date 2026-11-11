import withNuxt from './.nuxt/eslint.config.mjs'

export default withNuxt({
  rules: {
    'vue/multi-word-component-names': 'off',
    // Необязательные пропсы в TypeScript и так undefined по умолчанию
    'vue/require-default-prop': 'off',
  },
})
