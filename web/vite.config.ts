import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import wasm from "vite-plugin-wasm";

// https://vitejs.dev/config/
export default defineConfig({
	plugins: [wasm(), vue()],
	build: {
		target: 'esnext',
	},
	base: process.env.NODE_ENV === 'production' ? '/logic-mesh/' : undefined,
})
