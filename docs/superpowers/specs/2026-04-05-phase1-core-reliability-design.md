# Phase 1: Core Reliability Design (Computer Use CLI)

## Context
This project aims to improve the codebase of an AI Agent Computer Use CLI by addressing common limitations discussed by the community. Phase 1 focuses on the two most critical causes of misclicks: Retina/High-DPI display scaling mismatches and Animation Instability (clicking elements before they finish moving).

## Architecture & Components

### 1. DPI Scaling Strategy (Physical vs Logical)
AI vision models typically output coordinates in physical pixels (e.g., 2560x1600), while OS input APIs often expect logical coordinates (e.g., 1280x800).

- **`Platform` Trait Extension**: Add a `get_display_scale(&self) -> f64` method to the core platform trait (`agent-click-core/src/platform.rs`).
- **macOS Implementation**: `agent-click-macos` will use `CGDisplayPixelsWide` vs `CGDisplayBounds` or `NSScreen::backingScaleFactor` to calculate the scale factor.
- **Windows Implementation**: `agent-click-windows` will use `GetDpiForMonitor` or `GetScaleFactorForMonitor` to retrieve the DPI and convert it to a scaling ratio (e.g., 144 DPI / 96 = 1.5).
- **CLI Translation Layer**: In `actions.rs`, whenever the CLI receives an interaction command (click, type, drag) with physical coordinates (or bounding boxes derived from vision), it will automatically divide the coordinates by the `get_display_scale()` factor before executing the OS-level input simulation.

### 2. Animation Stability (Hybrid Approach)
Clicking an element while it is animating (e.g., sliding menus, expanding modals) causes the AI to miss the target.

- **`poll_for_stability(node, timeout)`**: A new utility function in `cli/src/wait.rs`.
- **Default Fallback (Approach A)**: It will record the element's bounding box `(x, y, width, height)`. It will wait 50ms, retrieve the bounding box again, and compare. If the box is identical across two consecutive frames, the element is considered stable.
- **Windows Fast-Path (Approach B)**: `agent-click-windows` will expose a capability to listen for UIAutomation `BoundingRectangleProperty` changes. If the target application supports UIA events, the platform will use native event listening to block until the property stops firing change events. If it times out or is unsupported, it falls back to the 50ms polling loop to guarantee success.

## Error Handling
- If `get_display_scale()` fails to determine a valid monitor, it must default to `1.0` and log a warning.
- If `poll_for_stability()` exceeds the user-defined timeout (e.g., 5 seconds), it will return a `TimeoutError` rather than proceeding with a click on a moving target.

## Testing Strategy
- Add unit tests for DPI calculation mock functions.
- Add integration tests for the `poll_for_stability` loop using a dummy element that intentionally changes its `(x,y)` coordinates for the first 3 polls.

## Scope & Boundaries
This phase is strictly limited to coordinate scaling and bounding-box stability. Smart scrolling (Phase 2) and advanced window management (Phase 3) will be handled in subsequent specs to maintain isolation and clarity.
