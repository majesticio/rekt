# Rekt Codebase Guidelines

## Build/Dev Commands
- `npm run dev` - Start development server
- `npm run build` - Build the application
- `npm run check` - Type check the codebase
- `npm run check:watch` - Type check with watch mode
- `npm run tauri dev` - Run Tauri development mode (recommended for local development)
- `npm run tauri build` - Build Tauri application for production
- `npm run preview` - Preview the built application
- `cargo test -p rekt_lib -- test_name` - Run a specific Rust test
- `cargo test -p rekt_lib` - Run all Rust tests
- `cargo check` - Check Rust code for errors without building
- `cargo clippy` - Run Rust linter
- `./build-appimage.sh` - Build AppImage for Linux distribution

## Code Style Guidelines
- **TypeScript**: Strict mode enabled, use PascalCase for types/interfaces
- **Imports**: Group imports in order: standard library, external packages, internal modules
- **Formatting**: 2-space indentation in TS/Svelte files, 4-space in Rust
- **Components**: Use Svelte 5 runes ($state, $derived) for reactive state management
- **Naming**: camelCase for JS/TS variables/functions, PascalCase for components, snake_case for Rust
- **State Management**: Store UI state with Svelte's reactive primitives, use Rust for persistent state
- **Types**: Provide explicit return types on functions, avoid `any`, use interfaces for complex structures
- **Error Handling**: Use try/catch for JS/TS async operations, Result/Option for Rust with descriptive messages
- **File Structure**: Follow SvelteKit conventions with +page.svelte pattern
- **Path Aliases**: Use $lib for imports from src/lib folder

## Project Architecture
- **Frontend**: SvelteKit with TypeScript and Vite
- **Backend**: Tauri with Rust
- **Audio Processing**: Uses cpal library for audio recording and hound for WAV file creation
- **Persistence**: Saves recordings to app data directory using Tauri's filesystem APIs
- **IPC**: Tauri commands for frontend-backend communication
- **Thread Safety**: Uses Arc, Mutex, and AtomicBool for thread-safe state management in Rust