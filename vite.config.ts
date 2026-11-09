import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;
// El 1520 y no el 1420 de la plantilla de Tauri, que usa otro proyecto de Pablo;
// HMR va en el siguiente. Tiene que coincidir con `build.devUrl` de
// `tauri.conf.json`. `PB_PUERTO` lo cambia (con
// `--config '{"build":{"devUrl":"http://localhost:<puerto>"}}'` en `tauri dev`).
const puerto = Number(process.env.PB_PUERTO) || 1520;

// https://vite.dev/config/
export default defineConfig(async () => ({
	plugins: [react(), tailwindcss()],
	// Los dados 3D se cargan con `import()` al tirar: sin esto, en desarrollo
	// Vite los descubre entonces, los optimiza y recarga la página.
	// Y el escaneo de dependencias, solo desde la app (no los `.html` de `plans/`).
	optimizeDeps: {
		entries: ["index.html"],
		include: ["three", "@dimforge/rapier3d-deterministic-compat"],
	},

	resolve: {
		alias: {
			"@": path.resolve(__dirname, "./src"),
		},
	},

	// Opciones para `tauri dev` / `tauri build`:
	// 1. que Vite no tape los errores de Rust,
	clearScreen: false,
	// 2. Tauri espera un puerto fijo; si está ocupado, fallar en vez de buscar otro,
	server: {
		port: puerto,
		strictPort: true,
		host: host || false,
		hmr: host ? { protocol: "ws", host, port: puerto + 1 } : undefined,
		// 3. y no vigilar `src-tauri` (lo recompila Cargo).
		watch: { ignored: ["**/src-tauri/**"] },
	},
}));
