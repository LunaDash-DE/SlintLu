//! `com.canonical.dbusmenu`: the menus of tray items.

use std::collections::HashMap;

use zbus::{Connection, OwnedVariant, Variant, proxy};

#[proxy(interface = "com.canonical.dbusmenu")]
trait DBusMenu {
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: &[&str],
    ) -> zbus::Result<(u32, (i32, HashMap<String, OwnedVariant>, Vec<OwnedVariant>))>;
    fn event(
        &self,
        id: i32,
        event_id: &str,
        data: &Variant<'_>,
        timestamp: u32,
    ) -> zbus::Result<()>;
    fn about_to_show(&self, id: i32) -> zbus::Result<bool>;
}

/// One menu entry. Submenus are flattened: their children follow with a larger `depth`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayMenuItem {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    /// "checkmark", "radio" or "".
    pub toggle_type: String,
    /// 1 checked, 0 unchecked, -1 indeterminate.
    pub toggle_state: i32,
    pub icon_name: String,
    pub depth: u32,
    pub has_children: bool,
}

type Properties = HashMap<String, OwnedVariant>;

fn prop_str(props: &Properties, key: &str) -> String {
    props.get(key).and_then(|v| String::try_from(&**v).ok()).unwrap_or_default()
}

/// Menu labels mark mnemonics with `_` (`__` is a literal underscore).
fn strip_mnemonic(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '_' {
            if chars.peek() == Some(&'_') {
                out.push('_');
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Values of `a{sv}` dictionaries arrive wrapped in a variant.
fn unwrap<'a>(value: &'a Variant<'a>) -> &'a Variant<'a> {
    match value {
        Variant::Variant(inner) => unwrap(inner),
        other => other,
    }
}

/// Decodes a `(ia{sv}av)` layout node.
fn decode_node(value: &Variant<'_>) -> Option<(i32, Properties, Vec<OwnedVariant>)> {
    let value = match value {
        Variant::Variant(inner) => inner.as_ref(),
        other => other,
    };
    let structure = match value {
        Variant::Structure(s) => s,
        _ => return None,
    };
    let fields = structure.fields();
    let id = i32::try_from(fields.first()?).ok()?;
    let props: Properties = match fields.get(1)? {
        Variant::Dict(dict) => dict
            .iter()
            .filter_map(|(k, v)| {
                Some((String::try_from(unwrap(k)).ok()?, OwnedVariant::try_from(unwrap(v)).ok()?))
            })
            .collect(),
        _ => Properties::new(),
    };
    let children = match fields.get(2)? {
        Variant::Array(array) => {
            array.iter().filter_map(|v| OwnedVariant::try_from(v).ok()).collect()
        }
        _ => Vec::new(),
    };
    Some((id, props, children))
}

fn flatten(
    id: i32,
    props: &Properties,
    children: &[OwnedVariant],
    depth: u32,
    out: &mut Vec<TrayMenuItem>,
) {
    let visible = props.get("visible").and_then(|v| bool::try_from(&**v).ok()).unwrap_or(true);
    if !visible {
        return;
    }
    // The root (id 0) is the menu itself, not an entry.
    let child_depth = if id == 0 && depth == 0 {
        0
    } else {
        out.push(TrayMenuItem {
            id,
            label: strip_mnemonic(&prop_str(props, "label")),
            enabled: props.get("enabled").and_then(|v| bool::try_from(&**v).ok()).unwrap_or(true),
            separator: prop_str(props, "type") == "separator",
            toggle_type: prop_str(props, "toggle-type"),
            toggle_state: props
                .get("toggle-state")
                .and_then(|v| i32::try_from(&**v).ok())
                .unwrap_or(-1),
            icon_name: prop_str(props, "icon-name"),
            depth,
            has_children: !children.is_empty() || prop_str(props, "children-display") == "submenu",
        });
        depth + 1
    };
    for child in children {
        if let Some((cid, cprops, cchildren)) = decode_node(child) {
            flatten(cid, &cprops, &cchildren, child_depth, out);
        }
    }
}

async fn proxy<'a>(
    connection: &Connection,
    bus: &str,
    path: &str,
) -> zbus::Result<DBusMenuProxy<'a>> {
    DBusMenuProxy::builder(connection)
        .destination(bus.to_owned())
        .path(path.to_owned())
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
}

pub(super) async fn fetch(
    connection: &Connection,
    bus: &str,
    path: &str,
) -> zbus::Result<Vec<TrayMenuItem>> {
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let menu = proxy(connection, bus, path).await?;
    // Lets applications fill lazily built menus; failures are harmless.
    let _ = menu.about_to_show(0).await;
    let (_, (id, props, children)) = menu.get_layout(0, -1, &[]).await?;
    let mut items = Vec::new();
    flatten(id, &props, &children, 0, &mut items);
    Ok(items)
}

pub(super) async fn clicked(
    connection: &Connection,
    bus: &str,
    path: &str,
    id: i32,
) -> zbus::Result<()> {
    let menu = proxy(connection, bus, path).await?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);
    menu.event(id, "clicked", &Variant::from(""), timestamp).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonics_are_removed() {
        assert_eq!(strip_mnemonic("_Quit"), "Quit");
        assert_eq!(strip_mnemonic("Save __as"), "Save _as");
        assert_eq!(strip_mnemonic("Plain"), "Plain");
    }
}
