use std::sync::{Arc, Mutex};
use tauri::{LogicalSize, PhysicalSize, WebviewWindow, WindowEvent};

type LastMinimum = Arc<Mutex<Option<PhysicalSize<u32>>>>;

fn minimum_with_frame(
    scale: f64,
    inner: PhysicalSize<u32>,
    outer: PhysicalSize<u32>,
) -> PhysicalSize<u32> {
    let client = LogicalSize::new(1280.0, 720.0).to_physical::<u32>(scale);
    PhysicalSize::new(
        client.width.saturating_add(outer.width.saturating_sub(inner.width)),
        client.height.saturating_add(outer.height.saturating_sub(inner.height)),
    )
}

fn update(window: &WebviewWindow, last: &LastMinimum) -> tauri::Result<()> {
    if window.is_maximized()? || window.is_minimized()? || window.is_fullscreen()? {
        return Ok(());
    }
    let inner = window.inner_size()?;
    if inner.width == 0 || inner.height == 0 {
        return Ok(());
    }
    let minimum = minimum_with_frame(window.scale_factor()?, inner, window.outer_size()?);
    {
        let mut previous = last.lock().expect("window minimum cache poisoned");
        if *previous == Some(minimum) {
            return Ok(());
        }
        *previous = Some(minimum);
    }
    let result = window.set_min_size(Some(minimum));
    if result.is_err() {
        *last.lock().expect("window minimum cache poisoned") = None;
    }
    result
}

pub fn install(window: &WebviewWindow) -> tauri::Result<()> {
    let last = Arc::new(Mutex::new(None));
    update(window, &last)?;
    let owned = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }) {
            if let Err(error) = update(&owned, &last) {
                eprintln!("Could not update minimum client size: {error}");
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_frame_does_not_allow_content_to_shrink_at_100_percent() {
        assert_eq!(
            minimum_with_frame(1.0, PhysicalSize::new(1280, 720), PhysicalSize::new(1296, 729)),
            PhysicalSize::new(1296, 729),
        );
    }

    #[test]
    fn frame_and_minimum_respect_monitor_scale() {
        assert_eq!(
            minimum_with_frame(1.5, PhysicalSize::new(1920, 1080), PhysicalSize::new(1944, 1093)),
            PhysicalSize::new(1944, 1093),
        );
    }

    #[test]
    fn resizing_larger_does_not_change_minimum() {
        assert_eq!(
            minimum_with_frame(1.0, PhysicalSize::new(1500, 850), PhysicalSize::new(1516, 859)),
            PhysicalSize::new(1296, 729),
        );
    }

    #[test]
    fn zero_or_transient_negative_frame_does_not_underflow() {
        assert_eq!(
            minimum_with_frame(1.0, PhysicalSize::new(1280, 720), PhysicalSize::new(1270, 710)),
            PhysicalSize::new(1280, 720),
        );
    }
}
