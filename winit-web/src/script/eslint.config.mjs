import eslint from '@eslint/js'
import tseslint from 'typescript-eslint'
import globals from 'globals'

export default tseslint.config(
	{
		ignores: ['**/*.min.js', 'eslint.config.mjs'],
	},
	eslint.configs.recommended,
	...tseslint.configs.strictTypeChecked,
	...tseslint.configs.stylisticTypeChecked,
	{
		languageOptions: {
			parserOptions: {
				ecmaVersion: 'latest',
				project: ['tsconfig.json'],
				sourceType: 'module',
			},
			globals: {
				...globals.browser,
			},
		},
		rules: {
			'@typescript-eslint/no-confusing-void-expression': [
				'error',
				{
					ignoreArrowShorthand: true,
				},
			],
		},
	}
)
