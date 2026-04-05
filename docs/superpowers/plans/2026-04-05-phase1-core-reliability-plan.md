# Phase 1 Core Reliability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement physical-to-logical pixel scaling (DPI awareness) and animation stability checks to prevent AI misclicks on Retina displays and moving elements.

**Architecture:** 
1. Expand the `Platform` trait to include `get_display_scale()`. Implement this natively for macOS and Windows. Automatically scale coordinates in the CLI layer before passing them to OS interactions.
2. Add a `poll_for_stability` function in `wait.rs` that re-fetches an element's bounding box over consecutive frames (50ms apart) until it stops changing, preventing clicks on animating elements.

**Tech Stack:** Rust, macOS CoreGraphics (`core-graphics` crate), Windows UIAutomation/Win32 (`windows` crate), Tokio.

---

### Task 1: Extend Platform Trait with Display Scaling

**Files:**
- Modify: `cli/crates/agent-click-core/src/platform.rs`

- [ ] **Step 1: Add `get_display_scale` to `Platform` trait**

```rust
    async fn check_permissions(&self) -> Result<bool>;

    /// Returns the scale factor of the primary display (e.g., 2.0 for Retina, 1.0 for standard).
    /// Physical pixels from vision models should be divided by this value to get logical OS coordinates.
    fn get_display_scale(&self) -> f64 {
        1.0
    }

    async fn activate(&self, _app: &str) -> Result<()> {
```

- [ ] **Step 2: Commit**

```bash
git add cli/crates/agent-click-core/src/platform.rs
git commit -m "feat(core): add get_display_scale to Platform trait with default 1.0"
```

---

### Task 2: Implement Display Scaling for macOS

**Files:**
- Modify: `cli/crates/agent-click-macos/src/platform.rs`

- [ ] **Step 1: Implement `get_display_scale` for `MacOSPlatform`**

Add the method to the `impl Platform for MacOSPlatform` block. We will use `core_graphics::display::CGDisplay::main().pixels_wide()` and `bounds().width` to calculate the scale factor.

```rust
    async fn check_permissions(&self) -> Result<bool> {
        Ok(crate::input::check_accessibility_permissions())
    }

    fn get_display_scale(&self) -> f64 {
        let display = core_graphics::display::CGDisplay::main();
        let physical_width = display.pixels_wide() as f64;
        let logical_width = display.bounds().size.width;
        if logical_width > 0.0 {
            physical_width / logical_width
        } else {
            1.0
        }
    }

    async fn activate(&self, app_name: &str) -> Result<()> {
```

- [ ] **Step 2: Commit**

```bash
git add cli/crates/agent-click-macos/src/platform.rs
git commit -m "feat(macos): implement get_display_scale using CoreGraphics"
```

---

### Task 3: Implement Display Scaling for Windows

**Files:**
- Modify: `cli/crates/agent-click-windows/src/platform.rs`

- [ ] **Step 1: Implement `get_display_scale` for `WindowsPlatform`**

In the `real` module, add `get_display_scale` to the `impl Platform for WindowsPlatform` block. We'll use `GetDpiForSystem()` from `windows::Win32::UI::HiDpi` divided by the standard DPI (96.0).

*Note: Ensure you add the method inside the `#[cfg(target_os = "windows")]` block.*

```rust
    async fn check_permissions(&self) -> Result<bool> {
        Ok(true) // UIAutomation doesn't require special permissions like macOS AX
    }

    fn get_display_scale(&self) -> f64 {
        unsafe {
            // Standard DPI is 96.0
            let dpi = windows::Win32::UI::HiDpi::GetDpiForSystem();
            dpi as f64 / 96.0
        }
    }

    async fn activate(&self, app_name: &str) -> Result<()> {
```

- [ ] **Step 2: Commit**

```bash
git add cli/crates/agent-click-windows/src/platform.rs
git commit -m "feat(windows): implement get_display_scale using GetDpiForSystem"
```

---

### Task 4: Apply Display Scaling in CLI Coordinate Fallbacks

**Files:**
- Modify: `cli/src/actions.rs`

