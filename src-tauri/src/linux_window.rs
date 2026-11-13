use tauri::{App, WebviewWindowBuilder};

pub fn rounded_corners_supported() -> bool {
    if std::env::var_os("MKSS_SQUARE_CORNERS").is_some() {
        return false;
    }
    gtk::gdk::Screen::default().is_some_and(|screen| screen.is_composited())
}

pub fn create(app: &App) -> tauri::Result<()> {
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == "main" && !w.create).cloned() else {
        return Ok(());
    };
    let rounded = rounded_corners_supported();
    let mut builder = WebviewWindowBuilder::from_config(app.handle(), &config)?.transparent(rounded);
    if rounded {
        builder = builder.initialization_script("window.__MKSS_ROUNDED__ = true;");
    }
    builder.build()?;
    Ok(())
}
