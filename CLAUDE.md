# Rekt Codebase Guidelines

## Build/Dev Commands
- `npm run dev` - Start development server
- `npm run build` - Build the application
- `npm run check` - Type check the codebase
- `npm run check:watch` - Type check with watch mode
- `npm run tauri dev` - Run Tauri development mode
- `npm run tauri build` - Build Tauri application
- `npm run preview` - Preview the built application
- `cargo test -p rekt_lib -- test_name` - Run a specific Rust test
- `cargo check` - Check Rust code for errors without building
- `./build-appimage.sh` - Build AppImage for Linux

## Code Style Guidelines
- **TypeScript**: Strict mode enabled, consistent PascalCase for types
- **Imports**: Group imports (standard library, external, internal)
- **Formatting**: Use 2-space indentation in TS/Svelte files, 4-space in Rust
- **Components**: Use Svelte 5 runes ($state, $derived) for reactive state
- **Naming**: camelCase for JS/TS variables/functions, PascalCase for components, snake_case for Rust
- **Types**: Explicit return types on functions, avoid `any`, use interfaces for complex structures
- **Error Handling**: Use try/catch blocks for async operations with detailed error messages
- **File Structure**: Follow SvelteKit conventions with +page.svelte pattern
- **Path Aliases**: Use $lib for imports from src/lib folder

## Project Architecture
- **Frontend**: SvelteKit with TypeScript and Vite
- **Backend**: Tauri with Rust
- **Audio Processing**: Uses cpal library for audio recording and hound for WAV file creation
- **Persistence**: Saves recordings to app data directory
- **IPC**: Tauri commands for frontend-backend communication