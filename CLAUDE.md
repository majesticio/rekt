# Rekt Codebase Guidelines

## Build/Dev Commands
- `npm run dev` - Start development server
- `npm run build` - Build the application
- `npm run check` - Type check the codebase
- `npm run check:watch` - Type check with watch mode
- `npm run tauri dev` - Run Tauri development mode
- `npm run tauri build` - Build Tauri application
- `cargo test -p rekt_lib -- test_name` - Run a specific Rust test

## Code Style Guidelines
- **TypeScript**: Strict mode enabled, consistent casing in filenames
- **Imports**: Group imports (standard library, external, internal)
- **Formatting**: Use 2-space indentation in TypeScript/Svelte files
- **Components**: Use Svelte 5 syntax with runes ($state, $derived)
- **Naming**: camelCase for variables/functions, PascalCase for components
- **Types**: Explicit return types on functions, avoid `any`
- **Error Handling**: Use try/catch for async operations
- **File Structure**: Follow SvelteKit conventions with +page.svelte pattern
- **Path Aliases**: Use $lib for imports from src/lib folder

## Project Structure
- Frontend: SvelteKit with TypeScript and Vite
- Backend: Tauri with Rust