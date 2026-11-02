use ::windows::Win32::UI::Accessibility::*;

#[allow(non_upper_case_globals)] // windows-rs names the UIA ids in CamelCase
pub(super) fn role(ct: UIA_CONTROLTYPE_ID) -> &'static str {
    match ct {
        UIA_ButtonControlTypeId => "button",
        UIA_SplitButtonControlTypeId => "splitbutton",
        UIA_EditControlTypeId => "edit",
        UIA_DocumentControlTypeId => "document",
        UIA_CheckBoxControlTypeId => "checkbox",
        UIA_RadioButtonControlTypeId => "radio",
        UIA_ComboBoxControlTypeId => "combobox",
        UIA_HyperlinkControlTypeId => "link",
        UIA_MenuItemControlTypeId => "menuitem",
        UIA_TabItemControlTypeId => "tabitem",
        UIA_ListItemControlTypeId => "listitem",
        UIA_TreeItemControlTypeId => "treeitem",
        UIA_DataItemControlTypeId => "dataitem",
        UIA_SliderControlTypeId => "slider",
        UIA_SpinnerControlTypeId => "spinner",
        UIA_HeaderItemControlTypeId => "headeritem",
        UIA_TextControlTypeId => "text",
        UIA_ImageControlTypeId => "image",
        UIA_ProgressBarControlTypeId => "progressbar",
        // A window inside the snapshot root is a dialog or popup.
        UIA_WindowControlTypeId => "dialog",
        UIA_PaneControlTypeId => "pane",
        UIA_GroupControlTypeId => "group",
        UIA_CustomControlTypeId => "custom",
        UIA_ListControlTypeId => "list",
        UIA_TreeControlTypeId => "tree",
        UIA_TabControlTypeId => "tab",
        UIA_MenuControlTypeId => "menu",
        UIA_MenuBarControlTypeId => "menubar",
        UIA_ToolBarControlTypeId => "toolbar",
        UIA_StatusBarControlTypeId => "statusbar",
        UIA_ScrollBarControlTypeId => "scrollbar",
        UIA_TitleBarControlTypeId => "titlebar",
        _ => "other",
    }
}
