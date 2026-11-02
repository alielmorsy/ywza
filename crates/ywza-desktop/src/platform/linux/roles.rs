//! AT-SPI role -> the control kinds of the shared element model (same names as the
//! Windows UIA mapping, see `snapshot/`).

use atspi::Role;

pub fn kind(role: Role) -> &'static str {
    use Role::*;
    match role {
        Button | PushButtonMenu | ToggleButton => "button",
        Entry | PasswordText | Text => "edit",
        DocumentFrame | DocumentWeb | DocumentText | DocumentEmail | DocumentSpreadsheet
        | DocumentPresentation => "document",
        CheckBox => "checkbox",
        RadioButton => "radio",
        ComboBox => "combobox",
        Link => "link",
        MenuItem | CheckMenuItem | RadioMenuItem | TearoffMenuItem => "menuitem",
        PageTab => "tabitem",
        ListItem => "listitem",
        TreeItem => "treeitem",
        TableCell => "dataitem",
        Slider => "slider",
        SpinButton => "spinner",
        ColumnHeader | RowHeader | TableColumnHeader | TableRowHeader => "headeritem",
        Label | Caption | Heading | Paragraph => "text",
        Image | Icon => "image",
        ProgressBar | LevelBar => "progressbar",
        Dialog | Alert | FileChooser | ColorChooser | FontChooser => "dialog",
        Panel | Filler | ScrollPane | Viewport | LayeredPane | RootPane | SplitPane | Frame
        | Window | InternalFrame => "pane",
        Section | Grouping | Form => "group",
        List | ListBox => "list",
        Tree | TreeTable => "tree",
        PageTabList => "tab",
        Menu | PopupMenu => "menu",
        MenuBar => "menubar",
        ToolBar => "toolbar",
        StatusBar => "statusbar",
        ScrollBar => "scrollbar",
        TitleBar => "titlebar",
        _ => "other",
    }
}

/// Roles whose checked/pressed state is the toggle state.
pub fn is_toggle(role: Role) -> bool {
    matches!(
        role,
        Role::CheckBox | Role::ToggleButton | Role::CheckMenuItem | Role::RadioMenuItem
    )
}

/// Roles that carry editable text worth reading back as the element value.
pub fn has_text_value(role: Role) -> bool {
    matches!(role, Role::Entry | Role::Text)
}

/// Roles that report a number through the Value interface.
pub fn has_number_value(role: Role) -> bool {
    matches!(role, Role::Slider | Role::SpinButton | Role::ScrollBar)
}

/// Top-level children of an application that are popups of its windows, not windows.
pub fn is_popup_root(role: Role) -> bool {
    matches!(role, Role::PopupMenu | Role::Menu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_like_windows() {
        assert_eq!(kind(Role::Button), "button");
        assert_eq!(kind(Role::Entry), "edit");
        assert_eq!(kind(Role::Text), "edit");
        assert_eq!(kind(Role::CheckBox), "checkbox");
        assert_eq!(kind(Role::MenuItem), "menuitem");
        assert_eq!(kind(Role::PageTab), "tabitem");
        assert_eq!(kind(Role::ListItem), "listitem");
        assert_eq!(kind(Role::Link), "link");
        assert_eq!(kind(Role::ComboBox), "combobox");
        assert_eq!(kind(Role::Label), "text");
        assert_eq!(kind(Role::Dialog), "dialog");
        assert_eq!(kind(Role::Panel), "pane");
        assert_eq!(kind(Role::Unknown), "other");
    }

    #[test]
    fn every_kind_is_a_known_name() {
        // Kinds the shared model knows; "other" and the containers are never listed.
        let listed = [
            "button",
            "edit",
            "document",
            "checkbox",
            "radio",
            "combobox",
            "link",
            "menuitem",
            "tabitem",
            "listitem",
            "treeitem",
            "dataitem",
            "slider",
            "spinner",
            "headeritem",
            "text",
            "image",
            "progressbar",
            "dialog",
        ];
        for r in [
            Role::Button,
            Role::Entry,
            Role::DocumentWeb,
            Role::RadioButton,
            Role::TreeItem,
            Role::TableCell,
            Role::Slider,
            Role::SpinButton,
            Role::ColumnHeader,
            Role::Image,
            Role::ProgressBar,
        ] {
            assert!(listed.contains(&kind(r)), "{r:?}");
        }
        assert!(is_toggle(Role::CheckBox) && !is_toggle(Role::Button));
    }
}