- [ ] **Step 1: Apply scaling in `click`**

When falling back to pointer coordinates, we must scale physical pixels to logical pixels.

```rust
    let scale = platform.get_display_scale();
    
    // Apply scaling if node has a center
    let (scaled_x, scaled_y) = if let Some(center) = node.center() {
        (center.x / scale, center.y / scale)
    } else {
        (0.0, 0.0) // Fallback handled by individual platform errors if needed
    };

    let target_pid = chain.first().app.as_deref().and_then(|app| {
```

Modify the `agent_click_macos::input::click` and `agent_click_windows::input::click` calls to use `scaled_x` and `scaled_y`.

```rust
    #[cfg(target_os = "macos")]
    {
        agent_click_macos::input::click(
            agent_click_core::node::Point { x: scaled_x, y: scaled_y },
            button,
            count,
            target_pid,
        )?;
        return Ok(agent_click_core::action::ActionResult::Clicked);
    }
```

*Apply similar modifications to the Windows click fallback block.*

- [ ] **Step 2: Apply scaling in handlers `drag`**

Modify `cli/src/cli/handlers.rs` in the `Command::Drag` match arm:

```rust
            let scale = platform.get_display_scale();
            let from_point = if let Some(sel) = from {
                let chain = actions::parse_selector_with_app(&sel, app.as_deref())?;
                let node = actions::find_element(platform, &chain, timeout).await?;
                agent_click_core::element::check_visible(&node)?;
                let center = node.center()
                    .ok_or_else(|| agent_click_core::Error::PlatformError {
                        message: "drag source has no position".into(),
                    })?;
                agent_click_core::node::Point { x: center.x / scale, y: center.y / scale }
            } else {
                // ... handle coordinate scaling ...
```

- [ ] **Step 3: Commit**

```bash
git add cli/src/actions.rs cli/src/cli/handlers.rs
git commit -m "feat(cli): apply get_display_scale to coordinate fallbacks"
```

---

### Task 5: Add Animation Stability Polling

**Files:**
- Modify: `cli/src/wait.rs`
- Modify: `cli/src/actions.rs`

- [ ] **Step 1: Implement `poll_for_stability` in `cli/src/wait.rs`**

```rust
pub async fn poll_for_stability(
    platform: &dyn Platform,
    chain: &SelectorChain,
    initial_node: AccessibilityNode,
    timeout: Duration,
) -> agent_click_core::Result<AccessibilityNode> {
    let start = Instant::now();
    let mut current_node = initial_node;
    let poll_interval = Duration::from_millis(50);

    loop {
        tokio::time::sleep(poll_interval).await;
        
        let next_node = match find_one_by_chain(platform, chain).await {
            Ok(n) => n,
            Err(_) => continue, // Element might temporarily disappear during animation
        };

        if current_node.position == next_node.position && current_node.size == next_node.size {
            return Ok(next_node);
        }

        current_node = next_node;

        if start.elapsed() >= timeout {
            return Err(Error::Timeout {
                seconds: timeout.as_secs_f64(),
                message: "element did not stabilize within timeout".into(),
            });
        }
    }
}
```

- [ ] **Step 2: Use `poll_for_stability` in `cli/src/actions.rs`**

In `actions.rs`, inside `find_element`:

```rust
pub async fn find_element(
    platform: &dyn Platform,
    chain: &agent_click_core::selector::SelectorChain,
    timeout: Option<std::time::Duration>,
) -> agent_click_core::Result<agent_click_core::AccessibilityNode> {
    let wait_timeout = timeout.unwrap_or(std::time::Duration::from_secs(5));
    let interval = std::time::Duration::from_millis(200);

    let initial_node = crate::wait::poll_for_one_element(platform, chain, wait_timeout, interval).await?;
    
    // Ensure the element has stopped animating before returning it for interaction
    crate::wait::poll_for_stability(platform, chain, initial_node, wait_timeout).await
}
```

- [ ] **Step 3: Commit**

```bash
git add cli/src/wait.rs cli/src/actions.rs
git commit -m "feat(cli): add animation stability polling before interactions"
```
