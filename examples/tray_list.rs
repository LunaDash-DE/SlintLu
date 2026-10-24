//! Lists system tray items and their menus (read-only): `cargo run --example tray_list`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tray = slintlu::dbus::tray::Tray::connect()?;
    for item in tray.items()? {
        println!(
            "{:<45} title={:?} icon={:?} pixmap={} status={:?} menu={:?}",
            item.service,
            item.title,
            item.icon_name,
            item.icon.as_ref().map_or("-".into(), |i| format!("{}x{}", i.width, i.height)),
            item.status,
            item.menu_path
        );
        match tray.menu(&item) {
            Ok(menu) => {
                for entry in menu.iter().take(6) {
                    let indent = "  ".repeat(entry.depth as usize + 1);
                    let label =
                        if entry.separator { "────".into() } else { entry.label.clone() };
                    println!(
                        "{indent}[{}] {label}{}",
                        entry.id,
                        if entry.has_children { " ▸" } else { "" }
                    );
                }
            }
            Err(e) => println!("  menu error: {e}"),
        }
    }
    Ok(())
}
