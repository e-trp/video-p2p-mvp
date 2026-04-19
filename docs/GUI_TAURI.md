# Tauri GUI Plan

## Why Tauri

Tauri is a reasonable fit for this project because:

- Rust remains the primary application language
- the desktop shell can stay lightweight
- the UI can evolve independently from the media backend
- platform-specific capture code can still live in Rust/native layers

## Current State

The repository now includes a desktop scaffold in `apps/desktop`:

- static frontend in `apps/desktop/public`
- Tauri Rust shell in `apps/desktop/src-tauri`
- initial commands for reading project status and the saved specification

## Intended GUI Responsibilities

1. Window/source selection
2. Room create/join flow
3. Start and stop streaming
4. Stream status and diagnostics
5. Settings for bitrate, fps, and audio routing

## Backend Split

- `app-core`: shared protocol, session state, mock transport today
- `transport-webrtc`: future real peer transport
- `capture-macos`: future ScreenCaptureKit bridge
- `capture-linux`: future Portal + PipeWire integration
- `desktop-app`: Tauri shell and user-facing commands

## Important Note

The Tauri shell is scaffolded but not included in the default workspace build yet. This avoids making the entire repo dependent on downloading Tauri crates and installing platform prerequisites before the core architecture is in place.
