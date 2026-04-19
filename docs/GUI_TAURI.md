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
- session manager hooks for `host`, `join`, `stop`, `status`, and `logs`
- UI controls for config save, host/viewer prep, mock stream stage, planned WebRTC stage, reset, refresh, and log clearing

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

## Session Management Layer

The current GUI iteration is backed by an in-memory session manager in `app-core`.

It already models:

- idle / host / viewer roles
- configured / awaiting peer / mock streaming / planned WebRTC / stopped stages
- room and signaling metadata
- a rolling in-memory event log

This gives the Tauri shell a stable API before the real transport and capture layers are connected.

## Current User Interface

The current Tauri UI is fully wired to the in-memory session manager and supports:

- editing room, signaling, and source label fields
- saving session configuration
- preparing host or viewer session states
- switching to mock streaming or planned WebRTC transport stages
- stopping or resetting a session
- viewing current session status and next action
- viewing and clearing the rolling session log

## Important Note

The Tauri shell is scaffolded but not included in the default workspace build yet. This avoids making the entire repo dependent on downloading Tauri crates and installing platform prerequisites before the core architecture is in place.
